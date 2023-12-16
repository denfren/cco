//! Snapshot tests
//!
//! Loads each *.hcl file in /tests/ individually and compares if the
//! output of expression `test` changes.

#[test]
fn snapshots() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("CCO_LOG"))
        .with_writer(std::io::stderr)
        .init();

    insta::glob!("*.hcl", |path| {
        let mut documents = cco::hcl_documents::HclDocuments::default();
        let reader = std::fs::read_to_string(path).unwrap();
        documents.insert(
            hcl_edit::parser::parse_body(&reader).unwrap(),
            Some(path.to_owned()),
        );
        let documents =
            cco::cco_document::CcoDocument::new(&documents).expect("must be valid cco document");

        let rendered = documents
            .evaluate_in_context(hcl::Variable::unchecked("test").into())
            .expect("valid value");

        insta::assert_yaml_snapshot!(rendered);
    });
}
