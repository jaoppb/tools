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
    assert!(arguments.directories().is_none());
    assert!(arguments.config().is_none());
}

#[test]
fn check_takes_a_directory_and_the_global_config() {
    let Invocation::Actions(arguments) =
        invocation(&["check", "--workflows", "w", "--config", "t.toml"])
    else {
        unreachable!("an actions command line parses to the actions group");
    };
    assert_eq!(
        arguments.directories(),
        Some([std::path::PathBuf::from("w")].as_slice())
    );
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

#[test]
fn rules_without_any_workflow_file_fail_instead_of_passing() {
    let directory =
        std::env::temp_dir().join(format!("tools-actions-empty-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("dir");
    let config = directory.join("tools.toml");
    std::fs::write(
        &config,
        "[github.repository]\nowner = \"o\"\nname = \"n\"\n[actions]\nlatest_major = true\n",
    )
    .expect("write");
    let line = [
        "check",
        "--config",
        config.to_str().expect("utf8"),
        "--workflows",
        directory.to_str().expect("utf8"),
    ];
    let exit = tools::run(invocation(&line)).expect_err("nothing was scanned");
    assert_eq!(exit.code, 2);
    assert!(exit.text.contains("no workflow file"), "{}", exit.text);
    std::fs::remove_dir_all(&directory).expect("cleanup");
}

#[test]
fn the_default_directories_sit_next_to_the_given_config_not_the_working_directory() {
    // `cargo test` runs from `crates/cli`, inside this repository's own checkout, so a scan
    // rooted at the working directory would read this repository's `.github` instead.
    let directory = std::env::temp_dir().join(format!("tools-actions-root-{}", std::process::id()));
    let workflows = directory.join(".github").join("workflows");
    std::fs::create_dir_all(&workflows).expect("dir");
    let config = directory.join("tools.toml");
    std::fs::write(
        &config,
        "[github.repository]\nowner = \"o\"\nname = \"n\"\n[actions]\ndeny = [\"actions/*\"]\n",
    )
    .expect("write");
    let workflow = workflows.join("ci.yml");
    std::fs::write(
        &workflow,
        "jobs:\n  build:\n    steps:\n      - uses: actions/checkout@v4\n",
    )
    .expect("write");
    let exit = tools::run(invocation(&[
        "check",
        "--config",
        config.to_str().expect("utf8"),
    ]))
    .expect_err("the denied reference is found");
    std::fs::remove_dir_all(&directory).expect("cleanup");
    assert_eq!(exit.code, 1, "{}", exit.text);
    assert!(
        exit.text.contains(&*workflow.to_string_lossy()),
        "{}",
        exit.text
    );
}
