//! Issue identity: the number, the Project status, the lifecycle and the role.

use std::fmt;

pub mod status_mapping;
pub use status_mapping::{StatusMapping, StatusMappingError};

use crate::domain::repo::Repo;

/// The repository-qualified number of a GitHub issue. It is the only identity in the domain.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IssueNumber {
    repo: Repo,
    number: u64,
}

impl IssueNumber {
    /// The issue with this repository and number.
    #[must_use]
    pub fn new(repo: Repo, number: u64) -> Self {
        Self { repo, number }
    }

    /// The repository this issue belongs to.
    #[must_use]
    pub const fn repo(&self) -> &Repo {
        &self.repo
    }

    /// The number as GitHub writes it, for building requests.
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.number
    }
}

impl fmt::Display for IssueNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.repo.name().is_empty() {
            write!(formatter, "#{}", self.number)
        } else {
            write!(formatter, "{}#{}", self.repo.name(), self.number)
        }
    }
}

/// Where an issue is in its life, as the Project's Status field says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    /// The status matching `name` case-insensitively, supporting space, hyphen, or underscore separators.
    #[must_use]
    pub fn from_name_case_insensitive(name: &str) -> Option<Self> {
        let trimmed = name.trim();
        Self::ALL.into_iter().find(|status| {
            status.name().eq_ignore_ascii_case(trimmed)
                || match status {
                    Self::InProgress => {
                        trimmed.eq_ignore_ascii_case("in_progress")
                            || trimmed.eq_ignore_ascii_case("in-progress")
                            || trimmed.eq_ignore_ascii_case("inprogress")
                    }
                    Self::InReview => {
                        trimmed.eq_ignore_ascii_case("in_review")
                            || trimmed.eq_ignore_ascii_case("in-review")
                            || trimmed.eq_ignore_ascii_case("inreview")
                    }
                    _ => false,
                }
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn issue_number_distinguishes_across_repositories() {
        let app = Repo::new("org", "app");
        let web = Repo::new("org", "web");
        let api = Repo::new("org", "api");

        let app_1 = IssueNumber::new(app.clone(), 1);
        let web_1 = IssueNumber::new(web.clone(), 1);
        let api_1 = IssueNumber::new(api.clone(), 1);
        let app_2 = IssueNumber::new(app, 2);

        // Same number, different repo -> not equal
        assert_ne!(app_1, web_1);
        assert_ne!(app_1, api_1);
        assert_ne!(web_1, api_1);

        // Same repo, different number -> not equal
        assert_ne!(app_1, app_2);

        // Hashing eliminates collisions
        let mut set = HashSet::new();
        set.insert(app_1.clone());
        set.insert(web_1.clone());
        set.insert(api_1.clone());
        set.insert(app_2.clone());

        assert_eq!(set.len(), 4);
        assert!(set.contains(&app_1));
        assert!(set.contains(&web_1));
    }

    #[test]
    fn issue_number_orders_by_repo_then_number() {
        let api = Repo::new("org", "api");
        let app = Repo::new("org", "app");
        let web = Repo::new("org", "web");

        let mut issues = vec![
            IssueNumber::new(web.clone(), 1),
            IssueNumber::new(app.clone(), 10),
            IssueNumber::new(api.clone(), 5),
            IssueNumber::new(app.clone(), 2),
            IssueNumber::new(api.clone(), 1),
        ];

        issues.sort();

        let expected = vec![
            IssueNumber::new(api.clone(), 1),
            IssueNumber::new(api, 5),
            IssueNumber::new(app.clone(), 2),
            IssueNumber::new(app, 10),
            IssueNumber::new(web, 1),
        ];

        assert_eq!(issues, expected);
    }

    #[test]
    fn issue_number_display_formats_qualified_and_unqualified() {
        let repo = Repo::new("org", "tools");
        let qualified = IssueNumber::new(repo, 42);
        assert_eq!(qualified.to_string(), "tools#42");

        let unqualified = IssueNumber::new(Repo::new("", ""), 42);
        assert_eq!(unqualified.to_string(), "#42");
    }

    #[test]
    fn issue_number_accessors() {
        let repo = Repo::new("org", "core");
        let num = IssueNumber::new(repo.clone(), 99);
        assert_eq!(num.value(), 99);
        assert_eq!(num.repo(), &repo);
    }
}
