//! The other transitions: pull requests, closing, reopening and demotion.

use tools_core::domain::{Lifecycle, Status};

use crate::support::{
    blocked_by, closed_card, epic, gaps_of, moves_of, open_card, snapshot, without_priority,
};

#[test]
fn an_epic_is_never_moved() {
    let plan = snapshot(vec![epic(5, Status::Backlog), epic(6, Status::Done)], &[5]).plan();
    assert!(moves_of(&plan).is_empty());
    assert!(gaps_of(&plan).is_empty());
}

#[test]
fn a_closed_issue_moves_to_done() {
    let plan = snapshot(vec![closed_card(20, Status::Ready)], &[]).plan();
    assert_eq!(moves_of(&plan), ["#20 Ready -> Done (closed)"]);
}

#[test]
fn a_done_card_that_is_closed_stays_done() {
    let plan = snapshot(vec![closed_card(20, Status::Done)], &[]).plan();
    assert!(moves_of(&plan).is_empty());
}

#[test]
fn an_open_pull_request_moves_a_ready_issue_to_in_review() {
    let plan = snapshot(vec![open_card(20, Status::Ready)], &[20]).plan();
    assert_eq!(
        moves_of(&plan),
        ["#20 Ready -> In review (pull request open)"]
    );
}

#[test]
fn an_open_pull_request_moves_an_in_progress_issue_to_in_review() {
    let plan = snapshot(vec![open_card(20, Status::InProgress)], &[20]).plan();
    assert_eq!(
        moves_of(&plan),
        ["#20 In progress -> In review (pull request open)"]
    );
}

#[test]
fn an_in_review_issue_without_a_pull_request_goes_back_to_in_progress() {
    let plan = snapshot(vec![open_card(20, Status::InReview)], &[]).plan();
    assert_eq!(
        moves_of(&plan),
        ["#20 In review -> In progress (pull request gone)"]
    );
}

#[test]
fn an_in_progress_issue_without_a_pull_request_stays() {
    let plan = snapshot(vec![open_card(20, Status::InProgress)], &[]).plan();
    assert!(moves_of(&plan).is_empty());
}

#[test]
fn a_reopened_issue_lands_in_backlog_when_the_gate_fails() {
    let plan = snapshot(vec![without_priority(20, Status::Done)], &[]).plan();
    assert_eq!(moves_of(&plan), ["#20 Done -> Backlog (reopened)"]);
}

#[test]
fn a_reopened_issue_lands_in_ready_when_the_gate_holds() {
    let plan = snapshot(vec![open_card(20, Status::Done)], &[]).plan();
    assert_eq!(moves_of(&plan), ["#20 Done -> Ready (reopened)"]);
}

#[test]
fn a_ready_issue_with_a_new_open_blocker_goes_back_to_backlog() {
    let card = blocked_by(open_card(20, Status::Ready), 21, Lifecycle::Open);
    let plan = snapshot(vec![card], &[]).plan();
    assert_eq!(moves_of(&plan), ["#20 Ready -> Backlog (gate lost)"]);
}

#[test]
fn a_ready_issue_that_lost_its_priority_goes_back_to_backlog() {
    let plan = snapshot(vec![without_priority(20, Status::Ready)], &[]).plan();
    assert_eq!(moves_of(&plan), ["#20 Ready -> Backlog (gate lost)"]);
    assert_eq!(gaps_of(&plan), ["#20 needs triage: no Priority"]);
}

#[test]
fn an_in_progress_issue_with_a_new_open_blocker_is_not_demoted() {
    let card = blocked_by(open_card(20, Status::InProgress), 21, Lifecycle::Open);
    let plan = snapshot(vec![card], &[]).plan();
    assert!(moves_of(&plan).is_empty());
}

#[test]
fn a_plan_never_moves_a_card_into_in_progress() {
    let cards = vec![
        open_card(1, Status::Backlog),
        open_card(2, Status::Ready),
        open_card(3, Status::Done),
        closed_card(4, Status::Backlog),
    ];
    let moves = moves_of(&snapshot(cards, &[]).plan());
    assert!(moves.iter().all(|line| !line.contains("-> In progress")));
}
