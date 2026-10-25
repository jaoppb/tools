//! An issue never sits under, or blocks, an issue of an earlier release.

use crate::domain::audit::survey::Survey;
use crate::domain::audit::survey::issue::TreeIssue;
use crate::domain::audit::violation::{Rule, Violation};

/// The open issues in a later release than their open parent, and the open
/// blockers in a later release than the open issue they block. Both ends must
/// have a readable release.
#[must_use]
pub fn check(survey: &Survey) -> Vec<Violation> {
    let mut found = under_earlier(survey);
    found.extend(blocking_earlier(survey));
    found
}

fn under_earlier(survey: &Survey) -> Vec<Violation> {
    survey
        .open_issues()
        .filter_map(|issue| {
            let parent = survey.find(issue.parent()?)?;
            let later = issue.is_in_later_release_than(parent);
            let rule = Rule::ParentEarlierRelease(parent.number());
            later.then_some(Violation::new(issue.number(), rule))
        })
        .collect()
}

fn blocking_earlier(survey: &Survey) -> Vec<Violation> {
    survey
        .open_issues()
        .flat_map(|blocked| later_blockers(blocked, survey))
        .collect()
}

fn later_blockers(blocked: &TreeIssue, survey: &Survey) -> Vec<Violation> {
    blocked
        .blockers()
        .filter_map(|number| survey.find(number))
        .filter(|blocker| blocker.is_in_later_release_than(blocked))
        .map(|blocker| {
            Violation::new(
                blocker.number(),
                Rule::BlocksEarlierRelease(blocked.number()),
            )
        })
        .collect()
}
