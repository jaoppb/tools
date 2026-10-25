//! Issue identity: the number, the Project status, the lifecycle and the role.

use std::fmt;

/// The number of a GitHub issue. It is the only identity in the domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IssueNumber(u64);

impl IssueNumber {
    /// The issue with this number.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The number as GitHub writes it, for building requests.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for IssueNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{}", self.0)
    }
}

/// Where an issue is in its life, as the Project's Status field says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// Not ready to start.
    Backlog,
    /// Triaged and unblocked.
    Ready,
    /// The analysis has started.
    InProgress,
    /// A pull request that closes the issue is open.
    InReview,
    /// The issue is closed.
    Done,
}

impl Status {
    const ALL: [Self; 5] = [
        Self::Backlog,
        Self::Ready,
        Self::InProgress,
        Self::InReview,
        Self::Done,
    ];

    /// The name of the option in the Project's Status field.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Backlog => "Backlog",
            Self::Ready => "Ready",
            Self::InProgress => "In progress",
            Self::InReview => "In review",
            Self::Done => "Done",
        }
    }

    /// The status the Project option `name` stands for, if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.name() == name)
    }

    /// Whether work has started: the issue is In progress or In review.
    #[must_use]
    pub const fn started(self) -> bool {
        matches!(self, Self::InProgress | Self::InReview)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Whether the issue is open or closed, completed or not planned alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lifecycle {
    /// Still open.
    Open,
    /// Closed, as completed or as not planned.
    Closed,
}

impl Lifecycle {
    /// Whether the issue is closed.
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

/// What a parent issue is to its children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// An epic, which groups stories and is never a story's waiting reason.
    Epic,
    /// A story, whose tasks wait until it has started.
    Story,
}
