//! Reading what `gh` printed. Every function here is pure: it takes text and
//! returns domain values, so it is tested without `gh`.

pub mod issues;
pub mod items;
pub mod pulls;
pub mod repo;
pub mod texts;
pub mod write;

use crate::adapters::github::error::GhError;
use crate::domain::Repo;

pub use crate::adapters::github::parse::issues::{IssuesPage, parse_issues, parse_issues_for};
pub use crate::adapters::github::parse::items::{ItemsPage, parse_items, parse_items_with};
pub use crate::adapters::github::parse::pulls::{PullsPage, parse_pulls};
pub use crate::adapters::github::parse::repo::{
    RepoLabelsPage, RepoMilestonesPage, parse_repo_labels, parse_repo_milestones,
    parse_repo_project, parse_repo_settings,
};
pub use crate::adapters::github::parse::texts::{
    CutOff, ParsedTextsPage, Tail, TextsPage, parse_issue_page, parse_issue_texts,
    parse_item_comments, parse_pull_page, parse_pull_review_threads, parse_pull_texts,
    parse_thread_comments,
};
pub use crate::adapters::github::parse::write::{StatusField, parse_item_id, parse_status_field};

/// Wraps a JSON failure as an unreadable answer.
pub(crate) fn bad_reply(error: &serde_json::Error) -> GhError {
    GhError::BadReply {
        reason: error.to_string(),
    }
}

/// GraphQL's `pageInfo`. `hasNextPage` is kept as a JSON value and matched
/// where it is read, so no boolean crosses a function boundary.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageCursor {
    has_next_page: serde_json::Value,
    end_cursor: Option<String>,
}

impl PageCursor {
    /// The cursor of the next page, if there is one.
    pub(crate) fn next(&self) -> Option<String> {
        match self.has_next_page {
            serde_json::Value::Bool(true) => self.end_cursor.clone(),
            _ => None,
        }
    }
}

/// GraphQL's `{ repository { nameWithOwner } }` reference.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RepositoryRef {
    pub(crate) name_with_owner: String,
}

impl RepositoryRef {
    /// Parses the repository coordinate from `owner/repo`.
    pub(crate) fn to_repo(&self) -> Option<Repo> {
        Repo::parse(&self.name_with_owner)
    }

    /// Resolves the coordinate or falls back to `default`.
    pub(crate) fn resolve_or(&self, default: &Repo) -> Repo {
        self.to_repo().unwrap_or_else(|| default.clone())
    }
}
