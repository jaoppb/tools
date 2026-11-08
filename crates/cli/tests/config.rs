//! `tools config check|migrate`: the command line and the runs against a file.

use std::path::{Path, PathBuf};

use tools::cli::{Invocation, UsageError, parse as parse_command_line};
use tools::config::cli::Arguments;
use tools::config::run;

const HEAD: &str = "[github.repository]\nowner = 'o'\nname = 'n'\n";

fn parse(words: &[&str]) -> Result<Arguments, UsageError> {
    let command_line = ["tools", "config"]
        .into_iter()
        .chain(words.iter().copied())
        .map(str::to_owned);
    match parse_command_line(command_line)? {
        Invocation::Config(arguments) => Ok(arguments),
        #[cfg(any(feature = "issues", feature = "repo", feature = "project"))]
        _ => unreachable!("a config command line parses to the config group"),
    }
}

/// A `tools.toml` in a directory of its own.
fn file(name: &str, contents: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tools-config-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("tools.toml");
    std::fs::write(&path, contents).expect("write");
    path
}

fn go(verb: &[&str], path: &Path) -> (String, u8) {
    let mut words = vec!["--config", path.to_str().expect("utf-8 path")];
    words.extend_from_slice(verb);
    let arguments = parse(&words).expect("parses");
    match run(&arguments) {
        Ok(exit) | Err(exit) => (exit.text, exit.code),
    }
}

#[test]
fn a_verb_is_required_and_dry_run_is_for_migrate() {
    assert_eq!(parse(&[]).expect_err("bare group").exit_code(), 2);
    assert!(parse(&["migrate", "--dry-run"]).expect("parses").dry_run());
    assert!(!parse(&["check"]).expect("parses").is_migrate());
    assert_eq!(
        parse(&["check", "--dry-run"])
            .expect_err("only migrate")
            .exit_code(),
        2
    );
}

#[test]
fn check_exits_one_while_a_migration_is_pending_and_zero_after_it() {
    let path = file(
        "check",
        &format!("{HEAD}[github.issues.rules]\ncycle = true\n"),
    );
    let (text, code) = go(&["check"], &path);
    assert_eq!(code, 1);
    assert!(text.contains("tools config migrate"));
    let (_, migrated) = go(&["migrate"], &path);
    assert_eq!(migrated, 0);
    let (text, code) = go(&["check"], &path);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("deprecated"));
}

#[test]
fn a_string_area_prefix_is_pending_and_migrate_wraps_it() {
    let path = file(
        "area-prefix",
        &format!(
            "{HEAD}[workflow]\nextends = \"builtin:tools\"\n[workflow.params]\narea_prefix = \"area:\"\n"
        ),
    );
    let (text, code) = go(&["check"], &path);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("area_prefix"), "{text}");
    assert_eq!(go(&["migrate"], &path).1, 0);
    let (text, code) = go(&["check"], &path);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn check_exits_two_with_the_location_for_a_param_a_list_does_not_fix() {
    let path = file(
        "param-type",
        &format!("{HEAD}[workflow]\nextends = \"builtin:tools\"\n[workflow.params]\nstarted = 3\n"),
    );
    let (text, code) = go(&["check"], &path);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("line 7, column 11"), "{text}");
}

#[test]
fn check_exits_two_for_a_file_that_does_not_load() {
    let path = file(
        "bad",
        &format!("{HEAD}[workflow]\nextends = \"builtin:nope\"\n"),
    );
    assert_eq!(go(&["check"], &path).1, 2);
}

#[test]
fn migrate_writes_the_file_and_dry_run_does_not() {
    let original = format!("{HEAD}[github.issues.rules]\nmembership = false\nparent = false\n");
    let dry = file("dry", &original);
    let (text, code) = go(&["migrate", "--dry-run"], &dry);
    assert_eq!(code, 0);
    assert!(text.contains("builtin:tools"));
    assert_eq!(std::fs::read_to_string(&dry).expect("read"), original);
    let real = file("real", &original);
    assert_eq!(go(&["migrate"], &real).1, 0);
    assert!(
        std::fs::read_to_string(&real)
            .expect("read")
            .contains("extends = \"builtin:tools\"")
    );
}

#[test]
fn migrate_refuses_a_rename_with_exit_one_and_writes_nothing() {
    let original = format!("{HEAD}[github.project.status_mapping]\n'To do' = 'Backlog'\n");
    let path = file("rename", &original);
    let (text, code) = go(&["migrate"], &path);
    assert_eq!(code, 1);
    assert!(text.contains("#88"));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), original);
}

#[test]
fn migrate_without_old_keys_is_a_no_op() {
    let path = file("noop", HEAD);
    let (_, code) = go(&["migrate"], &path);
    assert_eq!(code, 0);
    assert_eq!(std::fs::read_to_string(&path).expect("read"), HEAD);
}

#[test]
fn a_missing_file_is_exit_two() {
    let path = std::env::temp_dir().join("tools-config-missing/tools.toml");
    assert_eq!(go(&["check"], &path).1, 2);
}
