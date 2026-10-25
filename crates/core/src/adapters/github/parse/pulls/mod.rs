//! One page of the open pull requests, read into the issues they close.

use serde::Deserialize;

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{PageCursor, bad_reply};
use crate::domain::IssueNumber;

/// The issues closed by the pull requests of one page, and the next cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullsPage {
    closing: Vec<IssueNumber>,
    next: Option<String>,
}

impl PullsPage {
    /// The issues that the pull requests of this page close.
    #[must_use]
    pub fn closing(&self) -> &[IssueNumber] {
        &self.closing
    }

    /// The cursor to pass for the next page, if there is one.
    #[must_use]
    pub fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }
}

#[derive(Debug, Deserialize)]
struct Reply {
    data: Answer,
}

#[derive(Debug, Deserialize)]
struct Answer {
    repository: Repository,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Repository {
    pull_requests: Pulls,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pulls {
    page_info: PageCursor,
    nodes: Vec<Pull>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pull {
    closing_issues_references: Issues,
}

#[derive(Debug, Deserialize)]
struct Issues {
    nodes: Vec<Issue>,
}

#[derive(Debug, Deserialize)]
struct Issue {
    number: u64,
}

/// Reads one page of open pull requests.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON.
pub fn parse_pulls(json: &str) -> Result<PullsPage, GhError> {
    let reply: Reply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let pulls = reply.data.repository.pull_requests;
    let closing = pulls
        .nodes
        .iter()
        .flat_map(|pull| &pull.closing_issues_references.nodes)
        .map(|issue| IssueNumber::new(issue.number))
        .collect();
    Ok(PullsPage {
        closing,
        next: pulls.page_info.next(),
    })
}
