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
    let multi = parse(&["--owner", "jaoppb", "--repo", "app", "--repo", "web"])
        .expect("multiple repo flags parse");
    let resolved = multi.resolve().expect("resolves multi-repo target");
    assert_eq!(resolved.target.repos().len(), 2);
}

#[test]
fn accepts_migration_flags() {
    let migrate = parse(&["--migrate"]).expect("migrate parses");
    assert!(migrate.is_migrate());
    assert!(!migrate.is_apply());
    assert_eq!(migrate.default_area(), None);
    assert_eq!(migrate.default_type(), None);

    let apply = parse(&[
        "--migrate",
        "--apply",
        "--default-area",
        "core",
        "--default-type",
        "chore",
    ])
    .expect("apply parses");
    assert!(apply.is_migrate());
    assert!(apply.is_apply());
    assert_eq!(apply.default_area(), Some("core"));
    assert_eq!(apply.default_type(), Some("type:chore"));

    let prefixed =
        parse(&["--migrate", "--default-type", "type:feat"]).expect("prefixed type parses");
    assert_eq!(prefixed.default_type(), Some("type:feat"));

    let org = parse(&["--org", "gatuno-manga", "--repo", "api"]).expect("org parses");
    let resolved = org.resolve().expect("resolves org target");
    assert_eq!(resolved.target.owner(), "gatuno-manga");
}

#[test]
fn rejects_apply_or_default_area_or_default_type_without_migrate() {
    let failure1 = parse(&["--apply"]).expect_err("apply without migrate fails");
    assert_eq!(failure1.exit_code(), 2);

    let failure2 =
        parse(&["--default-area", "core"]).expect_err("default-area without migrate fails");
    assert_eq!(failure2.exit_code(), 2);

    let failure3 =
        parse(&["--default-type", "chore"]).expect_err("default-type without migrate fails");
    assert_eq!(failure3.exit_code(), 2);
}

#[test]
fn rejects_invalid_default_type() {
    let failure =
        parse(&["--migrate", "--default-type", "invalid"]).expect_err("invalid default type fails");
    assert_eq!(failure.exit_code(), 2);
    assert!(
        failure
            .to_string()
            .contains("invalid default type 'invalid'")
    );
}
