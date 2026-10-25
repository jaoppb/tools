//! Every open issue but an epic has a parent, and the parent is not closed.

use tools_core::domain::{Lifecycle, TreeIssue};

use crate::support::audit::{closed, epic, labels, lines, story, survey};
use crate::support::number;

fn orphan(lifecycle: Lifecycle) -> TreeIssue {
    TreeIssue::new(number(10), lifecycle)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_milestone(tools_core::domain::Milestone::new("v1"))
}

#[test]
fn reports_an_open_story_with_no_parent() {
    let broken = survey(vec![orphan(Lifecycle::Open)]);
    assert_eq!(lines(&broken), ["#10 no-parent: no parent and not an epic"]);
    let repaired = survey(vec![epic(1), story(10, 1)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn does_not_report_an_epic_with_no_parent() {
    assert!(lines(&survey(vec![epic(1)])).is_empty());
}

#[test]
fn does_not_report_a_closed_issue_with_no_parent() {
    assert!(lines(&survey(vec![orphan(Lifecycle::Closed)])).is_empty());
}

#[test]
fn reports_an_open_issue_under_a_closed_parent() {
    let broken = survey(vec![closed(5, 1), story(10, 5), epic(1)]);
    assert_eq!(
        lines(&broken),
        ["#10 closed-parent: open under the closed parent #5"]
    );
    let repaired = survey(vec![story(5, 1), story(10, 5), epic(1)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn does_not_report_a_closed_issue_under_a_closed_parent() {
    let history = survey(vec![closed(5, 1), closed(10, 5), epic(1)]);
    assert!(lines(&history).is_empty());
}

#[test]
fn skips_a_parent_that_is_not_in_the_tree() {
    assert!(lines(&survey(vec![story(10, 77)])).is_empty());
}
