//! One page of the open pull requests, read into the issues they close.

use serde::Deserialize;

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{PageCursor, RepositoryRef, bad_reply};
use crate::domain::{Closes, IssueNumber, Repo};

/// The closing references of the pull requests of one page, and the next cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullsPage {
    closing: Vec<Closes>,
    next: Option<String>,
}

impl PullsPage {
    /// Each open pull request of this page with the issue it closes.
    #[must_use]
    pub fn closing(&self) -> &[Closes] {
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
    number: u64,
    closing_issues_references: Issues,
}

#[derive(Debug, Deserialize)]
struct Issues {
    nodes: Vec<Issue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Issue {
    number: u64,
    repository: Option<RepositoryRef>,
}

/// Reads one page of open pull requests.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON.
pub fn parse_pulls(json: &str, repo: &Repo) -> Result<PullsPage, GhError> {
    let reply: Reply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let pulls = reply.data.repository.pull_requests;
    let closing = pulls
        .nodes
        .iter()
        .flat_map(|pull| {
            pull.closing_issues_references
                .nodes
                .iter()
                .map(move |issue| (pull.number, issue))
        })
        .map(|(pull, issue)| {
            let issue_repo = issue
                .repository
                .as_ref()
                .map_or_else(|| repo.clone(), |r| r.resolve_or(repo));
            Closes::new(
                IssueNumber::new(repo.clone(), pull),
                IssueNumber::new(issue_repo, issue.number),
            )
        })
        .collect();
    Ok(PullsPage {
        closing,
        next: pulls.page_info.next(),
    })
}
