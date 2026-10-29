//! Every open issue has a milestone, and it is a release.

use tools_core::domain::{Lifecycle, TreeIssue};

use crate::support::audit::{closed, epic, in_milestone, labels, lines, story, survey};
use crate::support::number;

fn without_milestone(lifecycle: Lifecycle) -> TreeIssue {
    TreeIssue::new(number(10), lifecycle)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_parent(number(1))
}

#[test]
fn reports_an_open_issue_with_no_milestone() {
    let broken = survey(vec![epic(1), without_milestone(Lifecycle::Open)]);
    assert_eq!(lines(&broken), ["#10 no-milestone: no milestone"]);
    let repaired = survey(vec![epic(1), story(10, 1)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn does_not_report_a_closed_issue_with_no_milestone() {
    let history = survey(vec![epic(1), without_milestone(Lifecycle::Closed)]);
    assert!(lines(&history).is_empty());
}

#[test]
fn reports_an_open_issue_with_a_milestone_that_is_not_a_release() {
    let broken = survey(vec![epic(1), in_milestone(story(10, 1), "backlog")]);
    assert_eq!(
        lines(&broken),
        ["#10 unreadable-milestone: milestone is not a release such as v0.1.0 or v0.2.0"]
    );
    let repaired = survey(vec![epic(1), in_milestone(story(10, 1), "v0.1.0")]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn does_not_report_a_closed_issue_with_a_milestone_that_is_not_a_release() {
    let history = in_milestone(closed(10, 1), "backlog");
    assert!(lines(&survey(vec![epic(1), history])).is_empty());
}
