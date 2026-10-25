//! The read-only picture of the Project that the rules look at.

use crate::domain::card::{Card, Cards};
use crate::domain::issue::{IssueNumber, Status};

/// The issues that an open pull request closes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Closing(Vec<IssueNumber>);

impl Closing {
    /// The issues closed by open pull requests.
    #[must_use]
    pub const fn new(issues: Vec<IssueNumber>) -> Self {
        Self(issues)
    }

    /// Whether an open pull request closes this issue.
    #[must_use]
    pub fn contains(&self, number: IssueNumber) -> bool {
        self.0.contains(&number)
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
    pub fn card(&self, number: IssueNumber) -> Option<&Card> {
        self.cards.find(number)
    }

    /// Whether an open pull request closes this issue.
    #[must_use]
    pub fn closed_by_open_pull(&self, number: IssueNumber) -> bool {
        self.closing.contains(number)
    }

    /// This snapshot with the card of `number` moved to `status`.
    #[must_use]
    pub fn with_status(self, number: IssueNumber, status: Status) -> Self {
        Self {
            cards: self.cards.with_status(number, status),
            ..self
        }
    }
}
