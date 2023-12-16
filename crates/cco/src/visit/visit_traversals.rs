use super::VisitMut;
use hcl::{
    template::{Directive, Element},
    Body, Expression, Operation, Structure, Template, TemplateExpr, Traversal, TraversalOperator,
};

/// Recursively visit all [hcl::Traversal]s mutably
pub trait VisitTraversalsMut {
    fn visit_traversals_mut(&mut self, visitor: &mut dyn VisitMut<Traversal>);
}

impl VisitTraversalsMut for Body {
    fn visit_traversals_mut(&mut self, visitor: &mut dyn VisitMut<Traversal>) {
        for structure in self {
            match structure {
                Structure::Attribute(attr) => attr.expr.visit_traversals_mut(visitor),
                Structure::Block(block) => block.body.visit_traversals_mut(visitor),
            }
        }
    }
}

impl VisitTraversalsMut for Expression {
    fn visit_traversals_mut(&mut self, visitor: &mut dyn VisitMut<Traversal>) {
        match self {
            Expression::Variable(variable) => {
                // a standalone variable is a traversal with no operators...kind of
                let mut traversal = Traversal::new(
                    Expression::Variable(variable.clone()),
                    Vec::<TraversalOperator>::new(),
                );
                visitor.visit_mut(&mut traversal);
                if let Expression::Variable(new_variable) = traversal.expr {
                    *variable = new_variable
                } else {
                    panic!("Traversal rewrite caused a variable to become something else");
                }
            }
            Expression::Traversal(traversal) => {
                visitor.visit_mut(traversal);
                traversal.expr.visit_traversals_mut(visitor);
            }
            Expression::Array(array) => {
                for expr in array {
                    expr.visit_traversals_mut(visitor);
                }
            }
            Expression::Object(object) => {
                for value in object.values_mut() {
                    value.visit_traversals_mut(visitor);
                }
            }
            Expression::TemplateExpr(template_expr) => {
                let mut template = Template::from_expr(template_expr).unwrap();
                template.visit_traversals_mut(visitor);
                // FIXME: Does template round-trip properly?
                *template_expr = Box::new(TemplateExpr::QuotedString(template.to_string()));
            }
            Expression::FuncCall(_) => {}
            Expression::Parenthesis(expr) => {
                expr.visit_traversals_mut(visitor);
            }
            Expression::Conditional(cond) => {
                cond.cond_expr.visit_traversals_mut(visitor);
                cond.true_expr.visit_traversals_mut(visitor);
                cond.false_expr.visit_traversals_mut(visitor);
            }
            Expression::Operation(operation) => match operation.as_mut() {
                Operation::Binary(binop) => {
                    binop.rhs_expr.visit_traversals_mut(visitor);
                    binop.lhs_expr.visit_traversals_mut(visitor);
                }
                Operation::Unary(unop) => {
                    unop.expr.visit_traversals_mut(visitor);
                }
            },
            Expression::ForExpr(forexpr) => {
                forexpr
                    .cond_expr
                    .iter_mut()
                    .for_each(|e| e.visit_traversals_mut(visitor));
                forexpr
                    .key_expr
                    .iter_mut()
                    .for_each(|e| e.visit_traversals_mut(visitor));
                forexpr.value_expr.visit_traversals_mut(visitor);
                forexpr.collection_expr.visit_traversals_mut(visitor);
            }
            _ => {}
        }
    }
}

impl VisitTraversalsMut for Template {
    fn visit_traversals_mut(&mut self, visitor: &mut dyn VisitMut<Traversal>) {
        for element in self.elements_mut() {
            match element {
                Element::Interpolation(interpolation) => {
                    interpolation.expr.visit_traversals_mut(visitor);
                }
                Element::Directive(directive) => match directive {
                    Directive::If(ifdir) => {
                        ifdir.cond_expr.visit_traversals_mut(visitor);
                        ifdir.true_template.visit_traversals_mut(visitor);
                        ifdir
                            .false_template
                            .iter_mut()
                            .for_each(|t| t.visit_traversals_mut(visitor));
                    }
                    Directive::For(fordir) => {
                        fordir.template.visit_traversals_mut(visitor);
                        fordir.collection_expr.visit_traversals_mut(visitor);
                    }
                },
                Element::Literal(_) => {}
            }
        }
    }
}
