//! `builtin:tools`: this repository's workflow, standalone issues, modelled on
//! the shapes of its real issues (AGENTS.md section 1).

use tools_core::domain::{Card, Facts, Labels, Lifecycle, Milestone, Snapshot, Standing, Status};
use tools_core::workflow::Workflow;

use crate::support::{blocked_by, number, snapshot, task_of};
use crate::workflow::support::{Remote, decision, evaluate};

fn tools() -> Workflow {
    Workflow::from_toml("[workflow]\nextends = \"builtin:tools\"\n").expect("the preset loads")
}

fn card(raw: u64, status: Status, lifecycle: Lifecycle, labels: &[&str], milestone: bool) -> Card {
    let facts = Facts::new(Labels::from_names(labels.iter().copied()));
    let facts = if milestone {
        facts.with_milestone(Milestone::new("v0.3.0"))
    } else {
        facts
    };
    Card::new(number(raw), Standing::new(status, lifecycle)).with_facts(facts)
}

/// type + area + milestone: what AGENTS.md asks of an open issue.
fn triaged(raw: u64, status: Status) -> Card {
    card(
        raw,
        status,
        Lifecycle::Open,
        &["type:feat", "area:core"],
        true,
    )
}

fn destination(board: &Snapshot, raw: u64) -> String {
    let evaluation = evaluate(&tools(), board, &Remote::default());
    decision(&evaluation, &number(raw)).destination().to_owned()
}

#[test]
fn a_triaged_issue_is_ready_without_priority_or_effort() {
    let board = snapshot(vec![triaged(1, Status::Backlog)], &[]);
    assert_eq!(destination(&board, 1), "Ready");
}

#[test]
fn every_gap_of_the_triage_keeps_the_issue_in_backlog() {
    let cases = [
        // no area label
        card(2, Status::Ready, Lifecycle::Open, &["type:feat"], true),
        // no milestone
        card(
            3,
            Status::Ready,
            Lifecycle::Open,
            &["type:feat", "area:core"],
            false,
        ),
        // no type label
        card(4, Status::Ready, Lifecycle::Open, &["area:core"], true),
        // two type labels
        card(
            5,
            Status::Ready,
            Lifecycle::Open,
            &["type:feat", "type:fix", "area:core"],
            true,
        ),
    ];
    for case in cases {
        let raw = case.number().clone();
        let board = snapshot(vec![case], &[]);
        let evaluation = evaluate(&tools(), &board, &Remote::default());
        assert_eq!(
            decision(&evaluation, &raw).destination(),
            "Backlog",
            "{raw}"
        );
    }
}

#[test]
fn an_open_blocker_holds_the_issue_back_and_a_closed_one_does_not() {
    let open = blocked_by(triaged(1, Status::Backlog), 9, Lifecycle::Open);
    let closed = blocked_by(triaged(2, Status::Backlog), 9, Lifecycle::Closed);
    let board = snapshot(vec![open, closed], &[]);
    assert_eq!(destination(&board, 1), "Backlog");
    assert_eq!(destination(&board, 2), "Ready");
}

#[test]
fn a_closed_issue_is_done_and_a_pull_request_moves_it_to_review() {
    let closed = card(1, Status::Ready, Lifecycle::Closed, &[], false);
    let board = snapshot(vec![closed, triaged(2, Status::Ready)], &[2]);
    assert_eq!(destination(&board, 1), "Done");
    assert_eq!(destination(&board, 2), "In review");
}

#[test]
fn a_started_card_keeps_going_even_if_triage_slipped() {
    let started = card(1, Status::InProgress, Lifecycle::Open, &[], false);
    let board = snapshot(vec![started], &[]);
    assert_eq!(destination(&board, 1), "In progress");
}

#[test]
fn a_stray_parent_link_changes_nothing_because_the_preset_has_no_hierarchy() {
    // The card says it is a task of #10 and #10 is a card that has not
    // started; under builtin:cosmo it would wait, here the link is ignored.
    let board = snapshot(
        vec![
            task_of(triaged(11, Status::Backlog), 10),
            triaged(10, Status::Backlog),
        ],
        &[],
    );
    assert_eq!(destination(&board, 11), "Ready");
}

#[test]
fn the_tools_preset_names_no_epic_and_no_effort_limit() {
    let workflow = tools();
    let ids = workflow.rule_ids();
    assert!(!ids.contains(&"epic-is-untouched"), "{ids:?}");
    assert_eq!(ids.len(), 6);
}
