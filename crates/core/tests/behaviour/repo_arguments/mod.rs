//! The command line of `repo-sync`.

use tools_core::application::RepoSyncMode;
use tools_core::cli::{RepoArguments, RepoUsageError};

fn parse(words: &[&str]) -> Result<RepoArguments, RepoUsageError> {
    let command_line = std::iter::once("repo-sync")
        .chain(words.iter().copied())
        .map(str::to_owned);
    RepoArguments::parse(command_line)
}

#[test]
fn no_argument_means_apply_mode() {
    let arguments = parse(&[]).expect("no argument is valid");
    assert_eq!(arguments.mode(), RepoSyncMode::Apply);
}

#[test]
fn check_means_check_mode() {
    let arguments = parse(&["--check"]).expect("--check is valid");
    assert_eq!(arguments.mode(), RepoSyncMode::Check);
}

#[test]
fn dry_run_means_preview_mode() {
    let arguments = parse(&["--dry-run"]).expect("--dry-run is valid");
    assert_eq!(arguments.mode(), RepoSyncMode::Preview);
}

#[test]
fn check_and_dry_run_conflict() {
    let failure = parse(&["--check", "--dry-run"]).expect_err("mutually exclusive flags fail");
    assert_eq!(failure.exit_code(), 2);
    assert!(failure.to_string().contains("--dry-run"));
}

#[test]
fn an_unknown_flag_is_a_usage_error_naming_it() {
    let failure = parse(&["--unknown-flag"]).expect_err("unknown flag fails");
    assert!(failure.to_string().contains("--unknown-flag"));
    assert_eq!(failure.exit_code(), 2);
}

#[test]
fn help_is_not_an_error_and_exits_with_zero() {
    let help = parse(&["--help"]).expect_err("--help stops the run");
    assert_eq!(help.exit_code(), 0);
    assert!(help.to_string().contains("repo-sync"));
}

#[test]
fn accepts_entity_filtering_flags() {
    assert!(parse(&["--settings"]).is_ok());
    assert!(parse(&["--labels"]).is_ok());
    assert!(parse(&["--milestones"]).is_ok());
    assert!(parse(&["--project"]).is_ok());
    assert!(parse(&["--settings", "--labels", "--milestones", "--project"]).is_ok());
}

#[test]
fn accepts_repository_target_and_config_overrides() {
    assert!(parse(&["--owner", "my-org", "--repo", "my-repo"]).is_ok());
    assert!(parse(&["--config", "custom-tools.toml"]).is_ok());
}
