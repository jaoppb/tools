//! The single decision of the sync: the Status a card should have.
//!
//! The target depends only on the snapshot, so planning after the moves are
//! applied gives an empty plan. A task looks at the target of its story, not
//! its current Status, so it becomes Ready in the same run in which its story
//! moves to In review.

use crate::domain::card::Card;
use crate::domain::issue::Status;
use crate::domain::snapshot::Snapshot;

/// The Status `card` should have, given everything in `snapshot`.
#[must_use]
pub fn target(card: &Card, snapshot: &Snapshot) -> Status {
    if card.is_epic() {
        return card.status();
    }
    if card.is_closed() {
        return Status::Done;
    }
    if card.status().started() {
        return when_started(card, snapshot);
    }
    when_waiting(card, snapshot)
}

/// An In progress or In review card follows its pull request, and is never
/// demoted for a new blocker.
fn when_started(card: &Card, snapshot: &Snapshot) -> Status {
    if snapshot.closed_by_open_pull(card.number()) {
        return Status::InReview;
    }
    Status::InProgress
}

/// A Backlog, Ready or reopened card is Ready when the gate holds, or In
/// review when a pull request already closes it.
fn when_waiting(card: &Card, snapshot: &Snapshot) -> Status {
    if !gate_holds(card, snapshot) {
        return Status::Backlog;
    }
    if snapshot.closed_by_open_pull(card.number()) {
        return Status::InReview;
    }
    Status::Ready
}

/// The Ready gate: triage is complete, every blocker is closed and, for a
/// task, the story has started.
fn gate_holds(card: &Card, snapshot: &Snapshot) -> bool {
    card.missing().is_empty() && card.unblocked() && story_has_started(card, snapshot)
}

/// A card without a story, or whose story is not in the Project or is closed,
/// does not wait. Otherwise the story's own target decides.
fn story_has_started(card: &Card, snapshot: &Snapshot) -> bool {
    let Some(story) = card.story().and_then(|number| snapshot.card(number)) else {
        return true;
    };
    story.is_closed() || target(story, snapshot).started()
}
