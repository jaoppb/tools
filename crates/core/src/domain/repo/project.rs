//! Repository Project V2 representation and diffing.

use serde::{Deserialize, Serialize};

/// Desired repository Project V2 definition in configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RepoProjectSpec {
    /// GitHub Project V2 number.
    pub number: u32,
    /// Desired Project V2 title.
    pub title: String,
}

impl RepoProjectSpec {
    /// Creates a new desired project specification.
    #[must_use]
    pub fn new(number: u32, title: impl Into<String>) -> Self {
        Self {
            number,
            title: title.into(),
        }
    }
}

/// Repository Project V2 state on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoProject {
    /// GitHub Project V2 node ID.
    pub id: String,
    /// GitHub Project V2 number.
    pub number: u32,
    /// Current Project V2 title.
    pub title: String,
}

impl RepoProject {
    /// Creates a new Project V2 representation.
    #[must_use]
    pub fn new(id: impl Into<String>, number: u32, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            number,
            title: title.into(),
        }
    }

    /// Computes differences between current Project V2 state and desired configuration.
    #[must_use]
    pub fn diff(&self, desired: &RepoProjectSpec) -> Option<ProjectDiff> {
        debug_assert_eq!(
            self.number, desired.number,
            "cannot diff disparate project numbers"
        );
        if self.title != desired.title {
            Some(ProjectDiff::Update {
                current: self.clone(),
                desired: desired.clone(),
            })
        } else {
            None
        }
    }
}

/// A difference or required action for repository Project V2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectDiff {
    /// Project title differs between GitHub and configuration.
    Update {
        /// Current project on GitHub.
        current: RepoProject,
        /// Desired project specification.
        desired: RepoProjectSpec,
    },
}

impl ProjectDiff {
    /// Whether this difference requires an action.
    #[must_use]
    pub const fn is_actionable(&self) -> bool {
        true
    }
}

impl std::fmt::Display for ProjectDiff {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Update { current, desired } => {
                write!(
                    formatter,
                    "project #{}: update title '{}' -> '{}'",
                    desired.number, current.title, desired.title
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_diff_detects_title_update() {
        let current = RepoProject::new("PVT_1", 4, "Old Title");
        let desired = RepoProjectSpec::new(4, "New Title");

        let diff = current.diff(&desired).expect("should produce diff");
        assert_eq!(
            diff,
            ProjectDiff::Update {
                current: current.clone(),
                desired: desired.clone(),
            }
        );
        assert!(diff.is_actionable());
        assert_eq!(
            diff.to_string(),
            "project #4: update title 'Old Title' -> 'New Title'"
        );
    }

    #[test]
    fn project_diff_returns_none_when_titles_match() {
        let current = RepoProject::new("PVT_1", 4, "Same Title");
        let desired = RepoProjectSpec::new(4, "Same Title");

        assert!(current.diff(&desired).is_none());
    }
}
