//! No text of an issue or a pull request names an ADR by an old number or a
//! numbered `spdd/` file. Closed and merged items are checked too.

use crate::domain::audit::survey::Survey;
use crate::domain::audit::violation::Violation;
use crate::domain::issue::IssueNumber;

/// The pull request of issue 144, the one item this rule skips.
///
/// Its comments hold the log of the rewrite of the references, with the old
/// text of every edit, so its text names old numbers on purpose. If the log
/// moves to an attachment, this constant goes away.
pub const LOG_CARRIER: IssueNumber = IssueNumber::new(151);

/// The findings of every text but the log carrier's, one per item, rule and
/// surface.
#[must_use]
pub fn check(survey: &Survey) -> Vec<Violation> {
    survey
        .texts()
        .filter(|item| item.number() != LOG_CARRIER)
        .flat_map(|item| {
            item.broken_rules()
                .into_iter()
                .map(|rule| Violation::new(item.number(), rule))
        })
        .collect()
}
