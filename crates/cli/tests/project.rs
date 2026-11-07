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
        #[cfg(any(feature = "issues", feature = "repo"))]
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
    let multi = parse(&[
        "check",
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
    let (target, _, _) = multi.resolve().expect("resolves multi-repo target");
    assert_eq!(target.repos().len(), 2);
}
