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
        ["#10 unreadable-milestone: milestone is not a release such as v0.1.0 or v0.2.0"]
    );
}

#[test]
fn reports_a_cross_repository_blocker_in_a_later_release() {
    use crate::support::audit::labels;
    use crate::support::repo_number;
    use tools_core::domain::{BlockedBy, Lifecycle, Milestone, TreeIssue};

    let app_epic = TreeIssue::new(repo_number("app", 1), Lifecycle::Open)
        .with_labels(labels(&["epic", "type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1"));
    let web_epic = TreeIssue::new(repo_number("web", 2), Lifecycle::Open)
        .with_labels(labels(&["epic", "type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1.1"));

    let app_story = TreeIssue::new(repo_number("app", 10), Lifecycle::Open)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
        .with_parent(repo_number("app", 1))
        .with_blocked_by(BlockedBy::new(vec![repo_number("web", 11)]));

    let web_story = TreeIssue::new(repo_number("web", 11), Lifecycle::Open)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1.1"))
        .with_parent(repo_number("web", 2));

    let broken = survey(vec![app_epic, web_epic, app_story, web_story]);
    assert_eq!(
        lines(&broken),
        ["web#11 blocks-earlier-release: blocks app#10, which is in an earlier release"]
    );
}
