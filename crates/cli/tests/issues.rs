//! The command line tests for `tools issues check|migrate`.
#![cfg(feature = "issues")]

use tools::cli::{Invocation, UsageError, parse as parse_command_line};
use tools::issues::cli::Arguments;

fn parse(words: &[&str]) -> Result<Arguments, UsageError> {
    let command_line = ["tools", "issues"]
        .into_iter()
        .chain(words.iter().copied())
        .map(str::to_owned);
    match parse_command_line(command_line)? {
        Invocation::Issues(arguments) => Ok(arguments),
        _ => unreachable!("an issues command line parses to the issues group"),
    }
}

#[test]
fn a_verb_is_required() {
    let failure = parse(&[]).expect_err("a bare group is not a run");
    assert_eq!(failure.exit_code(), 2);
}

#[test]
fn check_is_not_migrate() {
    assert!(!parse(&["check"]).expect("check parses").is_migrate());
    assert!(parse(&["migrate"]).expect("migrate parses").is_migrate());
}

#[test]
fn rejects_an_unknown_argument_with_exit_code_two() {
    let failure = parse(&["check", "--invalid-flag"]).expect_err("unknown flag fails");
    assert_eq!(failure.exit_code(), 2);
}

#[test]
fn shows_the_usage_and_exits_with_zero_for_help() {
    let help = parse(&["check", "--help"]).expect_err("help is not a run");
    assert_eq!(help.exit_code(), 0);
    assert!(help.to_string().contains("tools issues check"));
}

#[test]
fn accepts_rule_flags_and_overrides() {
    assert!(parse(&["check", "--no-parent", "--no-membership"]).is_ok());
    assert!(
        parse(&[
            "check",
            "--owner",
            "jaoppb",
            "--repo",
            "tools",
            "--project",
            "1"
        ])
        .is_ok()
    );
    assert!(parse(&["check", "--config", "custom-tools.toml"]).is_ok());
    let multi = parse(&[
        "check", "--owner", "jaoppb", "--repo", "app", "--repo", "web",
    ])
    .expect("multiple repo flags parse");
    let resolved = multi.resolve().expect("resolves multi-repo target");
    assert_eq!(resolved.target.repos().len(), 2);
}

#[test]
fn accepts_migrate_and_org_flags() {
    parse(&["migrate"]).expect("migrate parses");

    let org = parse(&["--org", "gatuno-manga", "--repo", "api", "check"]).expect("org parses");
    let resolved = org.resolve().expect("resolves org target");
    assert_eq!(resolved.target.owner(), "gatuno-manga");
}

#[test]
fn apply_is_gone_and_migrate_takes_no_audit_flags() {
    for words in [&["migrate", "--apply"][..], &["migrate", "--no-parent"]] {
        let failure = parse(words).expect_err("flag does not belong to the verb");
        assert_eq!(failure.exit_code(), 2);
    }
}

#[test]
fn default_label_flags_are_gone() {
    for words in [
        &["migrate", "--default-area", "core"][..],
        &["migrate", "--default-type", "chore"],
    ] {
        let failure = parse(words).expect_err("the default label flags no longer exist");
        assert_eq!(failure.exit_code(), 2);
    }
}
