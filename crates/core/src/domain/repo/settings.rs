//! Repository merge and branch settings enforced on GitHub.

use serde::{Deserialize, Serialize};

const fn default_true() -> bool {
    true
}

/// Default merge commit title format enforced on GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MergeCommitTitle {
    /// Use the pull request title.
    #[default]
    PrTitle,
    /// Use the classic merge message.
    MergeMessage,
}

impl std::fmt::Display for MergeCommitTitle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrTitle => write!(formatter, "PR_TITLE"),
            Self::MergeMessage => write!(formatter, "MERGE_MESSAGE"),
        }
    }
}

/// Default merge commit message format enforced on GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MergeCommitMessage {
    /// Use the pull request body.
    #[default]
    PrBody,
    /// Use the pull request title.
    PrTitle,
    /// Default to blank.
    Blank,
}

impl std::fmt::Display for MergeCommitMessage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrBody => write!(formatter, "PR_BODY"),
            Self::PrTitle => write!(formatter, "PR_TITLE"),
            Self::Blank => write!(formatter, "BLANK"),
        }
    }
}

/// Repository settings enforced on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSettings {
    /// Whether merge commits are allowed.
    #[serde(default = "default_true")]
    pub allow_merge_commit: bool,
    /// Whether squash merges are allowed.
    #[serde(default)]
    pub allow_squash_merge: bool,
    /// Whether rebase merges are allowed.
    #[serde(default)]
    pub allow_rebase_merge: bool,
    /// Whether head branches are automatically deleted upon merge.
    #[serde(default = "default_true")]
    pub delete_branch_on_merge: bool,
    /// Default merge commit title format (e.g. `PR_TITLE` or `MERGE_MESSAGE`).
    #[serde(default)]
    pub merge_commit_title: MergeCommitTitle,
    /// Default merge commit message format (e.g. `PR_BODY`, `PR_TITLE`, or `BLANK`).
    #[serde(default)]
    pub merge_commit_message: MergeCommitMessage,
}

/// A difference found between current and desired repository settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingDiff {
    /// Setting name.
    pub name: &'static str,
    /// Current value on GitHub.
    pub current: String,
    /// Desired value in configuration.
    pub desired: String,
}

impl std::fmt::Display for SettingDiff {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: current '{}' != desired '{}'",
            self.name, self.current, self.desired
        )
    }
}

impl RepoSettings {
    /// Computes differences between current settings (`self`) and `desired` settings.
    #[must_use]
    pub fn diff(&self, desired: &Self) -> Vec<SettingDiff> {
        let mut diffs = Vec::new();
        if self.allow_merge_commit != desired.allow_merge_commit {
            diffs.push(SettingDiff {
                name: "allow_merge_commit",
                current: self.allow_merge_commit.to_string(),
                desired: desired.allow_merge_commit.to_string(),
            });
        }
        if self.allow_squash_merge != desired.allow_squash_merge {
            diffs.push(SettingDiff {
                name: "allow_squash_merge",
                current: self.allow_squash_merge.to_string(),
                desired: desired.allow_squash_merge.to_string(),
            });
        }
        if self.allow_rebase_merge != desired.allow_rebase_merge {
            diffs.push(SettingDiff {
                name: "allow_rebase_merge",
                current: self.allow_rebase_merge.to_string(),
                desired: desired.allow_rebase_merge.to_string(),
            });
        }
        if self.delete_branch_on_merge != desired.delete_branch_on_merge {
            diffs.push(SettingDiff {
                name: "delete_branch_on_merge",
                current: self.delete_branch_on_merge.to_string(),
                desired: desired.delete_branch_on_merge.to_string(),
            });
        }
        if self.merge_commit_title != desired.merge_commit_title {
            diffs.push(SettingDiff {
                name: "merge_commit_title",
                current: self.merge_commit_title.to_string(),
                desired: desired.merge_commit_title.to_string(),
            });
        }
        if self.merge_commit_message != desired.merge_commit_message {
            diffs.push(SettingDiff {
                name: "merge_commit_message",
                current: self.merge_commit_message.to_string(),
                desired: desired.merge_commit_message.to_string(),
            });
        }
        diffs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_reports_only_differing_fields() {
        let current = RepoSettings {
            allow_merge_commit: true,
            allow_squash_merge: true,
            allow_rebase_merge: false,
            delete_branch_on_merge: false,
            merge_commit_title: MergeCommitTitle::MergeMessage,
            merge_commit_message: MergeCommitMessage::PrTitle,
        };
        let desired = RepoSettings {
            allow_merge_commit: true,
            allow_squash_merge: false,
            allow_rebase_merge: false,
            delete_branch_on_merge: true,
            merge_commit_title: MergeCommitTitle::PrTitle,
            merge_commit_message: MergeCommitMessage::PrBody,
        };

        let diffs = current.diff(&desired);
        assert_eq!(diffs.len(), 4);
        assert_eq!(diffs[0].name, "allow_squash_merge");
        assert_eq!(diffs[1].name, "delete_branch_on_merge");
        assert_eq!(diffs[2].name, "merge_commit_title");
        assert_eq!(diffs[3].name, "merge_commit_message");
    }
}
