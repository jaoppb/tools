//! One page of the text of the repository's issues or pull requests, read into
//! text items.

mod shapes;

pub use shapes::Tail;

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::bad_reply;
use crate::adapters::github::parse::texts::shapes::{
    Bodied, Head, IssueNode, IssuesReply, ItemCommentsReply, Listing, PullNode, PullThreadsReply,
    PullsReply, Thread, ThreadCommentsReply,
};
use crate::domain::{IssueNumber, Repo, Surface, Text, TextItem};

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
    /// Creates a new page of texts.
    #[must_use]
    pub const fn new(items: Vec<TextItem>, next: Option<String>, read: u64, total: u64) -> Self {
        Self {
            items,
            next,
            read,
            total,
        }
    }

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

/// A nested connection that was cut off and needs further pagination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CutOff {
    /// An issue whose comments have more pages.
    IssueComments(IssueNumber, Tail),
    /// A pull request whose comments have more pages.
    PullComments(IssueNumber, Tail),
    /// A pull request whose reviews have more pages.
    PullReviews(IssueNumber, Tail),
    /// A pull request whose review threads have more pages.
    PullThreads(IssueNumber, Tail),
    /// A review thread whose comments have more pages.
    ThreadComments(IssueNumber, Option<String>, Tail),
}

impl CutOff {
    /// Refuses this cut-off connection if not paginated.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Incomplete`] describing what was cut off.
    pub fn refuse(&self) -> Result<(), GhError> {
        match self {
            Self::IssueComments(number, tail) | Self::PullComments(number, tail) => {
                whole(number, "comments", tail)
            }
            Self::PullReviews(number, tail) => whole(number, "reviews", tail),
            Self::PullThreads(number, tail) => whole(number, "review threads", tail),
            Self::ThreadComments(number, _, tail) => {
                whole(number, "comments in a review thread", tail)
            }
        }
    }
}

/// The parsed texts of one top-level page and any nested connections that were cut off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTextsPage {
    page: TextsPage,
    cut_offs: Vec<CutOff>,
}

impl ParsedTextsPage {
    /// The page of texts.
    #[must_use]
    pub const fn page(&self) -> &TextsPage {
        &self.page
    }

    /// Consumes this parsed page into `TextsPage`, refusing if any nested connection was cut off.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Incomplete`] if any nested connection was cut off.
    pub fn into_complete(self) -> Result<TextsPage, GhError> {
        for cut_off in &self.cut_offs {
            cut_off.refuse()?;
        }
        Ok(self.page)
    }

    /// The cut-off connections that need further pagination.
    #[must_use]
    pub fn cut_offs(&self) -> &[CutOff] {
        &self.cut_offs
    }

    /// Consumes the parsed page into its parts: the page and any cut-offs.
    #[must_use]
    pub fn into_parts(self) -> (TextsPage, Vec<CutOff>) {
        (self.page, self.cut_offs)
    }
}

/// Parses one page of issues into items and any cut-offs.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON.
pub fn parse_issue_page(json: &str, repo: &Repo) -> Result<ParsedTextsPage, GhError> {
    let reply: IssuesReply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let page = reply.data.repository.issues;
    let mut items = Vec::new();
    let mut cut_offs = Vec::new();
    for node in &page.nodes {
        let (node_items, node_cut_offs) = issue_items(node, repo);
        items.extend(node_items);
        cut_offs.extend(node_cut_offs);
    }
    Ok(ParsedTextsPage {
        page: page_of(&page, items),
        cut_offs,
    })
}

/// Parses one page of pull requests into items and any cut-offs.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON.
pub fn parse_pull_page(json: &str, repo: &Repo) -> Result<ParsedTextsPage, GhError> {
    let reply: PullsReply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let page = reply.data.repository.pull_requests;
    let mut items = Vec::new();
    let mut cut_offs = Vec::new();
    for node in &page.nodes {
        let (node_items, node_cut_offs) = pull_items(node, repo);
        items.extend(node_items);
        cut_offs.extend(node_cut_offs);
    }
    Ok(ParsedTextsPage {
        page: page_of(&page, items),
        cut_offs,
    })
}

/// Reads one page of issues with their title, body and comments.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON, and
/// [`GhError::Incomplete`] when an issue has more comments than the page
/// returned.
pub fn parse_issue_texts(json: &str, repo: &Repo) -> Result<TextsPage, GhError> {
    parse_issue_page(json, repo)?.into_complete()
}

/// Reads one page of pull requests with their title, body, comments, reviews
/// and review threads.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON, and
/// [`GhError::Incomplete`] when a pull request has more comments, reviews,
/// review threads or thread comments than the page returned.
pub fn parse_pull_texts(json: &str, repo: &Repo) -> Result<TextsPage, GhError> {
    parse_pull_page(json, repo)?.into_complete()
}

/// Reads follow-up comments or reviews of an issue or pull request.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the JSON is malformed or the item is missing.
pub fn parse_item_comments(
    json: &str,
    number: &IssueNumber,
    surface: Surface,
) -> Result<(Vec<TextItem>, Tail), GhError> {
    let reply: ItemCommentsReply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let item = reply
        .data
        .repository
        .item
        .ok_or_else(|| GhError::BadReply {
            reason: format!("{number} not found in comments reply"),
        })?;
    let items = bodies(number, surface, &item.comments.nodes);
    Ok((items, item.comments.page_info))
}

/// Reads follow-up review threads of a pull request.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the JSON is malformed or the pull request is missing.
pub fn parse_pull_review_threads(
    json: &str,
    number: &IssueNumber,
) -> Result<(Vec<TextItem>, Vec<CutOff>, Tail), GhError> {
    let reply: PullThreadsReply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let pull = reply
        .data
        .repository
        .pull_request
        .ok_or_else(|| GhError::BadReply {
            reason: format!("{number} not found in review threads reply"),
        })?;
    let (items, cut_offs) = thread_items(number, &pull.review_threads.nodes);
    Ok((items, cut_offs, pull.review_threads.page_info))
}

/// Reads follow-up comments of a review thread.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the JSON is malformed or the review thread node is missing.
pub fn parse_thread_comments(
    json: &str,
    number: &IssueNumber,
) -> Result<(Vec<TextItem>, Tail), GhError> {
    let reply: ThreadCommentsReply =
        serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let node = reply.data.node.ok_or_else(|| GhError::BadReply {
        reason: format!("{number} review thread not found in comments reply"),
    })?;
    let items = bodies(number, Surface::ReviewComment, &node.comments.nodes);
    Ok((items, node.comments.page_info))
}

fn page_of<Node>(page: &Listing<Node>, items: Vec<TextItem>) -> TextsPage {
    TextsPage::new(
        items,
        page.page_info.next(),
        u64::try_from(page.nodes.len()).unwrap_or(u64::MAX),
        page.total_count,
    )
}

fn issue_items(node: &IssueNode, repo: &Repo) -> (Vec<TextItem>, Vec<CutOff>) {
    let number = IssueNumber::new(repo.clone(), node.head.number);
    let mut items = head_items(&number, &node.head);
    items.extend(bodies(&number, Surface::Comment, &node.comments.nodes));
    let mut cut_offs = Vec::new();
    if node.comments.page_info.is_cut_off() {
        cut_offs.push(CutOff::IssueComments(
            number,
            node.comments.page_info.clone(),
        ));
    }
    (items, cut_offs)
}

fn pull_items(node: &PullNode, repo: &Repo) -> (Vec<TextItem>, Vec<CutOff>) {
    let number = IssueNumber::new(repo.clone(), node.head.number);
    let mut items = head_items(&number, &node.head);
    let mut cut_offs = Vec::new();
    items.extend(bodies(&number, Surface::Comment, &node.comments.nodes));
    if node.comments.page_info.is_cut_off() {
        cut_offs.push(CutOff::PullComments(
            number.clone(),
            node.comments.page_info.clone(),
        ));
    }
    items.extend(bodies(&number, Surface::Review, &node.reviews.nodes));
    if node.reviews.page_info.is_cut_off() {
        cut_offs.push(CutOff::PullReviews(
            number.clone(),
            node.reviews.page_info.clone(),
        ));
    }
    let (t_items, t_cut_offs) = thread_items(&number, &node.review_threads.nodes);
    items.extend(t_items);
    cut_offs.extend(t_cut_offs);
    if node.review_threads.page_info.is_cut_off() {
        cut_offs.push(CutOff::PullThreads(
            number,
            node.review_threads.page_info.clone(),
        ));
    }
    (items, cut_offs)
}

fn thread_items(number: &IssueNumber, threads: &[Thread]) -> (Vec<TextItem>, Vec<CutOff>) {
    let mut items = Vec::new();
    let mut cut_offs = Vec::new();
    for thread in threads {
        items.extend(bodies(
            number,
            Surface::ReviewComment,
            &thread.comments.nodes,
        ));
        if thread.comments.page_info.is_cut_off() {
            cut_offs.push(CutOff::ThreadComments(
                number.clone(),
                thread.id.clone(),
                thread.comments.page_info.clone(),
            ));
        }
    }
    (items, cut_offs)
}

/// Refuses a nested connection that has more nodes than the page returned.
fn whole(number: &IssueNumber, what: &str, tail: &Tail) -> Result<(), GhError> {
    if tail.is_cut_off() {
        return Err(GhError::Incomplete {
            what: format!("{number} has more {what} than the page returned"),
        });
    }
    Ok(())
}

/// The title, and the body unless it is empty.
fn head_items(number: &IssueNumber, head: &Head) -> Vec<TextItem> {
    let title = item(number, Surface::Title, &head.title);
    let body = item(number, Surface::Body, &head.body);
    title.into_iter().chain(body).collect()
}

/// The non-empty bodies, all on one surface.
fn bodies(number: &IssueNumber, surface: Surface, nodes: &[Bodied]) -> Vec<TextItem> {
    nodes
        .iter()
        .filter_map(|node| item(number, surface, &node.body))
        .collect()
}

fn item(number: &IssueNumber, surface: Surface, raw: &str) -> Option<TextItem> {
    (!raw.trim().is_empty()).then(|| TextItem::new(number.clone(), surface, Text::new(raw)))
}
