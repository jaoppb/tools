//! An issue never sits under, or blocks, an issue of an earlier release.

use tools_core::domain::TreeIssue;

use crate::support::audit::{blocked_by, closed, epic, in_milestone, lines, story, survey};

fn later_epic(raw: u64) -> TreeIssue {
    in_milestone(epic(raw), "v1.1")
}

#[test]
fn reports_a_v1_1_issue_under_a_v1_parent() {
    let broken = survey(vec![epic(1), in_milestone(story(10, 1), "v1.1")]);
    assert_eq!(
        lines(&broken),
        ["#10 parent-earlier-release: in a later release than its parent #1"]
    );
    let repaired = survey(vec![epic(1), story(10, 1)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn reports_a_v1_2_issue_under_a_v1_1_parent() {
    let parent = in_milestone(epic(1), "v1.1");
    let broken = survey(vec![parent, in_milestone(story(10, 1), "v1.2")]);
    assert_eq!(
        lines(&broken),
        ["#10 parent-earlier-release: in a later release than its parent #1"]
    );
}

#[test]
fn does_not_report_a_v1_issue_under_a_v1_1_parent() {
    let parent = in_milestone(epic(1), "v1.1");
    assert!(lines(&survey(vec![parent, story(10, 1)])).is_empty());
}

#[test]
fn reports_a_v1_1_issue_that_blocks_a_v1_issue() {
    let blocker = in_milestone(story(11, 2), "v1.1");
    let broken = survey(vec![
        epic(1),
        later_epic(2),
        blocked_by(story(10, 1), &[11]),
        blocker,
    ]);
    assert_eq!(
        lines(&broken),
        ["#11 blocks-earlier-release: blocks #10, which is in an earlier release"]
    );
    let repaired = survey(vec![
        epic(1),
        later_epic(2),
        blocked_by(story(10, 1), &[11]),
        story(11, 1),
    ]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn does_not_report_a_closed_v1_1_blocker_of_a_v1_issue() {
    let history = in_milestone(closed(11, 1), "v1.1");
    let tree = survey(vec![epic(1), blocked_by(story(10, 1), &[11]), history]);
    assert!(lines(&tree).is_empty());
}

#[test]
fn does_not_compare_an_issue_with_an_unreadable_milestone() {
    let odd = in_milestone(story(10, 1), "backlog");
    let blocker = in_milestone(story(11, 2), "v1.1");
    let tree = survey(vec![
        epic(1),
        later_epic(2),
        blocked_by(odd, &[11]),
        blocker,
    ]);
    assert_eq!(
        lines(&tree),
        ["#10 unreadable-milestone: milestone is not a release such as v1 or v1.1"]
    );
}
