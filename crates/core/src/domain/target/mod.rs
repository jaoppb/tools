//! The single decision of the sync: the Status a card should have.
//!
//! The target depends only on the snapshot, so planning after the moves are
//! applied gives an empty plan. A task looks at the target of its story, not
//! its current Status, so it becomes Ready in the same run in which its story
//! moves to In review.

use crate::config::RulesConfig;
use crate::domain::card::Card;
use crate::domain::issue::Status;
use crate::domain::snapshot::Snapshot;

/// The Status `card` should have, given everything in `snapshot` under default rules.
#[must_use]
pub fn target(card: &Card, snapshot: &Snapshot) -> Status {
    target_with(card, snapshot, &RulesConfig::default())
}

/// The Status `card` should have, given everything in `snapshot` under `rules`.
#[must_use]
pub fn target_with(card: &Card, snapshot: &Snapshot, rules: &RulesConfig) -> Status {
    if waits(card) {
        if on_parent_cycle(card, snapshot) {
            return card.status();
        }
        return when_waiting(card, snapshot, rules);
    }
    if card.is_epic() {
        return card.status();
    }
    if card.is_closed() {
        return Status::Done;
    }
    when_started(card, snapshot)
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
fn when_waiting(card: &Card, snapshot: &Snapshot, rules: &RulesConfig) -> Status {
    if !gate_holds(card, snapshot, rules) {
        return Status::Backlog;
    }
    if snapshot.closed_by_open_pull(card.number()) {
        return Status::InReview;
    }
    Status::Ready
}

/// The Ready gate: triage is complete, every blocker is closed and, for a
/// task, the story has started.
fn gate_holds(card: &Card, snapshot: &Snapshot, rules: &RulesConfig) -> bool {
    card.missing_for(rules).is_empty()
        && card.unblocked()
        && story_has_started(card, snapshot, rules)
}

/// A card without a story, or whose story is not in the Project or is closed,
/// does not wait. Otherwise the story's own target decides.
fn story_has_started(card: &Card, snapshot: &Snapshot, rules: &RulesConfig) -> bool {
    let Some(story) = card.story().and_then(|number| snapshot.card(number)) else {
        return true;
    };
    story.is_closed() || target_with(story, snapshot, rules).started()
}

/// Whether `card` waits on a story chain that loops, on a cycle or into one.
///
/// Only a waiting card looks at its story, and the story's own target looks at
/// its story only while it waits too: the walk stops at an epic, a closed or a
/// started card, exactly where the recursion of [`target_with`] stops. A card
/// that waits on a loop has no target; the sync leaves it unchanged and
/// reports it.
#[must_use]
pub(crate) fn on_parent_cycle(card: &Card, snapshot: &Snapshot) -> bool {
    // The card itself is seeded, so a story that is its own parent is caught.
    let mut seen = vec![card.number()];
    let mut current = card;
    while waits(current) {
        let Some(story) = current.story().and_then(|number| snapshot.card(number)) else {
            return false;
        };
        if seen.contains(&story.number()) {
            return true;
        }
        seen.push(story.number());
        current = story;
    }
    false
}

/// Whether the target of `card` is decided by the gate, and so by its story.
/// [`target_with`] branches on it first; every other card settles alone.
fn waits(card: &Card) -> bool {
    !card.is_epic() && !card.is_closed() && !card.status().started()
}
