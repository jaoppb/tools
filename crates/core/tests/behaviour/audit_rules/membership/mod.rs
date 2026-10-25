//! Every issue of the repository is an item of the Project.

use crate::support::audit::{closed, epic, lines, story, survey, survey_with_members};

#[test]
fn reports_an_issue_that_is_not_a_project_item() {
    let broken = survey_with_members(vec![epic(1), story(10, 1)], &[1]);
    assert_eq!(
        lines(&broken),
        ["#10 not-in-project: not an item of the Project"]
    );
    let repaired = survey(vec![epic(1), story(10, 1)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn reports_a_closed_issue_that_is_not_a_project_item() {
    let broken = survey_with_members(vec![epic(1), closed(10, 1)], &[1]);
    assert_eq!(
        lines(&broken),
        ["#10 not-in-project: not an item of the Project"]
    );
}
