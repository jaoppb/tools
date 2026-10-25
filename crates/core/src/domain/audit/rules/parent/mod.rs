//! Every open issue but an epic has a parent, and the parent is not closed.

use crate::domain::audit::survey::Survey;
use crate::domain::audit::survey::issue::TreeIssue;
use crate::domain::audit::violation::{Rule, Violation};

/// The open issues with no parent that are not epics, and the open issues
/// under a closed parent. A parent missing from the survey is skipped.
#[must_use]
pub fn check(survey: &Survey) -> Vec<Violation> {
    survey
        .open_issues()
        .flat_map(|issue| Violation::each(issue.number(), [missing(issue), closed(issue, survey)]))
        .collect()
}

fn missing(issue: &TreeIssue) -> Option<Rule> {
    (issue.parent().is_none() && !issue.is_epic()).then_some(Rule::NoParent)
}

fn closed(issue: &TreeIssue, survey: &Survey) -> Option<Rule> {
    let parent = survey.find(issue.parent()?)?;
    parent
        .is_closed()
        .then_some(Rule::ClosedParent(parent.number()))
}
