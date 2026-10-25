//! No blocked-by cycle among open issues.

use std::collections::BTreeSet;

use crate::domain::audit::survey::Survey;
use crate::domain::audit::survey::issue::TreeIssue;
use crate::domain::audit::violation::{Cycle, Rule, Violation};
use crate::domain::issue::IssueNumber;

/// One violation per cycle, filed under its lowest member and naming all of
/// them. A closed issue ends a chain, and so does an issue outside the survey.
#[must_use]
pub fn check(survey: &Survey) -> Vec<Violation> {
    survey
        .open_issues()
        .filter_map(|issue| cycle_from(issue.number(), survey))
        .collect()
}

fn cycle_from(start: IssueNumber, survey: &Survey) -> Option<Violation> {
    let reach = reachable(start, survey);
    if !reach.contains(&start) {
        return None;
    }
    let members: Vec<IssueNumber> = reach
        .into_iter()
        .filter(|other| reachable(*other, survey).contains(&start))
        .collect();
    let lowest = members.first() == Some(&start);
    lowest.then_some(Violation::new(start, Rule::Cycle(Cycle::new(members))))
}

/// The open issues that block `start`, directly or through other blockers.
fn reachable(start: IssueNumber, survey: &Survey) -> BTreeSet<IssueNumber> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(current) = pending.pop() {
        let blockers = open_blockers(current, survey);
        pending.extend(blockers.into_iter().filter(|number| seen.insert(*number)));
    }
    seen
}

fn open_blockers(number: IssueNumber, survey: &Survey) -> Vec<IssueNumber> {
    survey
        .find(number)
        .into_iter()
        .flat_map(TreeIssue::blockers)
        .filter(|blocker| survey.find(*blocker).is_some_and(TreeIssue::is_open))
        .collect()
}
