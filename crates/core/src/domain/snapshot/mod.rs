//! The read-only picture of the Project that the rules look at.

use crate::domain::card::{Card, Cards};
use crate::domain::issue::{IssueNumber, StatusName};

/// An open pull request and the issue it closes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closes {
    pull: IssueNumber,
    issue: IssueNumber,
}

impl Closes {
    /// The open pull request `pull` closes `issue`.
    #[must_use]
    pub const fn new(pull: IssueNumber, issue: IssueNumber) -> Self {
        Self { pull, issue }
    }

    /// The pull request.
    #[must_use]
    pub const fn pull(&self) -> &IssueNumber {
        &self.pull
    }

    /// The issue it closes.
    #[must_use]
    pub const fn issue(&self) -> &IssueNumber {
        &self.issue
    }
}

/// The open pull requests and the issues they close.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Closing(Vec<Closes>);

impl Closing {
    /// The closing references of the open pull requests.
    #[must_use]
    pub const fn new(references: Vec<Closes>) -> Self {
        Self(references)
    }

    /// The open pull requests that close this issue.
    pub fn pulls_of<'a>(&'a self, issue: &'a IssueNumber) -> impl Iterator<Item = &'a IssueNumber> {
        self.0
            .iter()
            .filter(move |closes| closes.issue == *issue)
            .map(|closes| &closes.pull)
    }
}

/// The cards of the Project and the issues that open pull requests close.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    cards: Cards,
    closing: Closing,
}

impl Snapshot {
    /// A snapshot of these cards and closing references.
    #[must_use]
    pub const fn new(cards: Cards, closing: Closing) -> Self {
        Self { cards, closing }
    }

    /// The cards in Project order.
    pub fn cards(&self) -> impl Iterator<Item = &Card> {
        self.cards.iter()
    }

    /// The card of this issue, if the Project has it.
    #[must_use]
    pub fn card(&self, number: &IssueNumber) -> Option<&Card> {
        self.cards.find(number)
    }

    /// The open pull requests that close this issue.
    #[must_use]
    pub fn open_pulls_closing(&self, number: &IssueNumber) -> Vec<IssueNumber> {
        self.closing.pulls_of(number).cloned().collect()
    }

    /// This snapshot with the card of `number` moved to `status`.
    #[must_use]
    pub fn with_status(self, number: &IssueNumber, status: &StatusName) -> Self {
        Self {
            cards: self.cards.with_status(number, status),
            ..self
        }
    }
}
