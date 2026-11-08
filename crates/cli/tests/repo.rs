//! The command line tests for `tools repo check|sync`.
#![cfg(feature = "repo")]

use tools::cli::{Invocation, UsageError, parse as parse_command_line};
use tools::repo::cli::Arguments;
use tools_core::application::RepoSyncMode;

fn parse(words: &[&str]) -> Result<Arguments, UsageError> {
    let command_line = ["tools", "repo"]
        .into_iter()
        .chain(words.iter().copied())
        .map(str::to_owned);
    match parse_command_line(command_line)? {
        Invocation::Repo(arguments) => Ok(arguments),
        _ => unreachable!("a repo command line parses to the repo group"),
    }
}

#[test]
fn a_verb_is_required() {
    let failure = parse(&[]).expect_err("a bare group is not a run");
    assert_eq!(failure.exit_code(), 2);
}

#[test]
fn check_means_check_mode() {
    let arguments = parse(&["check"]).expect("check is valid");
    assert_eq!(arguments.mode(), RepoSyncMode::Check);
}

#[test]
fn sync_means_apply_mode() {
    let arguments = parse(&["sync"]).expect("sync is valid");
    assert_eq!(arguments.mode(), RepoSyncMode::Apply);
}

#[test]
fn dry_run_is_gone() {
    let failure = parse(&["sync", "--dry-run"]).expect_err("--dry-run no longer exists");
    assert_eq!(failure.exit_code(), 2);
    assert!(failure.to_string().contains("--dry-run"));
}

#[test]
fn an_unknown_flag_is_a_usage_error_naming_it() {
    let failure = parse(&["sync", "--unknown-flag"]).expect_err("unknown flag fails");
    assert!(failure.to_string().contains("--unknown-flag"));
    assert_eq!(failure.exit_code(), 2);
}

#[test]
fn help_is_not_an_error_and_exits_with_zero() {
    let help = parse(&["check", "--help"]).expect_err("--help stops the run");
    assert_eq!(help.exit_code(), 0);
    assert!(help.to_string().contains("tools repo check"));
}

#[test]
fn accepts_entity_filtering_flags_on_both_verbs() {
    for verb in ["check", "sync"] {
        assert!(parse(&[verb, "--settings"]).is_ok());
        assert!(parse(&[verb, "--labels"]).is_ok());
        assert!(parse(&[verb, "--milestones"]).is_ok());
        assert!(parse(&[verb, "--project"]).is_ok());
        assert!(parse(&[verb, "--settings", "--labels", "--milestones", "--project"]).is_ok());
    }
}

#[test]
fn shared_flags_parse_before_or_after_the_verb() {
    assert!(parse(&["--config", "tools.toml", "--repo", "web", "check"]).is_ok());
    assert!(
        parse(&[
            "check",
            "--config",
            "tools.toml",
            "--repo",
            "web",
            "--org",
            "acme"
        ])
        .is_ok()
    );
}

#[test]
fn accepts_repository_target_and_config_overrides() {
    let arguments = parse(&["sync", "--owner", "my-org", "--repo", "my-repo"])
        .expect("target overrides are valid");
    assert_eq!(arguments.owner(), Some("my-org"));
    assert_eq!(arguments.repos(), &["my-repo"]);
    assert!(parse(&["sync", "--config", "custom-tools.toml"]).is_ok());
}

#[test]
fn accepts_multiple_repo_arguments() {
    let arguments =
        parse(&["sync", "--repo", "repo-a", "--repo", "repo-b"]).expect("multiple --repo is valid");
    assert_eq!(arguments.repos(), &["repo-a", "repo-b"]);
}

#[test]
fn accepts_org_and_multiple_repos() {
    let arguments = parse(&[
        "sync", "--org", "my-org", "--repo", "repo-a", "--repo", "repo-b",
    ])
    .expect("org and multiple repos are valid");
    assert_eq!(arguments.owner(), Some("my-org"));
    assert_eq!(arguments.repos(), &["repo-a", "repo-b"]);
}
