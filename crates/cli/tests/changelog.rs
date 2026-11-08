//! `tools changelog check`: the command line, the verdict and a run against a git repository.
#![cfg(feature = "changelog")]

use std::path::Path;
use std::process::Command;

use tools::changelog::{Policy, verdict};
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

const WATCH: [&str; 4] = ["crates/", "action.yml", "scripts/", ".github/workflows/"];

/// The verdict of this repository's policy.
fn judge(changed: &[&str], labels: &[&str], skip_label: &str) -> tools::Exit {
    let (watch, labels) = (names(&WATCH), names(labels));
    let policy = Policy {
        watch: &watch,
        changelog: "CHANGELOG.md",
        labels: &labels,
        skip_label,
    };
    verdict(&policy, &names(changed))
}

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn a_change_outside_the_watched_paths_needs_no_entry() {
    let exit = judge(&["README.md", "docs/a.md", "tools.toml"], &[], "skip");
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
        let exit = judge(&[path], &[], "skip");
        assert_eq!(exit.code, 1, "{path}");
        assert!(exit.text.contains(path), "{}", exit.text);
        assert!(exit.text.contains("`skip`"), "{}", exit.text);
    }
}

#[test]
fn a_look_alike_path_is_not_watched() {
    let exit = judge(
        &[
            "action.yml.bak",
            "crates-notes.md",
            ".github/ISSUE_TEMPLATE/fix.yml",
            "docs/scripts/a",
        ],
        &[],
        "skip",
    );
    assert_eq!(exit.code, 0, "{}", exit.text);
}

#[test]
fn touching_the_changelog_satisfies_the_check() {
    let exit = judge(&["crates/cli/src/lib.rs", "CHANGELOG.md"], &[], "skip");
    assert_eq!(exit.code, 0);
}

#[test]
fn only_the_root_changelog_counts() {
    let exit = judge(&["crates/cli/src/lib.rs", "docs/CHANGELOG.md"], &[], "skip");
    assert_eq!(exit.code, 1);
}

#[test]
fn the_skip_label_waives_the_entry() {
    let exit = judge(&["action.yml"], &["type:chore", "skip"], "skip");
    assert_eq!(exit.code, 0);
    assert_eq!(judge(&["action.yml"], &["skipped"], "skip").code, 1);
}

/// The flags every run needs besides `--base`, `--labels` and `--skip-label`.
const POLICY_FLAGS: [&str; 4] = ["--watch", "crates/", "--changelog", "CHANGELOG.md"];

/// `check` with the given flags and this repository's watched paths and changelog.
fn check(flags: &[&str]) -> Result<tools::changelog::cli::Arguments, UsageError> {
    let words: Vec<&str> = ["check"]
        .into_iter()
        .chain(flags.iter().copied())
        .chain(POLICY_FLAGS)
        .collect();
    parse(&words)
}

#[test]
fn the_command_line_reads_every_flag() {
    let arguments = parse(&[
        "check",
        "--base",
        "origin/main",
        "--labels",
        "a, b,,c",
        "--skip-label",
        "skip",
        "--watch",
        "crates/",
        "--watch",
        "action.yml",
        "--changelog",
        "CHANGELOG.md",
    ])
    .expect("parses");
    assert_eq!(arguments.base(), "origin/main");
    assert_eq!(arguments.labels(), ["a", "b", "c"]);
    assert_eq!(arguments.skip_label(), "skip");
    assert_eq!(arguments.watch(), ["crates/", "action.yml"]);
    assert_eq!(arguments.changelog(), "CHANGELOG.md");
}

#[test]
fn the_command_line_needs_base_skip_label_watch_and_changelog() {
    assert!(check(&["--skip-label", "skip"]).is_err());
    assert!(check(&["--base", "main"]).is_err());
    let flags = ["--base", "main", "--skip-label", "s"];
    let without = |flag: &str| {
        let mut words = vec!["check"];
        words.extend(flags);
        words.extend(
            POLICY_FLAGS
                .chunks(2)
                .filter(|pair| pair[0] != flag)
                .flatten(),
        );
        parse(&words)
    };
    assert!(without("--watch").is_err());
    assert!(without("--changelog").is_err());
    assert!(without("--none").is_ok());
    assert!(parse(&[]).is_err());
}

#[test]
fn labels_may_be_empty() {
    let arguments =
        check(&["--base", "main", "--labels", "", "--skip-label", "skip"]).expect("parses");
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
    std::fs::write(dir.join("crates/é.rs"), "x").expect("write");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "--quiet", "-m", "code"]);

    // The built binary in `dir`, with git's own variables cleared: a git hook running this test
    // exports GIT_DIR, which would point the binary's `git diff` at the repository being pushed.
    let in_dir = |words: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_tools"))
            .current_dir(&dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_WORK_TREE")
            .args(["changelog", "check"])
            .args(words)
            .args(POLICY_FLAGS)
            .output()
            .expect("tools runs");
        (
            output.status.code().expect("exit code"),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };

    let (code, text) = in_dir(&["--base", "main", "--skip-label", "skip"]);
    assert_eq!(code, 1, "{text}");
    let (code, text) = in_dir(&["--base", "main", "--labels", "skip", "--skip-label", "skip"]);
    assert_eq!(code, 0, "{text}");

    std::fs::write(dir.join("CHANGELOG.md"), "x").expect("write");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "--quiet", "-m", "changelog"]);
    let (code, text) = in_dir(&["--base", "main", "--skip-label", "skip"]);
    assert_eq!(code, 0, "{text}");

    assert_eq!(in_dir(&["--base", "nowhere", "--skip-label", "skip"]).0, 2);
    assert_eq!(in_dir(&["--base=--output=x", "--skip-label", "skip"]).0, 2);
    std::fs::remove_dir_all(&dir).ok();
}
