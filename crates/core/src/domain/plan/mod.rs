//! The plan: the moves the rules produce and the gaps that need triage.

use std::fmt;

use crate::domain::card::Card;
use crate::domain::facts::Missing;
use crate::domain::issue::{IssueNumber, Status};
use crate::domain::snapshot::Snapshot;
use crate::domain::target::target;

/// Why a card moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The Ready gate holds.
    GateHolds,
    /// A Ready card no longer passes the gate.
    GateLost,
    /// A closed issue was reopened.
    Reopened,
    /// An open pull request closes the issue.
    PullRequestOpen,
    /// No open pull request closes the In review issue any more.
    PullRequestGone,
    /// The issue is closed.
    Closed,
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::GateHolds => "gate holds",
            Self::GateLost => "gate lost",
            Self::Reopened => "reopened",
            Self::PullRequestOpen => "pull request open",
            Self::PullRequestGone => "pull request gone",
            Self::Closed => "closed",
        };
        formatter.write_str(text)
    }
}

/// A change of Status from one value to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    origin: Status,
    destination: Status,
}

impl Transition {
    /// The change from `origin` to `destination`.
    #[must_use]
    pub const fn new(origin: Status, destination: Status) -> Self {
        Self {
            origin,
            destination,
        }
    }

    const fn reason(self) -> Reason {
        match (self.origin, self.destination) {
            (_, Status::Done) => Reason::Closed,
            (Status::Done, _) => Reason::Reopened,
            (_, Status::InReview) => Reason::PullRequestOpen,
            (Status::InReview, _) => Reason::PullRequestGone,
            (_, Status::Ready) => Reason::GateHolds,
            _ => Reason::GateLost,
        }
    }
}

/// One card changing Status, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    number: IssueNumber,
    transition: Transition,
    reason: Reason,
}

impl Move {
    /// A move of this issue by this transition, for this reason.
    #[must_use]
    pub const fn new(number: IssueNumber, transition: Transition, reason: Reason) -> Self {
        Self {
            number,
            transition,
            reason,
        }
    }

    /// The issue that moves.
    #[must_use]
    pub const fn number(&self) -> IssueNumber {
        self.number
    }

    /// The Status the issue moves to.
    #[must_use]
    pub const fn destination(&self) -> Status {
        self.transition.destination
    }
}

impl fmt::Display for Move {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {} -> {} ({})",
            self.number, self.transition.origin, self.transition.destination, self.reason
        )
    }
}

/// The moves of a plan, in Project order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Moves(Vec<Move>);

impl Moves {
    /// The moves of a plan.
    #[must_use]
    pub const fn new(moves: Vec<Move>) -> Self {
        Self(moves)
    }

    /// The moves in order.
    pub fn iter(&self) -> impl Iterator<Item = &Move> {
        self.0.iter()
    }

    /// How many moves there are.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there is no move.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// An issue that triage has to finish before it can be Ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    number: IssueNumber,
    missing: Missing,
}

impl Gap {
    /// A gap of this issue.
    #[must_use]
    pub const fn new(number: IssueNumber, missing: Missing) -> Self {
        Self { number, missing }
    }
}

impl fmt::Display for Gap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} needs triage: {}", self.number, self.missing)
    }
}

/// The gaps of a plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gaps(Vec<Gap>);

impl Gaps {
    /// The gaps of a plan.
    #[must_use]
    pub const fn new(gaps: Vec<Gap>) -> Self {
        Self(gaps)
    }

    /// The gaps in order.
    pub fn iter(&self) -> impl Iterator<Item = &Gap> {
        self.0.iter()
    }
}

/// What the sync would do, and what it cannot do for lack of triage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    moves: Moves,
    gaps: Gaps,
}

impl Plan {
    /// A plan of these moves and gaps.
    #[must_use]
    pub const fn new(moves: Moves, gaps: Gaps) -> Self {
        Self { moves, gaps }
    }

    /// The moves.
    #[must_use]
    pub const fn moves(&self) -> &Moves {
        &self.moves
    }

    /// The gaps.
    #[must_use]
    pub const fn gaps(&self) -> &Gaps {
        &self.gaps
    }
}

impl Snapshot {
    /// The moves and the gaps the rules produce from this snapshot.
    #[must_use]
    pub fn plan(&self) -> Plan {
        let moves = self
            .cards()
            .filter_map(|card| move_of(self, card))
            .collect();
        let gaps = self.cards().flat_map(|card| gaps_of(self, card)).collect();
        Plan::new(Moves::new(moves), Gaps::new(gaps))
    }
}

fn move_of(snapshot: &Snapshot, card: &Card) -> Option<Move> {
    let destination = target(card, snapshot);
    if destination == card.status() {
        return None;
    }
    let transition = Transition::new(card.status(), destination);
    Some(Move::new(card.number(), transition, transition.reason()))
}

fn gaps_of(snapshot: &Snapshot, card: &Card) -> Vec<Gap> {
    if card.is_epic() || target(card, snapshot) != Status::Backlog {
        return Vec::new();
    }
    card.missing()
        .into_iter()
        .map(|missing| Gap::new(card.number(), missing))
        .collect()
}
