//! # cco - cascading configuration
//!
//! For a user guide and material related to CLI usage see <https://github.com/denfren/cco>
//!
//! ## Introduction for developers
//!
//! Read this to understand how `cco` works internally.
//!
//! ### HCL Terms
//!
//! Quick introduction to terms used to describe elements of HCL documents.
//!
//! In hcl terms...
//! - a file gets parsed as a `body`
//! - ...which is just a list of `structures`
//! - ...where there are two kinds:
//!   - `attribute`: a "key = value" pair
//!   - or `block`:
//!     - 1 `identifier`
//!     - followed by 0 or more `labels`
//!     - and a `body` enclosed in `{` and `}`
//!
//! This is a valid hcl file:
//! ```hcl
//! # single line comments work like this
//! // ...or like this
//!
//! /* multi-line
//! comments
//! also work */
//!
//! an_attribute_key = "and its value"
//!
//! this_is_a_block_identifier this_is_a_label "another label, but in quotes" {
//!   attribute_inside_the_blocks_body = 42
//!
//!   an_empty_block_with_no_labels {}
//! }
//!
//! ```
//!
//! ### Loading files
//!
//! An `.hcl` document is parsed as a `body` ([hcl_edit::structure::Body]). `cco` can be used with multiple documents.
//! We use [hcl_documents::HclDocuments] to store all (root) Attributes and Blocks of all documents and track their
//! original source path. The path is stored so we can point to it when returning user friendly error messages.
//! At this point the loaded documents/files only have to be valid HCL to be accepted.
//!
//! [hcl_documents::HclDocuments] also assigns each [hcl_edit::structure::Attribute] and [hcl_edit::structure::Block] an
//! index which is used to uniquely identify them.
//!
//! ### Parsing
//!
//! see [cco_document::CcoDocument::new]
//!
//! The next step applies most extra rules that are specific to `cco` configuration management.
//!
//! - block labels are normalized (according to [Identifier::sanitized])
//! - block identifiers ([hcl_edit::Ident]) are checked for validity
//! - block label collision (after normalization)
//!
//! ### Transform into a list of addressable elements
//!
//! This part currently also happens in [cco_document::CcoDocument::new]
//!
//! We traverse the hcl tree and find all leaf attributes and blocks that can be addressed.
//!
//! **Example**
//!
//! ```hcl
//! data block one {
//!     attribute = 1
//! }
//!
//! data block two {
//!     attribute = {
//!         value = 2
//!     }
//! }
//! ```
//!
//! **Addressables**
//!
//! | **path** ([Vec]<[Identifier]>) | **substition** ([Identifier]) | **expression** ([Expression]) |
//! |-----------------------|---------------|--------------------------------------|
//! | `block.one.attribute` | `cco__b_o_a`  | `1`                                  |
//! | `block.one`           | `cco__b_o`    | `{ attribute = cco__b_o_a }`         |
//! | `block.two.attribute` | `cco__b_t_a`  | `{ value = 2 }`                      |
//! | `block.two`           | `cco__b_t`    | `{ attribute = cco__b_t_a }`         |
//! | `block`               | `cco__b`      | `{ one = cco__b_o, two = cco__b_t }` |
//!
//! _Note: Actual substitution identifiers have a different format._
//!
//! ### Evaluation
//!
//! We use [hcl::eval] to evaluate the hcl expressions. The [hcl::eval::Context] expects us to provide variables that it
//! can use to resolve variables it encounters.
//! Currently there is no way to respond to Traversals dynamically so we just change the problem.
//!
//! Before passing an expression to evaluate to [hcl::eval::Context] we walk the expression tree to find all
//! [hcl::expr::Traversal]s (such as `a.b.c`, `a[*]`, ...) and replace them with the most specific addressable-substitution
//! we know about.
//!
//! Given our previous example an expression of `block.one.attribute` would be rewritten to `cco__b_o_a`.
//!
//! After rewriting we try to resolve the expression. When successful then we're done.
//!
//! If not, then we have to check if the missing/unknown variable starts with `cco__`, our internal marker.
//! If so, then we try to parse this dependency first before coming back to our initial expression.
//! Also we do check if there is a dependency loop so we can abort and report.
//!
//! ### Output
//!
//! Once the expression is evaluated we parse it as a [value::Value] which in turn gets serialized via [serde].
//!
pub mod cco_document;
pub mod hcl_documents;
mod util;
pub mod value;
mod visit;
