//! `tools changelog check`: the command line, the verdict and a run against a git repository.
#![cfg(feature = "changelog")]

use std::path::Path;
use std::process::Command;

use tools::changelog::{run, verdict};
use tools::cli::{Invocation, UsageError, parse as parse_command_line};

fn parse(words: &[&str]) -> Result<tools::changelog::cli::Arguments, UsageError> {
    let command_line = ["tools", "changelog"]
        .into_iter()
        .chain(words.iter().copied())
        .map(str::to_owned);
    match parse_command_line(command_line)? {
        Invocation::Changelog(arguments) => Ok(arguments),
        #[allow(unreachable_patterns)]
        _ => unreachable!("a changelog command line parses to the changelog group"),
    }
}

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn a_change_outside_the_watched_paths_needs_no_entry() {
    let exit = verdict(
        &names(&["README.md", "docs/a.md", "tools.toml"]),
        &[],
        "skip",
    );
    assert_eq!(exit.code, 0);
}

#[test]
fn every_watched_path_demands_an_entry() {
    for path in [
        "crates/cli/src/lib.rs",
        "action.yml",
        "scripts/bump-version.sh",
        ".github/workflows/ci.yml",
    ] {
        let exit = verdict(&names(&[path]), &[], "skip");
        assert_eq!(exit.code, 1, "{path}");
        assert!(exit.text.contains(path), "{}", exit.text);
        assert!(exit.text.contains("`skip`"), "{}", exit.text);
    }
}

#[test]
fn a_look_alike_path_is_not_watched() {
    let exit = verdict(
        &names(&[
            "action.yml.bak",
            "crates-notes.md",
            ".github/ISSUE_TEMPLATE/fix.yml",
            "docs/scripts/a",
        ]),
        &[],
        "skip",
    );
    assert_eq!(exit.code, 0, "{}", exit.text);
}

#[test]
fn touching_the_changelog_satisfies_the_check() {
    let exit = verdict(
        &names(&["crates/cli/src/lib.rs", "CHANGELOG.md"]),
        &[],
        "skip",
    );
    assert_eq!(exit.code, 0);
}

#[test]
fn only_the_root_changelog_counts() {
    let exit = verdict(
        &names(&["crates/cli/src/lib.rs", "docs/CHANGELOG.md"]),
        &[],
        "skip",
    );
    assert_eq!(exit.code, 1);
}

#[test]
fn the_skip_label_waives_the_entry() {
    let exit = verdict(
        &names(&["action.yml"]),
        &names(&["type:chore", "skip"]),
        "skip",
    );
    assert_eq!(exit.code, 0);
    assert_eq!(
        verdict(&names(&["action.yml"]), &names(&["skipped"]), "skip").code,
        1
    );
}

#[test]
fn the_command_line_reads_base_labels_and_skip_label() {
    let arguments = parse(&[
        "check",
        "--base",
        "origin/main",
        "--labels",
        "a, b,,c",
        "--skip-label",
        "skip",
    ])
    .expect("parses");
    assert_eq!(arguments.base(), "origin/main");
    assert_eq!(arguments.labels(), ["a", "b", "c"]);
    assert_eq!(arguments.skip_label(), "skip");
}

#[test]
fn the_command_line_needs_a_base_and_a_skip_label() {
    assert!(parse(&["check", "--skip-label", "skip"]).is_err());
    assert!(parse(&["check", "--base", "main"]).is_err());
    assert!(parse(&[]).is_err());
}

#[test]
fn labels_may_be_empty() {
    let arguments = parse(&[
        "check",
        "--base",
        "main",
        "--labels",
        "",
        "--skip-label",
        "skip",
    ])
    .expect("parses");
    assert!(arguments.labels().is_empty());
}

/// Runs git in `dir` with a fixed identity and no global configuration.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

#[test]
fn run_diffs_the_branch_against_its_merge_base() {
    let dir = std::env::temp_dir().join(format!("tools-changelog-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("crates")).expect("temp dir");
    git(&dir, &["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(dir.join("README.md"), "x").expect("write");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "--quiet", "-m", "base"]);
    git(&dir, &["switch", "--quiet", "-c", "feature"]);
    std::fs::write(dir.join("crates/a.rs"), "x").expect("write");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "--quiet", "-m", "code"]);

    let in_dir = |words: &[&str]| {
        let previous = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(&dir).expect("enter");
        let result = run(&parse(words).expect("parses"));
        std::env::set_current_dir(previous).expect("leave");
        result
    };

    let failing = in_dir(&["check", "--base", "main", "--skip-label", "skip"]).expect("runs");
    assert_eq!(failing.code, 1, "{}", failing.text);
    let waived = in_dir(&[
        "check",
        "--base",
        "main",
        "--labels",
        "skip",
        "--skip-label",
        "skip",
    ])
    .expect("runs");
    assert_eq!(waived.code, 0);

    std::fs::write(dir.join("CHANGELOG.md"), "x").expect("write");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "--quiet", "-m", "changelog"]);
    let recorded = in_dir(&["check", "--base", "main", "--skip-label", "skip"]).expect("runs");
    assert_eq!(recorded.code, 0, "{}", recorded.text);

    let unknown =
        in_dir(&["check", "--base", "nowhere", "--skip-label", "skip"]).expect_err("fails");
    assert_eq!(unknown.code, 2);
    let option =
        in_dir(&["check", "--base=--output=x", "--skip-label", "skip"]).expect_err("fails");
    assert_eq!(option.code, 2);
    std::fs::remove_dir_all(&dir).ok();
}
