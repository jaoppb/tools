//! Every issue, open or closed, has exactly one `type:` label and an `area:` label.

use crate::domain::audit::survey::Survey;
use crate::domain::audit::violation::{Rule, Violation};

/// The issues without exactly one type label or without an area label matching the given prefixes.
#[must_use]
pub fn check<S: AsRef<str>>(survey: &Survey, area_prefixes: &[S]) -> Vec<Violation> {
    survey
        .issues()
        .flat_map(|issue| {
            Violation::each(
                issue.number(),
                [
                    (!issue.has_one_type()).then_some(Rule::NotOneType),
                    (!issue.has_area_matching(area_prefixes)).then_some(Rule::NoArea),
                ],
            )
        })
        .collect()
}
