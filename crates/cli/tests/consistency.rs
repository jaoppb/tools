//! The real templates and docs of this repository agree with its `tools.toml`.
#![cfg(feature = "repo")]

use std::path::Path;

use tools::repo::consistency::{self, LocalFiles};
use tools_core::config::load_from_path;

#[test]
fn this_repository_templates_and_docs_match_tools_toml() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = load_from_path(&root.join("tools.toml")).expect("tools.toml loads");
    let names: Vec<String> = config
        .spec_for("tools")
        .expect("spec")
        .labels
        .expect("labels declared")
        .items
        .into_iter()
        .map(|label| label.name)
        .collect();
    let files = LocalFiles {
        labels: names,
        templates: config.issue_templates().cloned(),
        docs: config.label_docs().to_vec(),
    };
    assert!(!files.docs.is_empty(), "this repository declares its docs");
    assert_eq!(consistency::check(&root, &files), Vec::<String>::new());
}
