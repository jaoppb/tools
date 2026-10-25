//! How an issue relates to others: its blockers and its parent.

use crate::domain::issue::{IssueNumber, Lifecycle, Role};

/// An issue that must close before another one can be Ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocker {
    number: IssueNumber,
    lifecycle: Lifecycle,
}

impl Blocker {
    /// A blocker with this number and lifecycle. A blocker from another
    /// repository is built as open, so nothing is promoted by mistake.
    #[must_use]
    pub const fn new(number: IssueNumber, lifecycle: Lifecycle) -> Self {
        Self { number, lifecycle }
    }

    /// The number of the blocking issue.
    #[must_use]
    pub const fn number(self) -> IssueNumber {
        self.number
    }
}

/// The blockers of one issue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blockers(Vec<Blocker>);

impl Blockers {
    /// The blockers of an issue.
    #[must_use]
    pub const fn new(blockers: Vec<Blocker>) -> Self {
        Self(blockers)
    }

    /// Whether every blocker is closed. No blockers counts as closed.
    #[must_use]
    pub fn all_closed(&self) -> bool {
        self.0.iter().all(|blocker| blocker.lifecycle.is_closed())
    }
}

/// The parent of an issue and what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parent {
    number: IssueNumber,
    role: Role,
}

impl Parent {
    /// A parent with this number and role.
    #[must_use]
    pub const fn new(number: IssueNumber, role: Role) -> Self {
        Self { number, role }
    }
}

/// The blockers and the parent of one issue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Links {
    blockers: Blockers,
    parent: Option<Parent>,
}

impl Links {
    /// Links with these blockers and no parent.
    #[must_use]
    pub const fn new(blockers: Blockers) -> Self {
        Self {
            blockers,
            parent: None,
        }
    }

    /// These links with a parent.
    #[must_use]
    pub fn with_parent(self, parent: Parent) -> Self {
        Self {
            parent: Some(parent),
            ..self
        }
    }

    /// Whether every blocker is closed.
    #[must_use]
    pub fn unblocked(&self) -> bool {
        self.blockers.all_closed()
    }

    /// The story this issue is a task of, if its parent is a story.
    #[must_use]
    pub fn story(&self) -> Option<IssueNumber> {
        self.parent
            .filter(|parent| matches!(parent.role, Role::Story))
            .map(|parent| parent.number)
    }
}
