//! collection of hcl documents ([Body] and path to source file)
//!
//! [HclDocuments] tracks
//! - the source path
//! - the root blocks
//! - the root attributes
//! and defines a numeric index for each. Once added those indices are stable (removal is not possible)
use hcl_edit::structure::{Attribute, Block, Body, Structure};
use std::path::Path;

#[derive(Default, Debug)]
pub struct HclDocuments {
    sources: Vec<Source>,
    root_attributes: Vec<(usize, Attribute)>,
    root_blocks: Vec<(usize, Block)>,
}

impl HclDocuments {
    /// Inserts and indexes an hcl document
    pub fn insert(&mut self, document: Body, path: impl Into<Option<std::path::PathBuf>>) {
        let source_index = self.sources.len();
        self.sources.push(path.into());

        for structure in document.into_iter() {
            match structure {
                Structure::Block(block) => self.root_blocks.push((source_index, block)),
                Structure::Attribute(attribute) => {
                    self.root_attributes.push((source_index, attribute))
                }
            }
        }
    }

    pub fn get_attribute(&self, index: usize) -> SourceAttribute {
        let (source_index, block) = &self.root_attributes[index];
        (index, &self.sources[*source_index], block)
    }

    pub fn attributes(&self) -> impl Iterator<Item = SourceAttribute> {
        self.root_attributes
            .iter()
            .enumerate()
            .map(|(index, (source_index, attribute))| {
                (index, &self.sources[*source_index], attribute)
            })
    }

    pub fn get_block(&self, index: usize) -> SourceBlock {
        let (source_index, block) = &self.root_blocks[index];
        (index, &self.sources[*source_index], block)
    }

    pub fn blocks(&self) -> impl Iterator<Item = SourceBlock> {
        self.root_blocks
            .iter()
            .enumerate()
            .map(|(index, (source_index, block))| (index, &self.sources[*source_index], block))
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
}

impl HclDocuments {
    pub fn load_file(&mut self, file_path: &Path) -> Result<(), LoadError> {
        let file_path = file_path.canonicalize()?;
        tracing::info!(path=%file_path.display(), "loading file");

        let file_contents = std::fs::read_to_string(&file_path)?;
        let body = hcl_edit::parser::parse_body(&file_contents)?;

        self.insert(body, Some(file_path));
        Ok(())
    }

    pub fn load_directory(&mut self, dir_path: &Path) -> Result<(), LoadError> {
        let mut any_files_loaded = false;

        let read_dir = std::fs::read_dir(dir_path)?;
        for dir_entry in read_dir {
            let dir_entry = dir_entry?;
            if !dir_entry.file_type()?.is_file() {
                continue;
            }

            let is_cco_hcl_file = dir_entry.file_name().to_string_lossy().ends_with("cco.hcl");
            if !is_cco_hcl_file {
                continue;
            }

            let file_path = dir_entry.path();
            self.load_file(&file_path)?;
            any_files_loaded = true;
        }

        if !any_files_loaded {
            return Err(LoadError::NoFilesFound);
        }

        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadError {
    #[error("No files found in directory")]
    NoFilesFound,
    #[error("IO error")]
    IoError(#[from] std::io::Error),
    #[error("Unable to parse hcl file")]
    HclParseFailed(#[from] hcl_edit::parser::Error),
}

impl From<Body> for HclDocuments {
    fn from(value: Body) -> Self {
        let mut tree = HclDocuments::default();
        tree.insert(value, None);
        tree
    }
}

/// Utility macro to create [HclDocuments]
///
/// Create from a single document
/// ```
/// # use cco::hcl_documents;
/// hcl_documents!("attribute = 42");
/// ```
///
/// Create from multiple documents (path required)
/// ```
/// # use cco::hcl_documents;
/// hcl_documents! {
///   "one.hcl" => "attribute_one = 1",
///   "two.hcl" => "attribute_two = 2"
/// };
/// ```
///
/// # Panic
/// Panics on invalid input
///
/// ```should_panic
/// # use cco::hcl_documents;
/// hcl_documents!("not = valid = hcl");
/// ```
#[macro_export]
macro_rules! hcl_documents {
    // single document without source
    { $expr:expr } => {
        $crate::hcl_documents::HclDocuments::from(hcl_edit::parser::parse_body($expr).expect("body must parse"))
    };
    // multi document with sources
    { $($source:expr => $expr:expr),+ } => {
        let mut docs = $crate::hcl_documents::HclDocuments::default();
        $(
            docs.insert(hcl_edit::parser::parse_body($expr).expect("body must parse"), Some($source.into()));
        )+

        docs
    };
}

pub type Source = Option<std::path::PathBuf>;
pub type SourceAttribute<'a> = (usize, &'a Source, &'a Attribute);
pub type SourceBlock<'a> = (usize, &'a Source, &'a Block);

#[cfg(test)]
pub(crate) mod test {
    #[test]
    fn iterators() {
        let hcl_documents = hcl_documents! {r#"
        attr_1 = 1
        one two {}
        three four five {}
        attr_2 = 2
        attr_3 = 3
        "#};

        assert_eq!(hcl_documents.attributes().count(), 3);
        assert_eq!(hcl_documents.blocks().count(), 2);
    }
}
