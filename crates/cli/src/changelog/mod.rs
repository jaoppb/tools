//! `tools changelog check`: a change to the product or its release flow records itself in
//! `CHANGELOG.md`.
//!
//! Exits with 0 when the change touches none of the watched paths, touches `CHANGELOG.md`, or the
//! pull request carries the skip label; with 1 when it touches a watched path without the
//! changelog; and with 2 when the changed files cannot be read from git.

pub mod cli;

use std::process::Command;

use crate::Exit;
use cli::Arguments;

const CHANGELOG: &str = "CHANGELOG.md";
/// Paths whose change a release note must mention: a directory ends in `/`, a file does not.
const WATCHED: [&str; 4] = ["crates/", "action.yml", "scripts/", ".github/workflows/"];

/// Runs `tools changelog check`.
///
/// # Errors
///
/// Returns an [`Exit`] with code 2 when git cannot list the changed files.
pub fn run(arguments: &Arguments) -> Result<Exit, Exit> {
    let changed = changed_files(arguments.base())?;
    Ok(verdict(
        &changed,
        arguments.labels(),
        arguments.skip_label(),
    ))
}

/// The verdict for the files a change touches and the labels on its pull request.
#[must_use]
pub fn verdict(changed: &[String], labels: &[String], skip_label: &str) -> Exit {
    let watched: Vec<&str> = changed
        .iter()
        .map(String::as_str)
        .filter(|path| is_watched(path))
        .collect();
    if watched.is_empty() {
        return Exit::new("no watched path changed: no changelog entry needed", 0);
    }
    if changed.iter().any(|path| path == CHANGELOG) {
        return Exit::new("CHANGELOG.md is part of the change", 0);
    }
    if labels.iter().any(|label| label == skip_label) {
        return Exit::new(
            format!("changelog entry skipped by the `{skip_label}` label"),
            0,
        );
    }
    let files: Vec<String> = watched.iter().map(|path| format!("  {path}")).collect();
    Exit::new(
        format!(
            "this change touches {} without touching {CHANGELOG}:\n{}\n\
             add one bullet under `Unreleased` ending `(#issue, #PR)`, or label the pull request \
             `{skip_label}` when nothing consumer-visible changed",
            if watched.len() == 1 {
                "a watched path"
            } else {
                "watched paths"
            },
            files.join("\n"),
        ),
        1,
    )
}

fn is_watched(path: &str) -> bool {
    WATCHED.iter().any(|watched| {
        if watched.ends_with('/') {
            path.starts_with(watched)
        } else {
            path == *watched
        }
    })
}

fn changed_files(base: &str) -> Result<Vec<String>, Exit> {
    if base.starts_with('-') {
        return Err(Exit::failed(format!("`{base}` is not a git revision")));
    }
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{base}...HEAD")])
        .output()
        .map_err(|error| Exit::failed(format!("failed to run git: {error}")))?;
    if !output.status.success() {
        return Err(Exit::failed(format!(
            "git diff {base}...HEAD failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}
