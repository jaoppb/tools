//! The command line tests for `issues-check`.

use issues_check::cli::{Arguments, UsageError};

fn parse(words: &[&str]) -> Result<Arguments, UsageError> {
    let command_line = std::iter::once("issues-check")
        .chain(words.iter().copied())
        .map(str::to_owned);
    Arguments::parse(command_line)
}

#[test]
fn accepts_no_arguments() {
    assert!(parse(&[]).is_ok());
}

#[test]
fn rejects_an_unknown_argument_with_exit_code_two() {
    let failure = parse(&["--invalid-flag"]).expect_err("unknown flag fails");
    assert_eq!(failure.exit_code(), 2);
}

#[test]
fn shows_the_usage_and_exits_with_zero_for_help() {
    let help = parse(&["--help"]).expect_err("help is not a run");
    assert_eq!(help.exit_code(), 0);
    assert!(help.to_string().contains("issues-check"));
}

#[test]
fn accepts_rule_flags_and_overrides() {
    assert!(parse(&["--no-parent", "--no-membership"]).is_ok());
    assert!(parse(&["--owner", "jaoppb", "--repo", "tools", "--project", "1"]).is_ok());
    assert!(parse(&["--config", "custom-tools.toml"]).is_ok());
}
