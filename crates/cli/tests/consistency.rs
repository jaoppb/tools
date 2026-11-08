//! The real templates and docs of this repository agree with its `tools.toml`.
#![cfg(feature = "repo")]

use std::path::Path;

use tools::cli::{Invocation, parse};
use tools::repo::consistency;

#[test]
fn this_repository_templates_and_docs_match_tools_toml() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = root.join("tools.toml");
    let command_line = ["tools", "repo", "check", "--config"]
        .map(str::to_owned)
        .into_iter()
        .chain([config.display().to_string()]);
    let Ok(Invocation::Repo(arguments)) = parse(command_line) else {
        panic!("a repo check command line parses to the repo group");
    };
    let files = arguments
        .resolve()
        .expect("tools.toml resolves")
        .local_files
        .expect("a plain check of the configured repository runs the offline step");
    assert!(!files.docs.is_empty(), "this repository declares its docs");
    assert!(
        files.templates.is_some(),
        "this repository declares its templates"
    );
    assert_eq!(consistency::check(&root, &files), Vec::<String>::new());
}
