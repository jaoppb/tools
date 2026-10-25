//! Builders for the issue trees the audit tests need.

use tools_core::domain::{
    BlockedBy, IssueNumber, IssueTree, Label, Labels, Lifecycle, Members, Milestone, Surface,
    Survey, Text, TextItem, Texts, TreeIssue,
};

use crate::support::number;

/// Labels with these names.
#[must_use]
pub fn labels(names: &[&str]) -> Labels {
    Labels::new(names.iter().map(|name| Label::new(name)).collect())
}

fn complete(raw: u64, lifecycle: Lifecycle) -> TreeIssue {
    TreeIssue::new(number(raw), lifecycle)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
}

/// An open epic in `v1` with no parent.
#[must_use]
pub fn epic(raw: u64) -> TreeIssue {
    complete(raw, Lifecycle::Open).with_labels(labels(&["epic", "type:chore", "area:tooling"]))
}

/// An open issue in `v1` under this parent.
#[must_use]
pub fn story(raw: u64, parent: u64) -> TreeIssue {
    complete(raw, Lifecycle::Open).with_parent(number(parent))
}

/// A closed issue in `v1` under this parent.
#[must_use]
pub fn closed(raw: u64, parent: u64) -> TreeIssue {
    complete(raw, Lifecycle::Closed).with_parent(number(parent))
}

/// This issue with exactly these labels.
#[must_use]
pub fn labelled(issue: TreeIssue, names: &[&str]) -> TreeIssue {
    issue.with_labels(labels(names))
}

/// This issue in the milestone with this title.
#[must_use]
pub fn in_milestone(issue: TreeIssue, title: &str) -> TreeIssue {
    issue.with_milestone(Milestone::new(title))
}

/// This issue blocked by these issues.
#[must_use]
pub fn blocked_by(issue: TreeIssue, blockers: &[u64]) -> TreeIssue {
    let numbers: Vec<IssueNumber> = blockers.iter().map(|raw| number(*raw)).collect();
    issue.with_blocked_by(BlockedBy::new(numbers))
}

/// A survey of these issues, every one of them a Project item.
#[must_use]
pub fn survey(issues: Vec<TreeIssue>) -> Survey {
    let members = issues.iter().map(TreeIssue::number).collect();
    Survey::new(
        IssueTree::new(issues),
        Members::new(members),
        Texts::default(),
    )
}

/// A survey of these issues where only these numbers are Project items.
#[must_use]
pub fn survey_with_members(issues: Vec<TreeIssue>, members: &[u64]) -> Survey {
    let numbers = members.iter().map(|raw| number(*raw)).collect();
    Survey::new(
        IssueTree::new(issues),
        Members::new(numbers),
        Texts::default(),
    )
}

/// The findings of a survey, one printed line each.
#[must_use]
pub fn lines(survey: &Survey) -> Vec<String> {
    survey
        .violations(&tools_core::config::RulesConfig::default())
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// A text on the issue or pull request with this number.
#[must_use]
pub fn text(raw: u64, surface: Surface, words: &str) -> TextItem {
    TextItem::new(number(raw), surface, Text::new(words))
}

/// A survey with no issue where only these texts exist.
#[must_use]
pub fn survey_of_texts(items: Vec<TextItem>) -> Survey {
    Survey::new(IssueTree::default(), Members::default(), Texts::new(items))
}
