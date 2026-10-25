//! Every open issue has a milestone, and the milestone is a release.

use crate::domain::audit::survey::Survey;
use crate::domain::audit::violation::{Rule, Violation};

/// The open issues without a milestone or with a milestone that is not a release.
#[must_use]
pub fn check(survey: &Survey) -> Vec<Violation> {
    survey
        .open_issues()
        .flat_map(|issue| {
            Violation::each(
                issue.number(),
                [
                    (!issue.has_milestone()).then_some(Rule::NoMilestone),
                    issue
                        .has_unreadable_release()
                        .then_some(Rule::UnreadableMilestone),
                ],
            )
        })
        .collect()
}
