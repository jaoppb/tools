//! The read-only picture of the repository that the audit rules look at.

pub mod issue;

use crate::domain::audit::rules;
use crate::domain::audit::survey::issue::TreeIssue;
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
    pub fn find(&self, number: &IssueNumber) -> Option<&TreeIssue> {
        self.0.iter().find(|issue| issue.number() == number)
    }

    /// The issues in the order they were read.
    pub fn iter(&self) -> impl Iterator<Item = &TreeIssue> {
        self.0.iter()
    }

    /// Consumes the tree, returning the issues.
    #[must_use]
    pub fn into_issues(self) -> Vec<TreeIssue> {
        self.0
    }
}

impl IntoIterator for IssueTree {
    type Item = TreeIssue;
    type IntoIter = std::vec::IntoIter<TreeIssue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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
    pub fn contains(&self, number: &IssueNumber) -> bool {
        self.0.contains(number)
    }
}

/// The repository's issues and the Project's items: all the audit sees.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Survey {
    issues: IssueTree,
    members: Members,
}

impl Survey {
    /// A survey of these issues and Project items.
    #[must_use]
    pub const fn new(issues: IssueTree, members: Members) -> Self {
        Self { issues, members }
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
    pub fn find(&self, number: &IssueNumber) -> Option<&TreeIssue> {
        self.issues.find(number)
    }

    /// Whether this issue is an item of the Project.
    #[must_use]
    pub fn is_member(&self, number: &IssueNumber) -> bool {
        self.members.contains(number)
    }

    /// Every rule broken, sorted by issue number and then by rule.
    #[must_use]
    pub fn violations(&self, rules: &crate::config::RulesConfig) -> Violations {
        rules::check(self, rules)
    }
}
