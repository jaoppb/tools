//! The Ready gate: what lets a Backlog issue become Ready.

use tools_core::domain::{Lifecycle, Status};

use crate::support::{
    blocked_by, child_of_epic, epic, gaps_of, moves_of, open_card, snapshot, task_of, with_effort,
    without_priority,
};

#[test]
fn a_backlog_issue_with_everything_set_becomes_ready() {
    let plan = snapshot(vec![open_card(20, Status::Backlog)], &[]).plan();
    assert_eq!(moves_of(&plan), ["#20 Backlog -> Ready (gate holds)"]);
    assert!(gaps_of(&plan).is_empty());
}

#[test]
fn an_issue_without_a_priority_stays_in_backlog_and_is_reported() {
    let plan = snapshot(vec![without_priority(20, Status::Backlog)], &[]).plan();
    assert!(moves_of(&plan).is_empty());
    assert_eq!(gaps_of(&plan), ["#20 needs triage: no Priority"]);
}

#[test]
fn an_effort_above_eight_stays_in_backlog_and_is_reported() {
    let plan = snapshot(vec![with_effort(25, 13)], &[]).plan();
    assert!(moves_of(&plan).is_empty());
    assert_eq!(
        gaps_of(&plan),
        ["#25 needs triage: Effort above 8, split the story"]
    );
}

#[test]
fn an_open_blocker_keeps_an_issue_in_backlog() {
    let card = blocked_by(open_card(20, Status::Backlog), 21, Lifecycle::Open);
    let plan = snapshot(vec![card], &[]).plan();
    assert!(moves_of(&plan).is_empty());
    assert!(gaps_of(&plan).is_empty());
}

#[test]
fn a_blocker_closed_as_not_planned_does_not_block() {
    let card = blocked_by(open_card(20, Status::Backlog), 21, Lifecycle::Closed);
    let plan = snapshot(vec![card], &[]).plan();
    assert_eq!(moves_of(&plan), ["#20 Backlog -> Ready (gate holds)"]);
}

#[test]
fn a_task_waits_until_its_story_has_started() {
    let waiting = snapshot(
        vec![
            open_card(10, Status::Ready),
            task_of(open_card(11, Status::Backlog), 10),
        ],
        &[],
    );
    assert!(moves_of(&waiting.plan()).is_empty());
    let started = snapshot(
        vec![
            open_card(10, Status::InProgress),
            task_of(open_card(11, Status::Backlog), 10),
        ],
        &[],
    );
    assert_eq!(
        moves_of(&started.plan()),
        ["#11 Backlog -> Ready (gate holds)"]
    );
}

#[test]
fn a_task_does_not_wait_for_an_epic() {
    let cards = vec![
        epic(5, Status::Backlog),
        child_of_epic(open_card(6, Status::Backlog), 5),
    ];
    let plan = snapshot(cards, &[]).plan();
    assert_eq!(moves_of(&plan), ["#6 Backlog -> Ready (gate holds)"]);
}

#[test]
fn a_task_becomes_ready_in_the_same_run_its_story_moves_to_in_review() {
    let cards = vec![
        open_card(10, Status::Ready),
        task_of(open_card(11, Status::Backlog), 10),
    ];
    let plan = snapshot(cards, &[10]).plan();
    assert_eq!(
        moves_of(&plan),
        [
            "#10 Ready -> In review (pull request open)",
            "#11 Backlog -> Ready (gate holds)"
        ]
    );
}
