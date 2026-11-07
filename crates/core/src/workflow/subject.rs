//! The input of the engine: one issue as the fact schema sees it.

use std::collections::BTreeMap;
use std::fmt;

use crate::domain::{Card, IssueNumber, Snapshot, StatusName};

/// A Project field value: a select option or a number.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// A single-select option name or text.
    Text(String),
    /// A number field.
    Number(i64),
}

/// Everything the schema exposes about one issue, before any rule runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Subject {
    /// The issue.
    pub number: IssueNumber,
    /// Whether the issue is closed.
    pub closed: bool,
    /// The board column; `None` when the issue is not a Project card.
    pub status: Option<StatusName>,
    /// The label names.
    pub labels: Vec<String>,
    /// The milestone title.
    pub milestone: Option<String>,
    /// The Project fields that are set. An unset field is absent.
    pub fields: BTreeMap<String, FieldValue>,
    /// The blockers and whether each is closed.
    pub blockers: Vec<(IssueNumber, bool)>,
    /// The parent issue.
    pub parent: Option<IssueNumber>,
    /// The open pull requests that close the issue.
    pub prs: Vec<IssueNumber>,
}

impl Subject {
    /// The subject of a Project card.
    #[must_use]
    pub fn from_card(card: &Card, snapshot: &Snapshot) -> Self {
        let facts = card.facts();
        let mut fields = BTreeMap::new();
        if let Some(priority) = facts.priority_name() {
            fields.insert("Priority".to_owned(), FieldValue::Text(priority));
        }
        if let Some(points) = facts.effort_points() {
            fields.insert("Effort".to_owned(), FieldValue::Number(i64::from(points)));
        }
        Self {
            number: card.number().clone(),
            closed: card.is_closed(),
            status: Some(card.status().clone()),
            labels: facts.labels().names().map(str::to_owned).collect(),
            milestone: facts.milestone_title(),
            fields,
            blockers: card
                .links()
                .blockers()
                .iter()
                .map(|(number, closed)| (number.clone(), closed))
                .collect(),
            parent: card.links().parent_number().cloned(),
            prs: snapshot.open_pulls_closing(card.number()),
        }
    }
}

/// Why a [`ParentResolver`] could not return an issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The issue does not exist (deleted, transferred).
    NotFound,
    /// The token cannot read the repository the issue lives in.
    Forbidden,
    /// Anything else, a transient network failure included.
    Failed(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => out.write_str("not found"),
            Self::Forbidden => out.write_str("the token cannot read its repository"),
            Self::Failed(message) => out.write_str(message),
        }
    }
}

/// Fetches an issue that is not a Project card, on demand: a parent in another
/// repository or outside the Project. Only the parent chain is ever asked for.
///
/// # Contract
///
/// - Synchronous and object-safe: the engine holds a `&dyn ParentResolver`.
/// - The engine asks for each issue number at most once per run and caches the
///   answer, an error included, so an implementation needs no cache of its own
///   for a single run.
/// - Only issues on the parent chain of a card that the Project does not hold
///   are requested; never a subtree, never a child.
/// - Every [`ResolveError`] freezes the child that needed the parent (no status
///   change, a `Violation::ParentUnreadable` carrying the error) and, because a
///   frozen issue has no derived facts, its own children too. The engine never
///   retries: a transient failure costs that issue one run, and the next run
///   evaluates it again.
/// - Must not panic; report every failure as a [`ResolveError`].
pub trait ParentResolver {
    /// The subject of `number`.
    ///
    /// # Errors
    ///
    /// A [`ResolveError`] when the issue cannot be read.
    fn fetch(&self, number: &IssueNumber) -> Result<Subject, ResolveError>;
}
