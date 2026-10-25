//! The read-only picture of the repository that the audit rules look at.

pub mod issue;

use crate::domain::audit::rules;
use crate::domain::audit::survey::issue::TreeIssue;
use crate::domain::audit::text::{TextItem, Texts};
use crate::domain::audit::violation::Violations;
use crate::domain::issue::IssueNumber;

/// Every issue of the repository, open and closed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssueTree(Vec<TreeIssue>);

impl IssueTree {
    /// The issues of a repository.
    #[must_use]
    pub const fn new(issues: Vec<TreeIssue>) -> Self {
        Self(issues)
    }

    /// The issue with this number, if the repository has it.
    #[must_use]
    pub fn find(&self, number: IssueNumber) -> Option<&TreeIssue> {
        self.0.iter().find(|issue| issue.number() == number)
    }

    /// The issues in the order they were read.
    pub fn iter(&self) -> impl Iterator<Item = &TreeIssue> {
        self.0.iter()
    }
}

/// The numbers of the issues that are items of the Project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Members(Vec<IssueNumber>);

impl Members {
    /// The numbers of the Project's issues.
    #[must_use]
    pub const fn new(numbers: Vec<IssueNumber>) -> Self {
        Self(numbers)
    }

    /// Whether this issue is an item of the Project.
    #[must_use]
    pub fn contains(&self, number: IssueNumber) -> bool {
        self.0.contains(&number)
    }
}

/// The repository's issues, the Project's items and the text of the issues and
/// pull requests: all the audit sees.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Survey {
    issues: IssueTree,
    members: Members,
    texts: Texts,
}

impl Survey {
    /// A survey of these issues, Project items and texts.
    #[must_use]
    pub const fn new(issues: IssueTree, members: Members, texts: Texts) -> Self {
        Self {
            issues,
            members,
            texts,
        }
    }

    /// Every issue, open and closed.
    pub fn issues(&self) -> impl Iterator<Item = &TreeIssue> {
        self.issues.iter()
    }

    /// The open issues.
    pub fn open_issues(&self) -> impl Iterator<Item = &TreeIssue> {
        self.issues.iter().filter(|issue| issue.is_open())
    }

    /// The issue with this number, if the repository has it.
    #[must_use]
    pub fn find(&self, number: IssueNumber) -> Option<&TreeIssue> {
        self.issues.find(number)
    }

    /// Whether this issue is an item of the Project.
    #[must_use]
    pub fn is_member(&self, number: IssueNumber) -> bool {
        self.members.contains(number)
    }

    /// Every text of an issue or a pull request, open or closed.
    pub fn texts(&self) -> impl Iterator<Item = &TextItem> {
        self.texts.iter()
    }

    /// Every rule broken, sorted by issue number and then by rule.
    #[must_use]
    pub fn violations(&self, rules: &crate::config::RulesConfig) -> Violations {
        rules::check(self, rules)
    }
}
