//! `builtin:cosmo` against the recorded behaviour of the old state machine:
//! the same destination for every card.

use tools_core::adapters::github::parse::parse_items;
use tools_core::domain::{Cards, Closes, Closing, Lifecycle, Snapshot};

use crate::support::{
    blocked_by, child_of_epic, closed_card, epic, open_card, snapshot, statuses, task_of,
    with_effort, without_priority,
};
use crate::workflow::support::{
    Remote, assert_destinations, cosmo, cosmo_issue, decision, evaluate, fixture_remote,
};

const LIVE: &str = include_str!("../../fixtures/items_live.json");
const EDGE: &str = include_str!("../../fixtures/items_edge.json");

fn recorded(json: &str, closing: &[u64]) -> Snapshot {
    let page = parse_items(json, &crate::support::fixture_target(), &statuses())
        .expect("the recorded page parses");
    let references = closing
        .iter()
        .map(|raw| Closes::new(cosmo_issue(1000 + *raw), cosmo_issue(*raw)))
        .collect();
    Snapshot::new(Cards::new(page.cards().to_vec()), Closing::new(references))
}

#[test]
fn the_recorded_live_page_matches() {
    assert_destinations(
        &recorded(LIVE, &[92, 20]),
        &fixture_remote(),
        &["Done", "Backlog", "Backlog", "Backlog", "In review"],
    );
    assert_destinations(
        &recorded(LIVE, &[]),
        &fixture_remote(),
        &["Done", "Backlog", "Backlog", "Backlog", "In progress"],
    );
}

#[test]
fn the_recorded_edge_page_matches() {
    assert_destinations(
        &recorded(EDGE, &[]),
        &fixture_remote(),
        &[
            "Ready", "Done", "Ready", "Backlog", "Ready", "Backlog", "Ready",
        ],
    );
}

#[test]
fn the_edge_page_with_every_blocker_open_matches() {
    let open = EDGE.replace("\"state\": \"CLOSED\"", "\"state\": \"OPEN\"");
    assert_destinations(
        &recorded(&open, &[]),
        &fixture_remote(),
        &[
            "Ready", "Ready", "Backlog", "Backlog", "Ready", "Backlog", "Ready",
        ],
    );
}

#[test]
fn transitions_match() {
    let open_blocker = |card| blocked_by(card, 21, Lifecycle::Open);
    let scenarios: Vec<(Vec<_>, &[u64], &[&str])> = vec![
        (
            vec![epic(5, "Backlog"), epic(6, "Done")],
            &[5],
            &["Backlog", "Done"],
        ),
        (vec![closed_card(20, "Ready")], &[], &["Done"]),
        (vec![closed_card(20, "Done")], &[], &["Done"]),
        (vec![open_card(20, "Ready")], &[20], &["In review"]),
        (vec![open_card(20, "In progress")], &[20], &["In review"]),
        (vec![open_card(20, "In review")], &[], &["In progress"]),
        (vec![open_card(20, "In progress")], &[], &["In progress"]),
        (vec![without_priority(20, "Done")], &[], &["Backlog"]),
        (vec![open_card(20, "Done")], &[], &["Ready"]),
        (
            vec![open_blocker(open_card(20, "Ready"))],
            &[],
            &["Backlog"],
        ),
        (vec![without_priority(20, "Ready")], &[], &["Backlog"]),
        (
            vec![open_blocker(open_card(20, "In progress"))],
            &[],
            &["In progress"],
        ),
        (vec![open_card(20, "Backlog")], &[], &["Ready"]),
        (vec![without_priority(20, "Backlog")], &[], &["Backlog"]),
        (vec![with_effort(25, 13)], &[], &["Backlog"]),
        (vec![with_effort(25, 8)], &[], &["Ready"]),
        (
            vec![open_blocker(open_card(20, "Backlog"))],
            &[],
            &["Backlog"],
        ),
        (
            vec![blocked_by(open_card(20, "Backlog"), 21, Lifecycle::Closed)],
            &[],
            &["Ready"],
        ),
        (vec![open_card(20, "Backlog")], &[20], &["In review"]),
    ];
    for (cards, closing, expected) in scenarios {
        assert_destinations(&snapshot(cards, closing), &Remote::default(), expected);
    }
}

#[test]
fn hierarchy_matches() {
    let scenarios: Vec<(Vec<_>, &[&str])> = vec![
        (
            vec![
                open_card(10, "Ready"),
                task_of(open_card(11, "Backlog"), 10),
            ],
            &["Ready", "Backlog"],
        ),
        (
            vec![
                open_card(10, "In progress"),
                task_of(open_card(11, "Backlog"), 10),
            ],
            &["In progress", "Ready"],
        ),
        (
            vec![
                open_card(10, "Ready"),
                task_of(open_card(11, "Backlog"), 10),
            ],
            &["In review", "Ready"],
        ),
        (
            vec![
                closed_card(10, "Done"),
                task_of(open_card(11, "Backlog"), 10),
            ],
            &["Done", "Ready"],
        ),
        (
            vec![
                epic(5, "Backlog"),
                child_of_epic(open_card(6, "Backlog"), 5),
            ],
            &["Backlog", "Ready"],
        ),
        (
            vec![
                open_card(10, "Backlog"),
                task_of(open_card(11, "Backlog"), 10),
                task_of(open_card(12, "Backlog"), 11),
            ],
            &["Ready", "Backlog", "Backlog"],
        ),
    ];
    let closing: [&[u64]; 6] = [&[], &[], &[10], &[], &[], &[]];
    for ((cards, expected), closing) in scenarios.into_iter().zip(closing) {
        assert_destinations(&snapshot(cards, closing), &Remote::default(), expected);
    }
}

#[test]
fn explain_of_a_fixture_issue_names_the_rule_that_held_it_back() {
    let evaluation = evaluate(&cosmo(), &recorded(LIVE, &[92, 20]), &fixture_remote());
    let text = decision(&evaluation, &cosmo_issue(20))
        .explain()
        .to_string();
    assert!(text.starts_with("cosmo#20: Backlog -> Backlog"), "{text}");
    assert!(text.contains("unblocked = false"), "{text}");
    assert!(text.contains("is_epic = false"), "{text}");
    assert!(text.contains("[FIRED      ] gate-closed"), "{text}");
}
