//! The wire shapes of one page of issue texts or pull request texts, as GraphQL
//! prints them.
//!
//! Everything here is visible to the parent module only, which turns the
//! shapes into domain values.

use serde::Deserialize;

use crate::adapters::github::parse::PageCursor;

#[derive(Debug, Deserialize)]
pub(super) struct IssuesReply {
    pub(super) data: IssuesAnswer,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssuesAnswer {
    pub(super) repository: IssuesRepository,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssuesRepository {
    pub(super) issues: Listing<IssueNode>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PullsReply {
    pub(super) data: PullsAnswer,
}

#[derive(Debug, Deserialize)]
pub(super) struct PullsAnswer {
    pub(super) repository: PullsRepository,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullsRepository {
    pub(super) pull_requests: Listing<PullNode>,
}

/// One page of issues or pull requests.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Listing<Node> {
    pub(super) total_count: u64,
    pub(super) page_info: PageCursor,
    pub(super) nodes: Vec<Node>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueNode {
    #[serde(flatten)]
    pub(super) head: Head,
    pub(super) comments: Bodies,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullNode {
    #[serde(flatten)]
    pub(super) head: Head,
    pub(super) comments: Bodies,
    pub(super) reviews: Bodies,
    pub(super) review_threads: Threads,
}

/// What an issue and a pull request have in common.
#[derive(Debug, Deserialize)]
pub(super) struct Head {
    pub(super) number: u64,
    pub(super) title: String,
    pub(super) body: String,
}

/// A connection of comments or reviews, read for their bodies.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Bodies {
    pub(super) page_info: Tail,
    pub(super) nodes: Vec<Bodied>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Bodied {
    pub(super) body: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Threads {
    pub(super) page_info: Tail,
    pub(super) nodes: Vec<Thread>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Thread {
    pub(super) id: Option<String>,
    pub(super) comments: Bodies,
}

/// The `pageInfo` of a nested connection. The value is kept as JSON so that
/// anything but an explicit `false` counts as cut off.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tail {
    has_next_page: serde_json::Value,
    end_cursor: Option<String>,
}

impl Tail {
    /// Whether the connection has more nodes than the page returned.
    #[must_use]
    pub const fn is_cut_off(&self) -> bool {
        !matches!(self.has_next_page, serde_json::Value::Bool(false))
    }

    /// The cursor of the next page, if there is one.
    #[must_use]
    pub fn cursor(&self) -> Option<&str> {
        self.end_cursor.as_deref()
    }
}

/// A unified response for additional comments or reviews using the `item` alias.
#[derive(Debug, Deserialize)]
pub(super) struct ItemCommentsReply {
    pub(super) data: ItemCommentsAnswer,
}

#[derive(Debug, Deserialize)]
pub(super) struct ItemCommentsAnswer {
    pub(super) repository: ItemCommentsRepo,
}

#[derive(Debug, Deserialize)]
pub(super) struct ItemCommentsRepo {
    pub(super) item: Option<CommentsNode>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CommentsNode {
    pub(super) comments: Bodies,
}

/// A response for additional review threads of a pull request.
#[derive(Debug, Deserialize)]
pub(super) struct PullThreadsReply {
    pub(super) data: PullThreadsAnswer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullThreadsAnswer {
    pub(super) repository: PullThreadsRepo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullThreadsRepo {
    pub(super) pull_request: Option<ThreadsNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ThreadsNode {
    pub(super) review_threads: Threads,
}

/// A response for additional comments of a single review thread.
#[derive(Debug, Deserialize)]
pub(super) struct ThreadCommentsReply {
    pub(super) data: ThreadCommentsAnswer,
}

#[derive(Debug, Deserialize)]
pub(super) struct ThreadCommentsAnswer {
    pub(super) node: Option<CommentsNode>,
}
