//! The GraphQL requests, with the Project and repository filled in.
//!
//! Each request lives in its own `.graphql` file in this directory and is read
//! at build time. The functions here only fill in the placeholders.

use crate::adapters::github::target::Target;

const ITEMS: &str = include_str!("items.graphql");
const ISSUES: &str = include_str!("issues.graphql");
const PULLS: &str = include_str!("pulls.graphql");
const TEXTS_ISSUES: &str = include_str!("texts_issues.graphql");
const TEXTS_PULLS: &str = include_str!("texts_pulls.graphql");
const ITEM_OF_ISSUE: &str = include_str!("item_of_issue.graphql");
const STATUS_FIELD: &str = include_str!("status_field.graphql");

/// The mutation that sets one single-select field. It takes four variables:
/// `project`, `item`, `field` and `option`.
pub const SET_STATUS: &str = include_str!("set_status.graphql");

fn fill(template: &str, target: &Target) -> String {
    let project_str = target
        .project()
        .map_or_else(String::new, |num| num.to_string());
    template
        .replace("@OWNER@", target.owner())
        .replace("@REPOSITORY@", target.repository())
        .replace("@PROJECT@", &project_str)
}

/// One page of the Project's items, with their issues.
#[must_use]
pub fn items(target: &Target) -> String {
    fill(ITEMS, target)
}

/// One page of the repository's issues, open and closed.
#[must_use]
pub fn issues(target: &Target) -> String {
    fill(ISSUES, target)
}

/// One page of the open pull requests and the issues they close.
#[must_use]
pub fn pulls(target: &Target) -> String {
    fill(PULLS, target)
}

/// One page of the repository's issues with their title, body and comments.
#[must_use]
pub fn texts_issues(target: &Target) -> String {
    fill(TEXTS_ISSUES, target)
}

/// One page of the repository's pull requests, open, closed and merged, with
/// their title, body, comments, reviews and review threads.
#[must_use]
pub fn texts_pulls(target: &Target) -> String {
    fill(TEXTS_PULLS, target)
}

/// The Project items of one issue.
#[must_use]
pub fn item_of_issue(target: &Target, number: u64) -> String {
    fill(ITEM_OF_ISSUE, target).replace("@NUMBER@", &number.to_string())
}

/// The Project id and the Status field with its options.
#[must_use]
pub fn status_field(target: &Target) -> String {
    fill(STATUS_FIELD, target)
}
