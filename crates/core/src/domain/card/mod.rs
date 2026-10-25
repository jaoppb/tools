//! A card of the Project: one issue with its status, facts and links.

use crate::domain::facts::{Facts, Missing};
use crate::domain::issue::{IssueNumber, Lifecycle, Status};
use crate::domain::links::Links;

/// A status together with whether the issue is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    status: Status,
    lifecycle: Lifecycle,
}

impl Standing {
    /// A standing with this status and lifecycle.
    #[must_use]
    pub const fn new(status: Status, lifecycle: Lifecycle) -> Self {
        Self { status, lifecycle }
    }
}

/// One issue of the Project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    number: IssueNumber,
    standing: Standing,
    facts: Facts,
    links: Links,
}

impl Card {
    /// A card with this number and standing, no facts and no links.
    #[must_use]
    pub fn new(number: IssueNumber, standing: Standing) -> Self {
        Self {
            number,
            standing,
            facts: Facts::default(),
            links: Links::default(),
        }
    }

    /// This card with these facts.
    #[must_use]
    pub fn with_facts(self, facts: Facts) -> Self {
        Self { facts, ..self }
    }

    /// This card with these links.
    #[must_use]
    pub fn with_links(self, links: Links) -> Self {
        Self { links, ..self }
    }

    /// This card moved to `status`.
    #[must_use]
    pub fn with_status(self, status: Status) -> Self {
        Self {
            standing: Standing::new(status, self.standing.lifecycle),
            ..self
        }
    }

    /// The issue number.
    #[must_use]
    pub const fn number(&self) -> IssueNumber {
        self.number
    }

    /// The current Status.
    #[must_use]
    pub const fn status(&self) -> Status {
        self.standing.status
    }

    /// Whether the issue is closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.standing.lifecycle.is_closed()
    }

    /// Whether the issue is an epic.
    #[must_use]
    pub fn is_epic(&self) -> bool {
        self.facts.is_epic()
    }

    /// Whether every blocker is closed.
    #[must_use]
    pub fn unblocked(&self) -> bool {
        self.links.unblocked()
    }

    /// What triage still has to set.
    #[must_use]
    pub fn missing(&self) -> Vec<Missing> {
        self.facts.missing()
    }

    /// The story this card is a task of, if its parent is a story.
    #[must_use]
    pub fn story(&self) -> Option<IssueNumber> {
        self.links.story()
    }
}

/// Every card of the Project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cards(Vec<Card>);

impl Cards {
    /// The cards of a Project.
    #[must_use]
    pub const fn new(cards: Vec<Card>) -> Self {
        Self(cards)
    }

    /// The card of this issue, if the Project has it.
    #[must_use]
    pub fn find(&self, number: IssueNumber) -> Option<&Card> {
        self.0.iter().find(|card| card.number == number)
    }

    /// The cards in Project order.
    pub fn iter(&self) -> impl Iterator<Item = &Card> {
        self.0.iter()
    }

    /// These cards with the card of `number` moved to `status`.
    #[must_use]
    pub fn with_status(self, number: IssueNumber, status: Status) -> Self {
        Self(
            self.0
                .into_iter()
                .map(|card| moved_if(card, number, status))
                .collect(),
        )
    }
}

fn moved_if(card: Card, number: IssueNumber, status: Status) -> Card {
    if card.number == number {
        return card.with_status(status);
    }
    card
}
