//! The command line.

use tools_core::application::Mode;
use tools_core::cli::{Arguments, UsageError};

fn parse(words: &[&str]) -> Result<Arguments, UsageError> {
    let command_line = std::iter::once("project-sync")
        .chain(words.iter().copied())
        .map(str::to_owned);
    Arguments::parse(command_line)
}

#[test]
fn no_argument_means_apply() {
    let arguments = parse(&[]).expect("no argument is valid");
    assert_eq!(arguments.mode(), Mode::Apply);
}

#[test]
fn dry_run_means_preview() {
    let arguments = parse(&["--dry-run"]).expect("--dry-run is valid");
    assert_eq!(arguments.mode(), Mode::Preview);
}

#[test]
fn an_unknown_flag_is_a_usage_error_naming_it() {
    let failure = parse(&["--force"]).expect_err("--force is unknown");
    assert!(failure.to_string().contains("--force"));
    assert_eq!(failure.exit_code(), 2);
    let extra = parse(&["--dry-run", "now"]).expect_err("a second argument is unknown");
    assert!(extra.to_string().contains("now"));
}

#[test]
fn help_is_not_an_error_and_exits_with_zero() {
    let help = parse(&["--help"]).expect_err("--help stops the run");
    assert_eq!(help.exit_code(), 0);
    assert!(help.to_string().contains("--dry-run"));
}
