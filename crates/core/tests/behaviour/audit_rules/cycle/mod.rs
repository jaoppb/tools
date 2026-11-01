//! No blocked-by cycle among open issues.

use crate::support::audit::{blocked_by, closed, epic, lines, story, survey};

#[test]
fn reports_two_issues_that_block_each_other_once() {
    let broken = survey(vec![
        epic(1),
        blocked_by(story(10, 1), &[11]),
        blocked_by(story(11, 1), &[10]),
    ]);
    assert_eq!(
        lines(&broken),
        ["#10 blocked-by-cycle: blocked-by cycle: #10, #11"]
    );
    let repaired = survey(vec![epic(1), blocked_by(story(10, 1), &[11]), story(11, 1)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn reports_a_three_issue_cycle_once_with_all_members() {
    let broken = survey(vec![
        epic(1),
        blocked_by(story(12, 1), &[10]),
        blocked_by(story(10, 1), &[11]),
        blocked_by(story(11, 1), &[12]),
    ]);
    assert_eq!(
        lines(&broken),
        ["#10 blocked-by-cycle: blocked-by cycle: #10, #11, #12"]
    );
}

#[test]
fn reports_an_issue_that_blocks_itself() {
    let broken = survey(vec![epic(1), blocked_by(story(10, 1), &[10])]);
    assert_eq!(
        lines(&broken),
        ["#10 blocked-by-cycle: blocked-by cycle: #10"]
    );
}

#[test]
fn reports_only_the_members_of_the_cycle_and_not_the_issue_waiting_on_it() {
    let broken = survey(vec![
        epic(1),
        blocked_by(story(20, 1), &[10]),
        blocked_by(story(10, 1), &[11]),
        blocked_by(story(11, 1), &[10]),
    ]);
    assert_eq!(
        lines(&broken),
        ["#10 blocked-by-cycle: blocked-by cycle: #10, #11"]
    );
}

#[test]
fn ignores_a_chain_with_no_cycle() {
    let chain = survey(vec![
        epic(1),
        blocked_by(story(10, 1), &[11]),
        blocked_by(story(11, 1), &[12]),
        story(12, 1),
    ]);
    assert!(lines(&chain).is_empty());
}

#[test]
fn ignores_a_cycle_closed_by_a_closed_issue() {
    let history = survey(vec![
        epic(1),
        blocked_by(story(10, 1), &[11]),
        blocked_by(closed(11, 1), &[10]),
    ]);
    assert!(lines(&history).is_empty());
}

#[test]
fn reports_a_cross_repository_blocked_by_cycle() {
    use crate::support::audit::labels;
    use crate::support::repo_number;
    use tools_core::domain::{BlockedBy, Lifecycle, Milestone, TreeIssue};

    let app_epic = TreeIssue::new(repo_number("app", 1), Lifecycle::Open)
        .with_labels(labels(&["epic", "type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1"));
    let web_epic = TreeIssue::new(repo_number("web", 1), Lifecycle::Open)
        .with_labels(labels(&["epic", "type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1"));

    let app_story = TreeIssue::new(repo_number("app", 10), Lifecycle::Open)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
        .with_parent(repo_number("app", 1))
        .with_blocked_by(BlockedBy::new(vec![repo_number("web", 20)]));

    let web_story = TreeIssue::new(repo_number("web", 20), Lifecycle::Open)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
        .with_parent(repo_number("web", 1))
        .with_blocked_by(BlockedBy::new(vec![repo_number("app", 10)]));

    let broken = survey(vec![app_epic, web_epic, app_story, web_story]);
    assert_eq!(
        lines(&broken),
        ["app#10 blocked-by-cycle: blocked-by cycle: app#10, web#20"]
    );
}
