//! The use case over the in-memory Project.

use tools_core::adapters::memory::MemoryProject;
use tools_core::application::{Mode, Sync};
use tools_core::domain::Status;

use crate::support::{moves_of, number, open_card, snapshot, without_priority};

const ANSWERS: &str = "the in-memory Project answers";

fn two_ready_candidates() -> Sync<MemoryProject> {
    let cards = vec![
        open_card(20, Status::Backlog),
        open_card(21, Status::Backlog),
    ];
    Sync::new(MemoryProject::new(snapshot(cards, &[])))
}

#[test]
fn a_preview_changes_nothing() {
    let mut sync = two_ready_candidates();
    let first = sync.run(Mode::Preview).expect(ANSWERS);
    let second = sync.run(Mode::Preview).expect(ANSWERS);
    assert_eq!(moves_of(first.plan()).len(), 2);
    assert_eq!(first, second);
}

#[test]
fn an_apply_moves_every_planned_card() {
    let mut sync = two_ready_candidates();
    sync.run(Mode::Apply).expect(ANSWERS);
    let after = sync.run(Mode::Preview).expect(ANSWERS);
    assert!(after.plan().moves().is_empty());
}

#[test]
fn a_second_run_after_an_apply_plans_nothing() {
    let mut sync = two_ready_candidates();
    let first = sync.run(Mode::Apply).expect(ANSWERS);
    let second = sync.run(Mode::Apply).expect(ANSWERS);
    assert_eq!(first.plan().moves().len(), 2);
    assert!(second.plan().moves().is_empty());
}

#[test]
fn a_failed_write_stops_the_run_and_names_the_issue() {
    let cards = vec![
        open_card(20, Status::Backlog),
        open_card(21, Status::Backlog),
    ];
    let project = MemoryProject::new(snapshot(cards, &[])).refusing(vec![number(20)]);
    let mut sync = Sync::new(project);
    let failure = sync
        .run(Mode::Apply)
        .expect_err("the first write is refused");
    assert!(
        failure
            .to_string()
            .starts_with("cannot set the status of #20: ")
    );
    assert_eq!(
        moves_of(sync.run(Mode::Preview).expect(ANSWERS).plan()).len(),
        2
    );
}

#[test]
fn a_run_after_a_failed_write_finishes_the_job() {
    let cards = vec![
        open_card(20, Status::Backlog),
        open_card(21, Status::Backlog),
    ];
    let project = MemoryProject::new(snapshot(cards, &[])).refusing(vec![number(20)]);
    let mut sync = Sync::new(project);
    assert!(sync.run(Mode::Apply).is_err());
    sync.run(Mode::Apply).expect(ANSWERS);
    assert!(
        sync.run(Mode::Preview)
            .expect(ANSWERS)
            .plan()
            .moves()
            .is_empty()
    );
}

#[test]
fn the_report_lists_each_move_and_each_gap() {
    let cards = vec![
        open_card(20, Status::Backlog),
        without_priority(30, Status::Backlog),
    ];
    let mut sync = Sync::new(MemoryProject::new(snapshot(cards, &[])));
    let report = sync.run(Mode::Preview).expect(ANSWERS).to_string();
    let expected = "#20 Backlog -> Ready (gate holds)\n\
                    #30 needs triage: no Priority\n\
                    1 move (dry run, nothing written)\n";
    assert_eq!(report, expected);
}
