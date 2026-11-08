//! The command line tests for `tools project check|sync`.
#![cfg(feature = "project")]

use tools::cli::{Invocation, UsageError, parse as parse_command_line};
use tools::project::cli::Arguments;
use tools_core::application::Mode;

fn parse(words: &[&str]) -> Result<Arguments, UsageError> {
    let command_line = ["tools", "project"]
        .into_iter()
        .chain(words.iter().copied())
        .map(str::to_owned);
    match parse_command_line(command_line)? {
        Invocation::Project(arguments) => Ok(arguments),
        _ => unreachable!("a project command line parses to the project group"),
    }
}

#[test]
fn a_verb_is_required() {
    let failure = parse(&[]).expect_err("a bare group is not a run");
    assert_eq!(failure.exit_code(), 2);
}

#[test]
fn sync_means_apply() {
    let arguments = parse(&["sync"]).expect("sync is valid");
    assert_eq!(arguments.mode(), Mode::Apply);
}

#[test]
fn check_means_preview() {
    let arguments = parse(&["check"]).expect("check is valid");
    assert_eq!(arguments.mode(), Mode::Preview);
}

#[test]
fn an_unknown_flag_is_a_usage_error_naming_it() {
    let failure = parse(&["sync", "--force"]).expect_err("--force is unknown");
    assert!(failure.to_string().contains("--force"));
    assert_eq!(failure.exit_code(), 2);
    let dry_run = parse(&["sync", "--dry-run"]).expect_err("--dry-run is gone");
    assert!(dry_run.to_string().contains("--dry-run"));
    let extra = parse(&["check", "now"]).expect_err("a second argument is unknown");
    assert!(extra.to_string().contains("now"));
}

#[test]
fn help_is_not_an_error_and_exits_with_zero() {
    let help = parse(&["check", "--help"]).expect_err("--help stops the run");
    assert_eq!(help.exit_code(), 0);
    assert!(help.to_string().contains("tools project check"));
}

#[test]
fn accepts_target_overrides_and_config() {
    assert!(
        parse(&[
            "sync",
            "--owner",
            "jaoppb",
            "--repo",
            "tools",
            "--project",
            "1"
        ])
        .is_ok()
    );
    assert!(parse(&["check", "--config", "tools.toml"]).is_ok());
    let config = config_file(
        "multi",
        "[github.repository]\nowner = \"jaoppb\"\nname = \"app\"\n\n[workflow]\nextends = \"builtin:tools\"\n",
    );
    let multi = parse(&[
        "check",
        "--config",
        config.to_str().expect("a utf-8 path"),
        "--owner",
        "jaoppb",
        "--repo",
        "app",
        "--repo",
        "web",
        "--project",
        "1",
    ])
    .expect("multiple repo flags parse");
    let (target, workflow) = multi.resolve().expect("resolves multi-repo target");
    assert_eq!(target.repos().len(), 2);
    assert_eq!(workflow.statuses().iter().count(), 5);
}

fn config_file(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tools-57-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let path = dir.join("tools.toml");
    std::fs::write(&path, text).expect("the config is written");
    path
}

#[test]
fn without_a_workflow_block_the_run_says_what_to_add() {
    let config = config_file(
        "bare",
        "[github.repository]\nowner = \"jaoppb\"\nname = \"app\"\n\n[github.project]\nnumber = 1\n",
    );
    let arguments = parse(&[
        "check",
        "--config",
        config.to_str().expect("a utf-8 path"),
        "--owner",
        "jaoppb",
        "--repo",
        "app",
        "--project",
        "1",
    ])
    .expect("the flags parse");
    let failure = arguments.resolve().expect_err("no workflow");
    let text = failure.to_string();
    assert!(text.contains("no [workflow] block"), "{text}");
    assert!(text.contains("builtin:cosmo"), "{text}");
}

#[test]
fn a_missing_project_is_reported_before_a_missing_workflow() {
    let config = config_file(
        "noproject",
        "[github.repository]\nowner = \"jaoppb\"\nname = \"app\"\n",
    );
    let arguments = parse(&["check", "--config", config.to_str().expect("a utf-8 path")])
        .expect("the flags parse");
    let text = arguments.resolve().expect_err("no project").to_string();
    assert!(text.contains("project number is required"), "{text}");
}

#[test]
fn explain_names_an_issue_on_check_only() {
    let arguments = parse(&["check", "--explain", "repo#57"]).expect("check explains");
    assert_eq!(arguments.explain(), Some("repo#57"));
    assert_eq!(parse(&["check"]).expect("valid").explain(), None);
    let failure = parse(&["sync", "--explain", "57"]).expect_err("sync does not explain");
    assert!(failure.to_string().contains("--explain"));
}
