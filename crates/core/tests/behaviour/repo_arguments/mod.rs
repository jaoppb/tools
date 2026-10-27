//! The command line of `repo-sync`.

use tools_core::application::RepoSyncMode;
use tools_core::cli::{RepoArguments, RepoUsageError};
use tools_core::domain::RepoEntities;

fn parse(words: &[&str]) -> Result<RepoArguments, RepoUsageError> {
    let command_line = std::iter::once("repo-sync")
        .chain(words.iter().copied())
        .map(str::to_owned);
    RepoArguments::parse(command_line)
}

#[test]
fn no_argument_means_apply_and_all_entities() {
    let arguments = parse(&[]).expect("no argument is valid");
    assert_eq!(arguments.mode(), RepoSyncMode::Apply);
    assert_eq!(arguments.entities(), RepoEntities::ALL);
}

#[test]
fn check_means_check_mode() {
    let arguments = parse(&["--check"]).expect("--check is valid");
    assert_eq!(arguments.mode(), RepoSyncMode::Check);
    assert_eq!(arguments.entities(), RepoEntities::ALL);
}

#[test]
fn dry_run_means_preview_mode() {
    let arguments = parse(&["--dry-run"]).expect("--dry-run is valid");
    assert_eq!(arguments.mode(), RepoSyncMode::Preview);
    assert_eq!(arguments.entities(), RepoEntities::ALL);
}

#[test]
fn check_and_dry_run_conflict() {
    let failure = parse(&["--check", "--dry-run"]).expect_err("mutually exclusive flags fail");
    assert_eq!(failure.exit_code(), 2);
    assert!(failure.to_string().contains("--dry-run"));
}

#[test]
fn entity_flags_select_individual_entities() {
    let labels_only = parse(&["--labels"]).expect("--labels is valid");
    assert_eq!(
        labels_only.entities(),
        RepoEntities {
            settings: false,
            labels: true,
            milestones: false,
        }
    );

    let multiple = parse(&["--settings", "--milestones"]).expect("multiple flags are valid");
    assert_eq!(
        multiple.entities(),
        RepoEntities {
            settings: true,
            labels: false,
            milestones: true,
        }
    );
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
