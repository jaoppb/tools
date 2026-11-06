//! Every open issue has a milestone, and its title has the configured format.

use crate::config::MilestoneFormat;
use crate::domain::audit::survey::Survey;
use crate::domain::audit::survey::issue::TreeIssue;
use crate::domain::audit::violation::{Rule, Violation};

/// The open issues without a milestone or with a milestone whose title the
/// `format` does not accept.
#[must_use]
pub fn check(survey: &Survey, format: &MilestoneFormat) -> Vec<Violation> {
    survey
        .open_issues()
        .flat_map(|issue| {
            Violation::each(
                issue.number(),
                [
                    (!issue.has_milestone()).then_some(Rule::NoMilestone),
                    badly_titled(issue, format),
                ],
            )
        })
        .collect()
}

/// The rule broken by a milestone the format rejects. The `semver` format keeps
/// its own rule, so reports of the default setup do not change.
fn badly_titled(issue: &TreeIssue, format: &MilestoneFormat) -> Option<Rule> {
    let milestone = issue.milestone()?;
    match format {
        MilestoneFormat::Semver => issue
            .release()
            .is_none()
            .then_some(Rule::UnreadableMilestone),
        MilestoneFormat::Any => None,
        MilestoneFormat::Pattern(pattern) => {
            (!pattern.is_match(&milestone.to_string())).then_some(Rule::InvalidMilestoneFormat)
        }
    }
}
