//! The GitHub-backed parent resolver, against a stand-in for `gh`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tools_core::adapters::github::{GhProject, GhRunner};
use tools_core::domain::{IssueNumber, Repo};
use tools_core::workflow::{ParentResolver, ResolveError};

use crate::support::{fixture_target, statuses};

const ISSUE: &str = r#"{"data":{"repository":{"issue":{
  "number": 5, "state": "OPEN",
  "repository": {"nameWithOwner": "other/api"},
  "labels": {"nodes": [{"name": "type:feat"}]},
  "milestone": {"title": "v1"},
  "parent": {"number": 2, "repository": {"nameWithOwner": "third/web"}, "labels": {"nodes": []}},
  "blockedBy": {"nodes": [{"number": 9, "state": "CLOSED", "repository": {"nameWithOwner": "other/api"}}]}
}}}}"#;
const NO_ISSUE: &str = r#"{"data":{"repository":{"issue":null}}}"#;

/// Every stand-in `gh`, written once before any test runs one: a script that
/// is being written while another thread forks cannot be executed ("text file
/// busy"), so no test writes its own.
fn scripts() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("tools-57-gh-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("a temp dir");
        let cases: [(&str, &str, &str); 6] = [
            ("ok", ISSUE, ""),
            (
                "hidden",
                "",
                "gh: Could not resolve to a Repository with the name 'other/api'.",
            ),
            (
                "refused",
                "",
                "gh: Resource not accessible by integration (HTTP 403)",
            ),
            ("gone", NO_ISSUE, ""),
            ("down", "", "gh: HTTP 502: Bad Gateway"),
            ("junk", "not json", ""),
        ];
        for (name, stdout, stderr) in cases {
            let script = dir.join(name);
            let body = if stderr.is_empty() {
                format!("#!/bin/sh\ncat <<'EOF'\n{stdout}\nEOF\n")
            } else {
                format!("#!/bin/sh\ncat >&2 <<'EOF'\n{stderr}\nEOF\nexit 1\n")
            };
            fs::write(&script, body).expect("the script is written");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
        }
        dir
    })
}

fn fake_gh(name: &str) -> GhRunner {
    let script = scripts().join(name);
    GhRunner::new(script.to_str().expect("a utf-8 path"))
}

fn parent() -> IssueNumber {
    IssueNumber::new(Repo::new("other", "api"), 5)
}

fn resolver(name: &str) -> GhProject {
    GhProject::with_runner(fake_gh(name), fixture_target(), statuses())
}

#[test]
fn a_parent_in_another_repository_is_read_with_its_own_parent() {
    let subject = resolver("ok").fetch(&parent()).expect("readable");
    assert_eq!(subject.number, parent());
    assert!(!subject.closed);
    assert_eq!(subject.labels, ["type:feat"]);
    assert_eq!(subject.milestone.as_deref(), Some("v1"));
    assert_eq!(subject.status, None);
    assert_eq!(subject.blockers.len(), 1);
    assert_eq!(
        subject.parent,
        Some(IssueNumber::new(Repo::new("third", "web"), 2))
    );
}

#[test]
fn a_repository_github_cannot_resolve_is_one_the_token_cannot_read() {
    let error = resolver("hidden").fetch(&parent()).expect_err("hidden");
    assert_eq!(error, ResolveError::Forbidden);
}

#[test]
fn a_refused_request_is_forbidden() {
    let error = resolver("refused").fetch(&parent()).expect_err("refused");
    assert_eq!(error, ResolveError::Forbidden);
}

#[test]
fn an_issue_that_does_not_exist_is_not_found() {
    let error = resolver("gone").fetch(&parent()).expect_err("gone");
    assert_eq!(error, ResolveError::NotFound);
}

#[test]
fn any_other_failure_is_reported_with_gh_s_words() {
    let error = resolver("down").fetch(&parent()).expect_err("down");
    assert!(
        matches!(&error, ResolveError::Failed(text) if text.contains("502")),
        "{error:?}"
    );
}

#[test]
fn an_answer_that_is_not_json_is_a_failure_not_a_guess() {
    let error = resolver("junk").fetch(&parent()).expect_err("junk");
    assert!(matches!(error, ResolveError::Failed(_)), "{error:?}");
}
