//! The audit rules, one module per theme. Each test reports a violation and
//! then, with the violation repaired, reports nothing.

mod cycle;
mod labels;
mod membership;
mod milestone;
mod parent;
mod references;
mod release;

use tools_core::domain::Survey;

use crate::support::audit::{epic, labelled, lines, story, survey, survey_with_members};

#[test]
fn reports_nothing_for_a_clean_tree() {
    let clean = survey(vec![epic(1)]);
    assert!(lines(&clean).is_empty());
}

#[test]
fn an_empty_survey_has_no_violations() {
    assert!(lines(&Survey::default()).is_empty());
}

#[test]
fn reports_several_findings_of_one_issue_in_rule_order() {
    let broken = labelled(story(10, 1), &["type:feat"]);
    let tree = survey_with_members(vec![epic(1), broken], &[1]);
    assert_eq!(
        lines(&tree),
        [
            "#10 no-area: no area label",
            "#10 not-in-project: not an item of the Project",
        ]
    );
}

#[test]
fn prints_the_same_findings_for_the_same_survey() {
    let tree = survey_with_members(vec![epic(1), story(10, 1)], &[]);
    assert_eq!(lines(&tree), lines(&tree));
}
