//! One page of the text of the repository's issues or pull requests, read into
//! text items.

mod shapes;

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::bad_reply;
use crate::adapters::github::parse::texts::shapes::{
    Bodied, Bodies, Head, IssueNode, IssuesReply, Listing, PullNode, PullsReply, Tail, Thread,
};
use crate::domain::{IssueNumber, Surface, Text, TextItem};

/// The texts of one page, the cursor of the next page and how many issues or
/// pull requests the page held and the repository has in all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextsPage {
    items: Vec<TextItem>,
    next: Option<String>,
    read: u64,
    total: u64,
}

impl TextsPage {
    /// The texts of this page.
    #[must_use]
    pub fn items(&self) -> &[TextItem] {
        &self.items
    }

    /// The cursor to pass for the next page, if there is one.
    #[must_use]
    pub fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }

    /// How many issues or pull requests this page held.
    #[must_use]
    pub const fn read(&self) -> u64 {
        self.read
    }

    /// How many the repository has, open and closed.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }
}

/// Reads one page of issues with their title, body and comments.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON, and
/// [`GhError::Incomplete`] when an issue has more comments than the page
/// returned.
pub fn parse_issue_texts(json: &str) -> Result<TextsPage, GhError> {
    let reply: IssuesReply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let page = reply.data.repository.issues;
    let items = page
        .nodes
        .iter()
        .map(issue_items)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(page_of(&page, items.concat()))
}

/// Reads one page of pull requests with their title, body, comments, reviews
/// and review threads.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON, and
/// [`GhError::Incomplete`] when a pull request has more comments, reviews,
/// review threads or thread comments than the page returned.
pub fn parse_pull_texts(json: &str) -> Result<TextsPage, GhError> {
    let reply: PullsReply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let page = reply.data.repository.pull_requests;
    let items = page
        .nodes
        .iter()
        .map(pull_items)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(page_of(&page, items.concat()))
}

fn page_of<Node>(page: &Listing<Node>, items: Vec<TextItem>) -> TextsPage {
    TextsPage {
        items,
        next: page.page_info.next(),
        read: u64::try_from(page.nodes.len()).unwrap_or(u64::MAX),
        total: page.total_count,
    }
}

fn issue_items(node: &IssueNode) -> Result<Vec<TextItem>, GhError> {
    let number = IssueNumber::new(node.head.number);
    whole(number, "comments", &node.comments.page_info)?;
    let mut items = head_items(number, &node.head);
    items.extend(bodies(number, Surface::Comment, &node.comments.nodes));
    Ok(items)
}

fn pull_items(node: &PullNode) -> Result<Vec<TextItem>, GhError> {
    let number = IssueNumber::new(node.head.number);
    whole(number, "comments", &node.comments.page_info)?;
    whole(number, "reviews", &node.reviews.page_info)?;
    whole(number, "review threads", &node.review_threads.page_info)?;
    let mut items = head_items(number, &node.head);
    items.extend(bodies(number, Surface::Comment, &node.comments.nodes));
    items.extend(bodies(number, Surface::Review, &node.reviews.nodes));
    items.extend(thread_items(number, &node.review_threads.nodes)?);
    Ok(items)
}

fn thread_items(number: IssueNumber, threads: &[Thread]) -> Result<Vec<TextItem>, GhError> {
    let lists = threads
        .iter()
        .map(|thread| comments_of(number, &thread.comments))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lists.concat())
}

fn comments_of(number: IssueNumber, comments: &Bodies) -> Result<Vec<TextItem>, GhError> {
    whole(number, "comments in a review thread", &comments.page_info)?;
    Ok(bodies(number, Surface::ReviewComment, &comments.nodes))
}

/// Refuses a nested connection that has more nodes than the page returned.
fn whole(number: IssueNumber, what: &str, tail: &Tail) -> Result<(), GhError> {
    if tail.is_cut_off() {
        return Err(GhError::Incomplete {
            what: format!("{number} has more {what} than the page returned"),
        });
    }
    Ok(())
}

/// The title, and the body unless it is empty.
fn head_items(number: IssueNumber, head: &Head) -> Vec<TextItem> {
    let title = item(number, Surface::Title, &head.title);
    let body = item(number, Surface::Body, &head.body);
    title.into_iter().chain(body).collect()
}

/// The non-empty bodies, all on one surface.
fn bodies(number: IssueNumber, surface: Surface, nodes: &[Bodied]) -> Vec<TextItem> {
    nodes
        .iter()
        .filter_map(|node| item(number, surface, &node.body))
        .collect()
}

fn item(number: IssueNumber, surface: Surface, raw: &str) -> Option<TextItem> {
    (!raw.trim().is_empty()).then(|| TextItem::new(number, surface, Text::new(raw)))
}
