use crate::{cco_document, visit};
use hcl::{Expression, Identifier, Traversal, TraversalOperator};

#[derive(derive_new::new)]
pub(crate) struct AttributeReferenceRewriter<'d> {
    documents: &'d cco_document::CcoDocument,
}

impl<'d> visit::VisitMut<Traversal> for AttributeReferenceRewriter<'d> {
    fn visit_mut(&mut self, traversal: &mut Traversal) {
        // was already rewritten
        if let Expression::Variable(var) = &traversal.expr {
            if var.starts_with("cco__") {
                return;
            }
        }

        let path = traversal.get_longest_path();
        let Some((subst, len)) = self.documents.get_most_specific_node(&path) else {
            return;
        };

        traversal.apply_substitution(Expression::Variable(subst.clone().into()), len);
    }
}

#[derive(derive_new::new)]
pub(crate) struct SelfRewriter<'a> {
    block_name: &'a [Identifier],
}

impl<'a> visit::VisitMut<Traversal> for SelfRewriter<'a> {
    #[tracing::instrument(level = "trace", skip_all)]
    fn visit_mut(&mut self, traversal: &mut Traversal) {
        let Expression::Variable(var) = &traversal.expr else {
            return;
        };

        if var.as_str() != "self" {
            return;
        }

        if let Some(TraversalOperator::Index(Expression::Number(num))) = traversal.operators.first()
        {
            if let Some(idx) = num.as_u64() {
                let idx = idx as usize;
                if idx < self.block_name.len() {
                    traversal.apply_substitution(
                        Expression::String(self.block_name[idx].clone().to_string()),
                        2,
                    );
                    return;
                }
            }
        }

        let mut path = self.block_name.iter().cloned();
        let mut block_traversal =
            Traversal::builder(Expression::Variable(path.next().unwrap().into()));
        for element in path {
            block_traversal = block_traversal.attr(element);
        }
        traversal.apply_substitution(block_traversal.build().into(), 1);
    }
}

trait TraversalExt {
    fn apply_substitution(&mut self, expr: Expression, path_len: usize);
    fn get_longest_path(&self) -> Vec<Identifier>;
    fn squash(&mut self);
}

impl TraversalExt for Traversal {
    #[tracing::instrument(level = "trace")]
    fn apply_substitution(&mut self, expr: Expression, path_len: usize) {
        let remove = path_len
            .checked_sub(1)
            .expect("must remove at least one element when substituting");

        self.expr = expr;

        if remove >= self.operators.len() {
            self.operators.clear();
        } else {
            self.operators.rotate_left(remove);
            self.operators.truncate(self.operators.len() - remove);
        }

        // HACK: Flatten traversal if case we were passed a traversal as expression
        self.squash();

        tracing::trace!(traversal=?self,"after substitution");
    }

    fn get_longest_path(&self) -> Vec<Identifier> {
        let Expression::Variable(var) = &self.expr else {
            return vec![];
        };

        let mut path = vec![hcl::Identifier::unchecked(var.as_str())];
        for operator in &self.operators {
            let TraversalOperator::GetAttr(ident) = operator else {
                break;
            };

            path.push(ident.clone());
        }

        path
    }

    /// Squash nested Traversals
    ///
    /// If a [Traversal]'s Expression is a [hcl::Expression::Traversal]
    /// merge them as if they are one traversal.
    ///
    /// Turns `<foo.bar>.baz` into `foo.bar.baz`.
    ///
    /// This is an internal workaround - parsed documents usually do not contain nested traversals.
    fn squash(&mut self) {
        let Traversal {
            expr: Expression::Traversal(inner),
            operators,
            ..
        } = self
        else {
            return;
        };

        inner.operators.append(operators);
        std::mem::swap(&mut self.operators, &mut inner.operators);

        self.expr = std::mem::replace(&mut inner.expr, Expression::Null);
        tracing::trace!(traversal=?self, "traversal squashed")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn apply_substitution() {
        let mut traversal = Traversal::builder(hcl::Variable::unchecked("one"))
            .attr("two")
            .attr("three")
            .attr("four")
            .build();

        traversal.apply_substitution(hcl::Variable::unchecked("substitution").into(), 3);

        let expected = Traversal::builder(hcl::Variable::unchecked("substitution"))
            .attr("four")
            .build();

        assert_eq!(traversal, expected);
    }
}
