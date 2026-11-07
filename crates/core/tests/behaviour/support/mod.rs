//! Builders for the snapshots the tests need.

pub mod audit;

use tools_core::adapters::github::Target;
use tools_core::domain::{
    Blocker, Blockers, Card, Cards, Closes, Closing, Effort, Facts, IssueNumber, Label, Labels,
    Lifecycle, Links, Milestone, Parent, Priority, Repo, Role, Snapshot, Standing, StatusName,
    Statuses,
};

/// The Project the recorded fixtures were taken from: project 4 of `jaoppb`,
/// linked to `jaoppb/cosmo`. The name is data of the fixtures, not of the code.
#[must_use]
pub fn fixture_target() -> Target {
    Target::single("jaoppb", "cosmo", Some(4))
}

/// The statuses of the board the fixtures were recorded from. The names are
/// data of the fixtures, not of the code.
#[must_use]
pub fn statuses() -> Statuses {
    Statuses::new(
        &["Backlog", "Ready", "In progress", "In review", "Done"],
        "Backlog",
    )
    .expect("the fixture board is valid")
}

/// The declared status with this name.
#[must_use]
pub fn status_name(name: &str) -> StatusName {
    statuses()
        .parse(name)
        .expect("the fixture board declares it")
}

/// The issue with this number in the test repo.
#[must_use]
pub fn number(raw: u64) -> IssueNumber {
    IssueNumber::new(Repo::new("", ""), raw)
}

/// The issue with this number in the named repository.
#[must_use]
pub fn repo_number(repo: &str, raw: u64) -> IssueNumber {
    IssueNumber::new(Repo::new("", repo), raw)
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
pub fn open_card(raw: u64, status: &str) -> Card {
    Card::new(
        number(raw),
        Standing::new(status_name(status), Lifecycle::Open),
    )
    .with_facts(complete_facts())
}

/// An open card with complete facts in a named repository.
#[must_use]
pub fn repo_card(repo: &str, raw: u64, status: &str) -> Card {
    Card::new(
        repo_number(repo, raw),
        Standing::new(status_name(status), Lifecycle::Open),
    )
    .with_facts(complete_facts())
}

/// A closed card with complete facts and no links.
#[must_use]
pub fn closed_card(raw: u64, status: &str) -> Card {
    Card::new(
        number(raw),
        Standing::new(status_name(status), Lifecycle::Closed),
    )
    .with_facts(complete_facts())
}

/// An open epic.
#[must_use]
pub fn epic(raw: u64, status: &str) -> Card {
    let facts = Facts::new(labels(&["epic", "type:feat", "area:tooling"]));
    Card::new(
        number(raw),
        Standing::new(status_name(status), Lifecycle::Open),
    )
    .with_facts(facts)
}

/// An open card that has everything but a Priority.
#[must_use]
pub fn without_priority(raw: u64, status: &str) -> Card {
    let facts = Facts::new(labels(&["type:feat", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
        .with_effort(Effort::new(3));
    Card::new(
        number(raw),
        Standing::new(status_name(status), Lifecycle::Open),
    )
    .with_facts(facts)
}

/// An open card with complete facts but this Effort.
#[must_use]
pub fn with_effort(raw: u64, points: u8) -> Card {
    let facts = Facts::new(labels(&["type:feat", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
        .with_priority(Priority::new("P1"))
        .with_effort(Effort::new(points));
    Card::new(
        number(raw),
        Standing::new(status_name("Backlog"), Lifecycle::Open),
    )
    .with_facts(facts)
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

/// A snapshot of these cards, where an open pull request (numbered 1000 + the
/// issue) closes each of these issues.
#[must_use]
pub fn snapshot(cards: Vec<Card>, closing: &[u64]) -> Snapshot {
    let references = closing
        .iter()
        .map(|raw| Closes::new(number(1000 + *raw), number(*raw)))
        .collect();
    Snapshot::new(Cards::new(cards), Closing::new(references))
}
