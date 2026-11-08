//! `builtin:tools`: this repository's workflow, standalone issues, modelled on
//! the shapes of its real issues (AGENTS.md section 1).

use tools_core::domain::{Card, Facts, Labels, Lifecycle, Milestone, Snapshot, Standing};
use tools_core::workflow::Workflow;

use crate::support::{blocked_by, number, snapshot, status_name, task_of};
use crate::workflow::support::{Remote, decision, evaluate};

fn tools() -> Workflow {
    Workflow::from_toml("[workflow]\nextends = \"builtin:tools\"\n").expect("the preset loads")
}

fn card(raw: u64, status: &str, lifecycle: Lifecycle, labels: &[&str], milestone: bool) -> Card {
    let facts = Facts::new(Labels::from_names(labels.iter().copied()));
    let facts = if milestone {
        facts.with_milestone(Milestone::new("v0.3.0"))
    } else {
        facts
    };
    Card::new(number(raw), Standing::new(status_name(status), lifecycle)).with_facts(facts)
}

/// type + area + milestone: what AGENTS.md asks of an open issue.
fn triaged(raw: u64, status: &str) -> Card {
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
    decision(&evaluation, &number(raw))
        .destination()
        .to_string()
}

#[test]
fn a_triaged_issue_is_ready_without_priority_or_effort() {
    let board = snapshot(vec![triaged(1, "Backlog")], &[]);
    assert_eq!(destination(&board, 1), "Ready");
}

#[test]
fn every_gap_of_the_triage_keeps_the_issue_in_backlog() {
    let cases = [
        // no area label
        card(2, "Ready", Lifecycle::Open, &["type:feat"], true),
        // no milestone
        card(
            3,
            "Ready",
            Lifecycle::Open,
            &["type:feat", "area:core"],
            false,
        ),
        // no type label
        card(4, "Ready", Lifecycle::Open, &["area:core"], true),
        // two type labels
        card(
            5,
            "Ready",
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
    let open = blocked_by(triaged(1, "Backlog"), 9, Lifecycle::Open);
    let closed = blocked_by(triaged(2, "Backlog"), 9, Lifecycle::Closed);
    let board = snapshot(vec![open, closed], &[]);
    assert_eq!(destination(&board, 1), "Backlog");
    assert_eq!(destination(&board, 2), "Ready");
}

#[test]
fn a_closed_issue_is_done_and_a_pull_request_moves_it_to_review() {
    let closed = card(1, "Ready", Lifecycle::Closed, &[], false);
    let board = snapshot(vec![closed, triaged(2, "Ready")], &[2]);
    assert_eq!(destination(&board, 1), "Done");
    assert_eq!(destination(&board, 2), "In review");
}

#[test]
fn a_started_card_keeps_going_even_if_triage_slipped() {
    let started = card(1, "In progress", Lifecycle::Open, &[], false);
    let board = snapshot(vec![started], &[]);
    assert_eq!(destination(&board, 1), "In progress");
}

#[test]
fn a_stray_parent_link_changes_nothing_because_the_preset_has_no_hierarchy() {
    // The card says it is a task of #10 and #10 is a card that has not
    // started; under builtin:cosmo it would wait, here the link is ignored.
    let board = snapshot(
        vec![task_of(triaged(11, "Backlog"), 10), triaged(10, "Backlog")],
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

#[test]
fn an_area_prefix_list_accepts_each_prefix_and_ignores_case_and_accents() {
    let workflow = Workflow::from_toml(
        "[workflow]\nextends = \"builtin:tools\"\n[workflow.params]\narea_prefix = [\"area:\", \"domain:\"]\n",
    )
    .expect("a list param loads");
    let ready = |label: &str| {
        let board = snapshot(
            vec![card(
                1,
                "Backlog",
                Lifecycle::Open,
                &["type:feat", label],
                true,
            )],
            &[],
        );
        let evaluation = evaluate(&workflow, &board, &Remote::default());
        decision(&evaluation, &number(1)).destination().to_string()
    };
    assert_eq!(ready("domain:web"), "Ready");
    assert_eq!(ready("Área:core"), "Ready");
    assert_eq!(ready("zone:web"), "Backlog");
}
