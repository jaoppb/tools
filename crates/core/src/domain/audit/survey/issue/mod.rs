//! One issue of the repository, with the facts the audit rules read.

use crate::domain::audit::release::Release;
use crate::domain::facts::{Labels, Milestone};
use crate::domain::issue::{IssueNumber, Lifecycle};

/// The issues that block an issue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockedBy(Vec<IssueNumber>);

impl BlockedBy {
    /// The blockers of an issue.
    #[must_use]
    pub const fn new(blockers: Vec<IssueNumber>) -> Self {
        Self(blockers)
    }
}

/// Where an issue sits in the tree: its release, its parent and its blockers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placement {
    milestone: Option<Milestone>,
    parent: Option<IssueNumber>,
    blocked_by: BlockedBy,
}

/// One issue of the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeIssue {
    number: IssueNumber,
    lifecycle: Lifecycle,
    labels: Labels,
    placement: Placement,
}

impl TreeIssue {
    /// An issue with this number and lifecycle, no labels and no placement.
    #[must_use]
    pub fn new(number: IssueNumber, lifecycle: Lifecycle) -> Self {
        Self {
            number,
            lifecycle,
            labels: Labels::default(),
            placement: Placement::default(),
        }
    }

    /// This issue with these labels.
    #[must_use]
    pub fn with_labels(self, labels: Labels) -> Self {
        Self { labels, ..self }
    }

    /// This issue in this milestone.
    #[must_use]
    pub fn with_milestone(self, milestone: Milestone) -> Self {
        let placement = Placement {
            milestone: Some(milestone),
            ..self.placement
        };
        Self { placement, ..self }
    }

    /// This issue under this parent.
    #[must_use]
    pub fn with_parent(self, parent: IssueNumber) -> Self {
        let placement = Placement {
            parent: Some(parent),
            ..self.placement
        };
        Self { placement, ..self }
    }

    /// This issue blocked by these issues.
    #[must_use]
    pub fn with_blocked_by(self, blockers: BlockedBy) -> Self {
        let placement = Placement {
            blocked_by: blockers,
            ..self.placement
        };
        Self { placement, ..self }
    }

    /// The issue number.
    #[must_use]
    pub const fn number(&self) -> IssueNumber {
        self.number
    }

    /// Whether the issue is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        !self.lifecycle.is_closed()
    }

    /// Whether the issue is closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.lifecycle.is_closed()
    }

    /// Whether the issue is an epic.
    #[must_use]
    pub fn is_epic(&self) -> bool {
        self.labels.is_epic()
    }

    /// Whether the issue has exactly one `type:` label.
    #[must_use]
    pub fn has_one_type(&self) -> bool {
        self.labels.has_one_type()
    }

    /// Whether the issue has an `area:` label.
    #[must_use]
    pub fn has_area(&self) -> bool {
        self.labels.has_area()
    }

    /// Whether the issue has a milestone.
    #[must_use]
    pub const fn has_milestone(&self) -> bool {
        self.placement.milestone.is_some()
    }

    /// The release the milestone names, if the issue has one and it is a release.
    #[must_use]
    pub fn release(&self) -> Option<Release> {
        self.placement.milestone.as_ref().and_then(Release::of)
    }

    /// Whether the issue has a milestone that is not a release.
    #[must_use]
    pub fn has_unreadable_release(&self) -> bool {
        self.has_milestone() && self.release().is_none()
    }

    /// The parent, if the issue has one.
    #[must_use]
    pub const fn parent(&self) -> Option<IssueNumber> {
        self.placement.parent
    }

    /// The issues that block this one.
    pub fn blockers(&self) -> impl Iterator<Item = IssueNumber> + '_ {
        self.placement.blocked_by.0.iter().copied()
    }

    /// Whether both issues are open, have readable releases, and this one is
    /// in the later release.
    #[must_use]
    pub fn is_in_later_release_than(&self, other: &Self) -> bool {
        let both_open = self.is_open() && other.is_open();
        let releases = self.release().zip(other.release());
        both_open && releases.is_some_and(|(mine, theirs)| mine.is_later_than(&theirs))
    }
}
