//! Every issue of the repository is an item of the Project.

use crate::domain::audit::survey::Survey;
use crate::domain::audit::violation::{Rule, Violation};

/// The issues that are not Project items, open or closed.
#[must_use]
pub fn check(survey: &Survey) -> Vec<Violation> {
    survey
        .issues()
        .filter(|issue| !survey.is_member(issue.number()))
        .map(|issue| Violation::new(issue.number(), Rule::NotInProject))
        .collect()
}
