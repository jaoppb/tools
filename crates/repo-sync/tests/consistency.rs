//! The real templates and docs of this repository agree with its `tools.toml`.

use std::path::Path;

use repo_sync::consistency;
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
    assert_eq!(
        consistency::check(&root, &names, config.github.issue_templates.as_ref()),
        Vec::<String>::new()
    );
}
