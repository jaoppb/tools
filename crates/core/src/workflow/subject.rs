//! The input of the engine: one issue as the fact schema sees it.

use std::collections::BTreeMap;

use crate::domain::{Card, IssueNumber, Snapshot};

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
    pub status: Option<String>,
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
    /// Whether an open pull request closes the issue.
    pub closed_by_open_pr: bool,
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
            status: Some(card.status().name().to_owned()),
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
            closed_by_open_pr: snapshot.closed_by_open_pull(card.number()),
        }
    }
}

/// Fetches an issue that is not a Project card, on demand: a parent in another
/// repository or outside the Project. Only the parent chain is ever asked for.
pub trait ParentResolver {
    /// The subject of `number`.
    ///
    /// # Errors
    ///
    /// A message when the issue cannot be read (a repository the token cannot
    /// see, a deleted issue). The engine freezes the child that needed it.
    fn fetch(&self, number: &IssueNumber) -> Result<Subject, String>;
}
