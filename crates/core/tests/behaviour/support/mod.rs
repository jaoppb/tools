//! Builders for the snapshots the tests need.

pub mod audit;

use tools_core::domain::{
    Blocker, Blockers, Card, Cards, Closing, Effort, Facts, IssueNumber, Label, Labels, Lifecycle,
    Links, Milestone, Parent, Plan, Priority, Role, Snapshot, Standing, Status,
};

/// The issue with this number.
#[must_use]
pub const fn number(raw: u64) -> IssueNumber {
    IssueNumber::new(raw)
}

fn labels(names: &[&str]) -> Labels {
    Labels::new(names.iter().map(|name| Label::new(name)).collect())
}

/// Facts that pass the Ready gate: one type, an area, a milestone, a Priority
/// and an Effort of 3.
#[must_use]
pub fn complete_facts() -> Facts {
    Facts::new(labels(&["type:feat", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
        .with_priority(Priority::new("P1"))
        .with_effort(Effort::new(3))
}

/// An open card with complete facts and no links.
#[must_use]
pub fn open_card(raw: u64, status: Status) -> Card {
    Card::new(number(raw), Standing::new(status, Lifecycle::Open)).with_facts(complete_facts())
}

/// A closed card with complete facts and no links.
#[must_use]
pub fn closed_card(raw: u64, status: Status) -> Card {
    Card::new(number(raw), Standing::new(status, Lifecycle::Closed)).with_facts(complete_facts())
}

/// An open epic.
#[must_use]
pub fn epic(raw: u64, status: Status) -> Card {
    let facts = Facts::new(labels(&["epic", "type:feat", "area:tooling"]));
    Card::new(number(raw), Standing::new(status, Lifecycle::Open)).with_facts(facts)
}

/// An open card that has everything but a Priority.
#[must_use]
pub fn without_priority(raw: u64, status: Status) -> Card {
    let facts = Facts::new(labels(&["type:feat", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
        .with_effort(Effort::new(3));
    Card::new(number(raw), Standing::new(status, Lifecycle::Open)).with_facts(facts)
}

/// An open card with complete facts but this Effort.
#[must_use]
pub fn with_effort(raw: u64, points: u8) -> Card {
    let facts = Facts::new(labels(&["type:feat", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
        .with_priority(Priority::new("P1"))
        .with_effort(Effort::new(points));
    Card::new(number(raw), Standing::new(Status::Backlog, Lifecycle::Open)).with_facts(facts)
}

/// This card, blocked by one issue in this lifecycle.
#[must_use]
pub fn blocked_by(card: Card, blocker: u64, lifecycle: Lifecycle) -> Card {
    let blockers = Blockers::new(vec![Blocker::new(number(blocker), lifecycle)]);
    card.with_links(Links::new(blockers))
}

/// This card, a task of the story with this number.
#[must_use]
pub fn task_of(card: Card, story: u64) -> Card {
    child_of(card, story, Role::Story)
}

/// This card, a child of the epic with this number.
#[must_use]
pub fn child_of_epic(card: Card, epic_number: u64) -> Card {
    child_of(card, epic_number, Role::Epic)
}

fn child_of(card: Card, parent: u64, role: Role) -> Card {
    let links = Links::default().with_parent(Parent::new(number(parent), role));
    card.with_links(links)
}

/// A snapshot of these cards, where open pull requests close these issues.
#[must_use]
pub fn snapshot(cards: Vec<Card>, closing: &[u64]) -> Snapshot {
    let issues = closing.iter().map(|raw| number(*raw)).collect();
    Snapshot::new(Cards::new(cards), Closing::new(issues))
}

/// The moves of a plan, as the report prints them.
#[must_use]
pub fn moves_of(plan: &Plan) -> Vec<String> {
    plan.moves().iter().map(ToString::to_string).collect()
}

/// The gaps of a plan, as the report prints them.
#[must_use]
pub fn gaps_of(plan: &Plan) -> Vec<String> {
    plan.gaps().iter().map(ToString::to_string).collect()
}
