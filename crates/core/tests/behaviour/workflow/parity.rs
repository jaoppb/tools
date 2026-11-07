//! `builtin:cosmo` against the state machine: same destination for every card.

use tools_core::adapters::github::Target;
use tools_core::adapters::github::parse::parse_items;
use tools_core::domain::{Cards, Closing, IssueNumber, Lifecycle, Snapshot, Status};

use crate::support::{
    blocked_by, child_of_epic, closed_card, epic, open_card, snapshot, task_of, with_effort,
    without_priority,
};
use crate::workflow::support::{Remote, assert_parity, outside};

const LIVE: &str = include_str!("../../fixtures/items_live.json");
const EDGE: &str = include_str!("../../fixtures/items_edge.json");

fn cosmo_issue(raw: u64) -> IssueNumber {
    IssueNumber::new(tools_core::domain::Repo::new("jaoppb", "cosmo"), raw)
}

fn recorded(json: &str, closing: &[u64]) -> Snapshot {
    let page = parse_items(json, &Target::cosmo()).expect("the recorded page parses");
    let issues = closing.iter().map(|raw| cosmo_issue(*raw)).collect();
    Snapshot::new(Cards::new(page.cards().to_vec()), Closing::new(issues))
}

/// The parents of the fixtures that are not cards of the Project.
fn fixture_remote() -> Remote {
    Remote::default()
        .with(outside(cosmo_issue(2), &["epic"]))
        .with(outside(cosmo_issue(91), &["type:feat"]))
        .with(outside(cosmo_issue(107), &["epic"]))
}

#[test]
fn the_recorded_live_page_matches() {
    assert_parity(&recorded(LIVE, &[92, 20]), &fixture_remote());
    assert_parity(&recorded(LIVE, &[]), &fixture_remote());
}

#[test]
fn the_recorded_edge_page_matches() {
    assert_parity(&recorded(EDGE, &[]), &fixture_remote());
}

#[test]
fn the_edge_page_with_every_blocker_open_matches() {
    let open = EDGE.replace("\"state\": \"CLOSED\"", "\"state\": \"OPEN\"");
    assert_parity(&recorded(&open, &[]), &fixture_remote());
}

#[test]
fn transitions_match() {
    let scenarios: Vec<(Vec<_>, &[u64])> = vec![
        (vec![epic(5, Status::Backlog), epic(6, Status::Done)], &[5]),
        (vec![closed_card(20, Status::Ready)], &[]),
        (vec![closed_card(20, Status::Done)], &[]),
        (vec![open_card(20, Status::Ready)], &[20]),
        (vec![open_card(20, Status::InProgress)], &[20]),
        (vec![open_card(20, Status::InReview)], &[]),
        (vec![open_card(20, Status::InProgress)], &[]),
        (vec![without_priority(20, Status::Done)], &[]),
        (vec![open_card(20, Status::Done)], &[]),
        (
            vec![blocked_by(
                open_card(20, Status::Ready),
                21,
                Lifecycle::Open,
            )],
            &[],
        ),
        (vec![without_priority(20, Status::Ready)], &[]),
        (
            vec![blocked_by(
                open_card(20, Status::InProgress),
                21,
                Lifecycle::Open,
            )],
            &[],
        ),
        (vec![open_card(20, Status::Backlog)], &[]),
        (vec![without_priority(20, Status::Backlog)], &[]),
        (vec![with_effort(25, 13)], &[]),
        (vec![with_effort(25, 8)], &[]),
        (
            vec![blocked_by(
                open_card(20, Status::Backlog),
                21,
                Lifecycle::Open,
            )],
            &[],
        ),
        (
            vec![blocked_by(
                open_card(20, Status::Backlog),
                21,
                Lifecycle::Closed,
            )],
            &[],
        ),
        (vec![open_card(20, Status::Backlog)], &[20]),
    ];
    for (cards, closing) in scenarios {
        assert_parity(&snapshot(cards, closing), &Remote::default());
    }
}

#[test]
fn hierarchy_matches() {
    let scenarios: Vec<(Vec<_>, &[u64])> = vec![
        (
            vec![
                open_card(10, Status::Ready),
                task_of(open_card(11, Status::Backlog), 10),
            ],
            &[],
        ),
        (
            vec![
                open_card(10, Status::InProgress),
                task_of(open_card(11, Status::Backlog), 10),
            ],
            &[],
        ),
        (
            vec![
                open_card(10, Status::Ready),
                task_of(open_card(11, Status::Backlog), 10),
            ],
            &[10],
        ),
        (
            vec![
                closed_card(10, Status::Done),
                task_of(open_card(11, Status::Backlog), 10),
            ],
            &[],
        ),
        (
            vec![
                epic(5, Status::Backlog),
                child_of_epic(open_card(6, Status::Backlog), 5),
            ],
            &[],
        ),
        (
            vec![
                open_card(10, Status::Backlog),
                task_of(open_card(11, Status::Backlog), 10),
                task_of(open_card(12, Status::Backlog), 11),
            ],
            &[],
        ),
    ];
    for (cards, closing) in scenarios {
        assert_parity(&snapshot(cards, closing), &Remote::default());
    }
}

#[test]
fn explain_of_a_fixture_issue_names_the_rule_that_held_it_back() {
    let evaluation = crate::workflow::support::evaluate(
        &crate::workflow::support::cosmo(),
        &recorded(LIVE, &[92, 20]),
        &fixture_remote(),
    );
    let text = crate::workflow::support::decision(&evaluation, &cosmo_issue(20))
        .explain()
        .to_string();
    eprintln!("{text}");
    assert!(text.starts_with("cosmo#20: Backlog -> Backlog"), "{text}");
    assert!(text.contains("unblocked = false"), "{text}");
    assert!(text.contains("is_epic = false"), "{text}");
    assert!(text.contains("[FIRED      ] gate-closed"), "{text}");
}
