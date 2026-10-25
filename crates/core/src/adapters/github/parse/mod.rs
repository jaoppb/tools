//! Reading what `gh` printed. Every function here is pure: it takes text and
//! returns domain values, so it is tested without `gh`.

pub mod issues;
pub mod items;
pub mod pulls;
pub mod texts;
pub mod write;

use crate::adapters::github::error::GhError;

pub use crate::adapters::github::parse::issues::{IssuesPage, parse_issues};
pub use crate::adapters::github::parse::items::{ItemsPage, parse_items};
pub use crate::adapters::github::parse::pulls::{PullsPage, parse_pulls};
pub use crate::adapters::github::parse::texts::{TextsPage, parse_issue_texts, parse_pull_texts};
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
