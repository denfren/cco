//! Collection of known [Addressable]s
use crate::hcl_documents::HclDocuments;
use crate::util::{AttributeReferenceRewriter, SelfRewriter};
use crate::visit::VisitTraversalsMut;
use hcl::eval::{ErrorKind, Evaluate};

/// Multiple HCL Documents containing valid CCO blocks
#[derive(Debug)]
pub struct CcoDocument {
    /// All addressable expressions
    addressables: Vec<Addressable>,

    /// Helper structure for path traversal
    ///
    /// indices point to self.addressables
    tree: Tree,
}

impl CcoDocument {
    pub fn new(hcl_documents: &HclDocuments) -> Result<Self, CcoParseErrors> {
        let mut _self = Self {
            tree: Default::default(),
            addressables: Default::default(),
        };

        let mut e = CcoParseErrors::new();
        let mut data_groups: std::collections::HashMap<hcl::Identifier, DataGroup> =
            Default::default();
        let mut type_specs: std::collections::HashMap<hcl::Identifier, usize> = Default::default();

        for (index, _source, _attribute) in hcl_documents.attributes() {
            e.log(Issue::RootAttribute(index))
        }

        for (index, _source, block) in hcl_documents.blocks() {
            match block.ident.value().as_str() {
                "data" => {
                    if block.labels.is_empty() {
                        e.log(Issue::DataBlockLabelMissing(index));
                        break;
                    }

                    let data_block = DataBlock::new(index, block);

                    let group: &mut DataGroup =
                        if data_groups.contains_key(data_block.identifiers[0].as_str()) {
                            data_groups.get_mut(&data_block.identifiers[0]).unwrap()
                        } else {
                            data_groups.insert(data_block.identifiers[0].clone(), DataGroup::new());
                            data_groups.get_mut(&data_block.identifiers[0]).unwrap()
                        };

                    if let Some(existing_member) = group.data_blocks.first() {
                        if existing_member.identifiers.len() != block.labels.len() {
                            e.log(Issue::DataBlockLabelMismatch {
                                existing: existing_member.block_index,
                                new: index,
                            });
                            continue;
                        }
                    }

                    if let Some(existing) = group
                        .data_blocks
                        .iter()
                        .find(|existing_block| *existing_block == &data_block)
                    {
                        e.log(Issue::DataBlockLabelCollision {
                            existing: existing.block_index,
                            new: index,
                        });
                        continue;
                    }

                    group.data_blocks.push(DataBlock::new(index, block));
                }
                "type" => {
                    if block.labels.is_empty() {
                        e.log(Issue::TypeBlockLabelMissing(index));
                        continue;
                    }

                    if block.labels.len() > 1 {
                        e.log(Issue::TypeBlockTooManyLabels(index));
                        continue;
                    }

                    let type_name = hcl::Identifier::sanitized(block.labels[0].as_str());

                    if let Some(existing_type_spec) = type_specs.get(&type_name) {
                        e.log(Issue::TypeBlockLabelCollision {
                            new: index,
                            existing: *existing_type_spec,
                        });
                        continue;
                    }

                    type_specs.insert(type_name, index);
                }
                _ => e.log(Issue::UnknownBlockType(index)),
            }
        }

        if !e.issues.is_empty() {
            return Err(e);
        };

        for data_block in data_groups.iter().flat_map(|(_, group)| &group.data_blocks) {
            // direct attributes
            let data_block_hcl = hcl_documents.get_block(data_block.block_index);
            for attribute in data_block_hcl.2.body.attributes() {
                let mut path = data_block.identifiers.clone();
                path.push(hcl::Identifier::sanitized(attribute.key.value()));

                tracing::trace!(?path, "add direct attribute");
                assert!(
                    _self
                        .insert(Kind::Attribute, path, attribute.value.clone().into())
                        .is_ok(),
                    "attribute collision: {:?}.{:?}",
                    data_block.identifiers,
                    attribute.key.value(),
                );
            }

            // default/fallback attributes
            if let Some(type_spec_index) = type_specs.get(&data_block.identifiers[0]).copied() {
                let type_spec_hcl = hcl_documents.get_block(type_spec_index);
                for attribute in type_spec_hcl.2.body.attributes() {
                    let mut path = data_block.identifiers.clone();
                    path.push(hcl::Identifier::sanitized(attribute.key.value()));

                    // not being added means that we already have a direct attribute. ignore.
                    let _ =
                        _self.insert(Kind::DefaultAttribute, path, attribute.value.clone().into());
                }
            }

            // insert object
            let node = _self.tree.get_or_insert(&data_block.identifiers);
            let mut data_block_expression: hcl::Object<hcl::ObjectKey, hcl::Expression> =
                Default::default();
            for (ident, child_node) in &node.children {
                if let Some(addressable) = child_node.value {
                    let addr = &_self.addressables[addressable];
                    data_block_expression.insert(
                        ident.clone().into(),
                        hcl::Expression::Variable(addr.subst.clone().into()),
                    );
                }
            }

            assert!(
                _self
                    .insert(
                        Kind::Block,
                        data_block.identifiers.clone(),
                        hcl::Expression::Object(data_block_expression),
                    )
                    .is_ok(),
                "data block object collision {:?}",
                data_block.identifiers
            );
        }

        let mut root_groups = vec![];
        for (ident, group) in _self.tree.root.iter() {
            if group.value.is_none() {
                let children: hcl::Object<hcl::ObjectKey, hcl::Expression> = group
                    .children
                    .iter()
                    .flat_map(|(key, value)| {
                        value.value.map(|index| {
                            (
                                key.clone().into(),
                                hcl::Expression::Variable(
                                    _self.addressables[index].subst.clone().into(),
                                ),
                            )
                        })
                    })
                    .collect();

                root_groups.push((ident.clone(), children));
            }
        }

        for (ident, children) in root_groups {
            let _ = _self.insert(
                Kind::Virtual,
                vec![ident],
                hcl::Expression::Object(children),
            );
        }

        Ok(_self)
    }

    /// Insert a new addressable
    ///
    /// Returns new index when added or existing index when failed
    fn insert(
        &mut self,
        kind: Kind,
        path: Vec<hcl::Identifier>,
        expression: hcl::Expression,
    ) -> Result<usize, usize> {
        let node = self.tree.get_or_insert(&path);
        if let Some(existing) = node.value {
            tracing::debug!(?path, "collision");
            return Err(existing);
        }

        let index = self.addressables.len();
        node.value = Some(index);

        self.addressables
            .push(Addressable::new(path, kind, expression));

        Ok(index)
    }

    pub fn get_by_subst(&self, subst: &hcl::Identifier) -> Option<&Addressable> {
        self.addressables.iter().find(|addr| &addr.subst == subst)
    }

    pub fn get_most_specific_node(
        &self,
        path: &[hcl::Identifier],
    ) -> Option<(&hcl::Identifier, usize)> {
        self.tree
            .get(path)
            .map(|(idx, ident)| (&self.addressables[idx].subst, path.len() - ident.len()))
    }

    fn get_by_subst_and_rewrite(&self, ident: &hcl::Identifier) -> Option<hcl::Expression> {
        self.get_by_subst(ident).map(|addressable| {
            let mut expr = addressable.expression.clone();

            let block_path = &addressable.path[0..(addressable.path.len() - 1)];
            let mut self_rewriter = SelfRewriter::new(block_path);
            expr.visit_traversals_mut(&mut self_rewriter);

            let mut dependency_writer = AttributeReferenceRewriter::new(self);
            expr.visit_traversals_mut(&mut dependency_writer);

            expr
        })
    }

    pub fn evaluate_in_context(
        &self,
        mut expression: hcl::Expression,
    ) -> anyhow::Result<crate::value::Value> {
        let mut dependency_writer = AttributeReferenceRewriter::new(self);
        expression.visit_traversals_mut(&mut dependency_writer);

        let mut context = hcl::eval::Context::new();
        let mut stack = vec![(hcl::Identifier::unchecked("output"), expression)];

        while let Some((current, mut expression)) = stack.pop() {
            let Err(eval_errors) = expression.evaluate_in_place(&context) else {
                if stack.is_empty() {
                    return Ok(expression.into());
                }

                context.declare_var(current, expression);
                continue;
            };

            // we did not succeed
            stack.push((current, expression));

            if let Some(err) = eval_errors.iter().next() {
                let ErrorKind::UndefinedVar(var) = err.kind() else {
                    // some other error
                    return Err(eval_errors.into());
                };

                if !var.starts_with("cco__") {
                    // unknown identifier
                    return Err(eval_errors.into());
                }

                if stack
                    .iter()
                    .any(|(ident, _)| ident.as_str() == var.as_str())
                {
                    // loop detected
                    dbg!(stack);
                    if let Some(resolved_addressable) = self.get_by_subst(var) {
                        anyhow::bail!("Loop detected at {:?} ({var})", resolved_addressable.path);
                    } else {
                        anyhow::bail!("Loop detected {var}");
                    }
                }

                let Some(expr) = self.get_by_subst_and_rewrite(&var) else {
                    anyhow::bail!("Missing internal dependency {var}");
                };

                stack.push((var.clone(), expr));
            } else {
                panic!("evaluation errored but no error was returned");
            }
        }

        unreachable!();
    }
}

#[derive(derive_new::new, Debug)]
pub struct DataGroup {
    #[new(default)]
    pub data_blocks: Vec<DataBlock>,
}

#[derive(Debug)]
pub struct DataBlock {
    pub identifiers: Vec<hcl::Identifier>,
    pub block_index: usize,
}

// FIXME: Revisit if this is a good idea. A DataBlock must be unique in its labels, so this should be ok.
impl PartialEq for DataBlock {
    fn eq(&self, other: &Self) -> bool {
        self.identifiers.eq(&other.identifiers)
    }
}

impl DataBlock {
    pub fn new(block_index: usize, block: &hcl_edit::structure::Block) -> Self {
        let identifiers: Vec<_> = block
            .labels
            .iter()
            .map(hcl::Identifier::sanitized)
            .collect();

        assert!(
            !identifiers.is_empty(),
            "data block labels must not be empty"
        );

        Self {
            block_index,
            identifiers,
        }
    }
}

#[derive(derive_new::new, Debug)]
pub struct CcoParseErrors {
    #[new(default)]
    issues: Vec<Issue>,
}

impl CcoParseErrors {
    pub fn log(&mut self, issue: Issue) {
        tracing::trace!(?issue, "issue found");
        self.issues.push(issue);
    }
}

impl std::error::Error for CcoParseErrors {}

impl std::fmt::Display for CcoParseErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Debug;
        self.issues.first().unwrap().fmt(f)
    }
}

#[derive(Debug, PartialEq)]
pub enum Issue {
    RootAttribute(usize),
    UnknownBlockType(usize),
    DataBlockLabelMissing(usize),
    DataBlockLabelCollision { existing: usize, new: usize },
    DataBlockLabelMismatch { existing: usize, new: usize },
    TypeBlockLabelMissing(usize),
    TypeBlockTooManyLabels(usize),
    TypeBlockLabelCollision { existing: usize, new: usize },
}

#[derive(Debug, Default)]
pub struct Tree {
    pub root: indexmap::IndexMap<hcl::Identifier, Node>,
}

impl Tree {
    fn get<'a>(
        &'a self,
        key_path: &'a [hcl::Identifier],
    ) -> Option<(usize, &'a [hcl::Identifier])> {
        if key_path.is_empty() {
            return None;
        }

        self.root
            .get(&key_path[0])
            .and_then(|child| child.get(&key_path[1..]))
    }

    fn get_or_insert(&mut self, key_path: &[hcl::Identifier]) -> &mut Node {
        let key = &key_path[0];

        let child = if self.root.contains_key(key) {
            self.root.get_mut(key).unwrap()
        } else {
            self.root.insert(key.clone(), Default::default());
            self.root.last_mut().map(|(_key, value)| value).unwrap()
        };

        child.get_or_insert(&key_path[1..])
    }
}

#[derive(Debug, derive_new::new)]
pub struct Node {
    pub value: Option<usize>,
    #[new(default)]
    pub children: indexmap::IndexMap<hcl::Identifier, Node>,
}

impl Node {
    fn get<'a>(
        &'a self,
        key_path: &'a [hcl::Identifier],
    ) -> Option<(usize, &'a [hcl::Identifier])> {
        if key_path.is_empty() {
            return self.value.as_ref().map(|r| (*r, [].as_slice()));
        }

        if let Some(result) = self.children.get(&key_path[0]) {
            if let Some(result) = result.get(&key_path[1..]) {
                return Some(result);
            }
        }

        self.value.as_ref().map(|r| (*r, key_path))
    }

    fn get_or_insert(&mut self, key_path: &[hcl::Identifier]) -> &mut Node {
        if key_path.is_empty() {
            return self;
        }

        let key = &key_path[0];
        let next = if self.children.contains_key(key) {
            self.children.get_mut(key).unwrap()
        } else {
            self.children.insert(key.clone(), Default::default());
            self.children.last_mut().map(|(_key, value)| value).unwrap()
        };

        next.get_or_insert(&key_path[1..])
    }
}

impl Default for Node {
    fn default() -> Self {
        Self {
            children: Default::default(),
            value: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct Addressable {
    pub path: Vec<hcl::Identifier>,
    pub kind: Kind,
    pub expression: hcl::expr::Expression,
    pub subst: hcl::Identifier,
}

impl Addressable {
    fn new(path: Vec<hcl::Identifier>, kind: Kind, expression: hcl::expr::Expression) -> Self {
        let subst = format!("cco__{}_{}", kind, path.join("__")).into();
        Self {
            path,
            kind,
            expression,
            subst,
        }
    }
}

#[derive(Debug)]
pub enum Kind {
    /// A "proper" attribute
    Attribute,
    /// A attribute with a default expression
    DefaultAttribute,
    /// A block
    Block,
    /// A node that refers to all its children
    Virtual,
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kind::Attribute => f.write_str("attribute"),
            Kind::DefaultAttribute => f.write_str("defaultattribute"),
            Kind::Block => f.write_str("block"),
            Kind::Virtual => f.write_str("virtual"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::hcl_documents;

    fn cco_parse_errors_for(doc: HclDocuments) -> CcoParseErrors {
        CcoDocument::new(&doc).expect_err("must error")
    }

    #[test]
    fn root_attribute_errors() {
        let errors = cco_parse_errors_for(hcl_documents! {"root_attr = 1"});
        assert_eq!(errors.issues.as_slice(), &[Issue::RootAttribute(0)]);
    }

    #[test]
    fn unknown_block_type_errors() {
        let errors = cco_parse_errors_for(hcl_documents! {"unknown_block_type {}"});
        assert!(errors.issues.contains(&Issue::UnknownBlockType(0)));
    }

    #[test]
    fn data_label_missing() {
        let errors = cco_parse_errors_for(hcl_documents! {"data {}"});
        assert!(errors.issues.contains(&Issue::DataBlockLabelMissing(0)));
    }

    #[test]
    fn data_label_collision() {
        let errors = cco_parse_errors_for(hcl_documents! {"data one two {}\ndata one two {}"});
        assert!(errors.issues.contains(&Issue::DataBlockLabelCollision {
            existing: 0,
            new: 1
        }));
    }

    #[test]
    fn data_label_collision_sanitized() {
        // sanitation may change labels, for example, a single whitespace is replaced with _
        let errors = cco_parse_errors_for(hcl_documents! {"data one \" \" {}\ndata one _ {}"});
        assert!(errors.issues.contains(&Issue::DataBlockLabelCollision {
            existing: 0,
            new: 1
        }));
    }
}
