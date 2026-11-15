//! `tools actions check`: the command line and a run that needs no GitHub.
#![cfg(feature = "actions")]

use tools::cli::{Invocation, parse};

fn invocation(words: &[&str]) -> Invocation {
    let line = ["tools", "actions"]
        .into_iter()
        .chain(words.iter().copied())
        .map(str::to_owned);
    parse(line).expect("parses")
}

#[test]
fn check_scans_workflows_and_composite_actions_by_default() {
    let Invocation::Actions(arguments) = invocation(&["check"]) else {
        unreachable!("an actions command line parses to the actions group");
    };
    assert_eq!(arguments.directories().len(), 2);
    assert!(!arguments.explicit());
    assert!(arguments.config().is_none());
}

#[test]
fn check_takes_a_directory_and_the_global_config() {
    let Invocation::Actions(arguments) =
        invocation(&["check", "--workflows", "w", "--config", "t.toml"])
    else {
        unreachable!("an actions command line parses to the actions group");
    };
    assert_eq!(arguments.directories(), [std::path::PathBuf::from("w")]);
    assert!(arguments.explicit());
    assert_eq!(arguments.config(), Some(std::path::Path::new("t.toml")));
}

#[test]
fn a_file_without_rules_checks_nothing_and_succeeds() {
    let directory = std::env::temp_dir().join(format!("tools-actions-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("dir");
    let config = directory.join("tools.toml");
    std::fs::write(
        &config,
        "[github.repository]\nowner = \"o\"\nname = \"n\"\n",
    )
    .expect("write");
    let Invocation::Actions(arguments) = invocation(&[
        "check",
        "--config",
        config.to_str().expect("utf8"),
        "--workflows",
        "does-not-exist",
    ]) else {
        unreachable!("an actions command line parses to the actions group");
    };
    let exit = tools::run(Invocation::Actions(arguments)).expect("runs");
    assert_eq!(exit.code, 0);
    assert!(exit.text.contains("nothing to check"), "{}", exit.text);
    std::fs::remove_dir_all(&directory).expect("cleanup");
}
