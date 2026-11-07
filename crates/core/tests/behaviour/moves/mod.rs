//! The use case over the in-memory Project.

use tools_core::adapters::memory::MemoryProject;
use tools_core::application::{Mode, Sync};

use crate::support::{number, open_card, snapshot, task_of};
use crate::workflow::support::cosmo;

const ANSWERS: &str = "the in-memory Project answers";

fn two_ready_candidates() -> Sync<MemoryProject> {
    let cards = vec![open_card(20, "Backlog"), open_card(21, "Backlog")];
    Sync::new(MemoryProject::new(snapshot(cards, &[])), cosmo())
}

#[test]
fn a_preview_changes_nothing() {
    let mut sync = two_ready_candidates();
    let first = sync.run(Mode::Preview).expect(ANSWERS);
    let second = sync.run(Mode::Preview).expect(ANSWERS);
    assert_eq!(first.moved(), 2);
    assert_eq!(first.to_string(), second.to_string());
}

#[test]
fn an_apply_moves_every_planned_card() {
    let mut sync = two_ready_candidates();
    sync.run(Mode::Apply).expect(ANSWERS);
    let after = sync.run(Mode::Preview).expect(ANSWERS);
    assert_eq!(after.moved(), 0);
    assert_eq!(after.kept(), 2);
}

#[test]
fn a_second_run_after_an_apply_plans_nothing() {
    let mut sync = two_ready_candidates();
    let first = sync.run(Mode::Apply).expect(ANSWERS);
    let second = sync.run(Mode::Apply).expect(ANSWERS);
    assert_eq!(first.moved(), 2);
    assert_eq!(second.moved(), 0);
}

#[test]
fn a_failed_write_stops_the_run_and_names_the_issue() {
    let cards = vec![open_card(20, "Backlog"), open_card(21, "Backlog")];
    let project = MemoryProject::new(snapshot(cards, &[])).refusing(vec![number(20)]);
    let mut sync = Sync::new(project, cosmo());
    let failure = sync
        .run(Mode::Apply)
        .expect_err("the first write is refused");
    assert!(
        failure
            .to_string()
            .starts_with("cannot set the status of #20: ")
    );
    assert_eq!(sync.run(Mode::Preview).expect(ANSWERS).moved(), 2);
}

#[test]
fn a_run_after_a_failed_write_finishes_the_job() {
    let cards = vec![open_card(20, "Backlog"), open_card(21, "Backlog")];
    let project = MemoryProject::new(snapshot(cards, &[])).refusing(vec![number(20)]);
    let mut sync = Sync::new(project, cosmo());
    assert!(sync.run(Mode::Apply).is_err());
    sync.run(Mode::Apply).expect(ANSWERS);
    assert_eq!(sync.run(Mode::Preview).expect(ANSWERS).moved(), 0);
}

#[test]
fn the_report_lists_each_move_and_each_frozen_card_and_counts_them() {
    // #11 is a task whose story #10 is not a card and the in-memory Project
    // cannot read it: frozen, not guessed.
    let cards = vec![
        open_card(20, "Backlog"),
        open_card(21, "Ready"),
        task_of(open_card(11, "Backlog"), 10),
    ];
    let mut sync = Sync::new(MemoryProject::new(snapshot(cards, &[])), cosmo());
    let outcome = sync.run(Mode::Preview).expect(ANSWERS);
    let expected = "#20 Backlog -> Ready (gate-holds)\n\
                    #11 frozen: parent #10 cannot be read: not found\n\
                    1 kept, 1 moved, 1 frozen (check, nothing written)\n";
    assert_eq!(outcome.to_string(), expected);
    assert!(outcome.pending());
}

#[test]
fn an_apply_report_does_not_say_check() {
    let mut sync = two_ready_candidates();
    let outcome = sync.run(Mode::Apply).expect(ANSWERS);
    assert!(outcome.to_string().ends_with("0 kept, 2 moved, 0 frozen\n"));
}

#[test]
fn a_board_that_needs_nothing_is_not_pending() {
    let cards = vec![open_card(21, "Ready")];
    let mut sync = Sync::new(MemoryProject::new(snapshot(cards, &[])), cosmo());
    assert!(!sync.run(Mode::Preview).expect(ANSWERS).pending());
}
