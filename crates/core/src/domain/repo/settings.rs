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

/// A specific repository setting difference with current and desired values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingDiff {
    /// Allow merge commits setting differs.
    AllowMergeCommit {
        /// Current value on GitHub.
        current: bool,
        /// Desired value in configuration.
        desired: bool,
    },
    /// Allow squash merges setting differs.
    AllowSquashMerge {
        /// Current value on GitHub.
        current: bool,
        /// Desired value in configuration.
        desired: bool,
    },
    /// Allow rebase merges setting differs.
    AllowRebaseMerge {
        /// Current value on GitHub.
        current: bool,
        /// Desired value in configuration.
        desired: bool,
    },
    /// Delete branch on merge setting differs.
    DeleteBranchOnMerge {
        /// Current value on GitHub.
        current: bool,
        /// Desired value in configuration.
        desired: bool,
    },
    /// Merge commit title format differs.
    MergeCommitTitle {
        /// Current value on GitHub.
        current: MergeCommitTitle,
        /// Desired value in configuration.
        desired: MergeCommitTitle,
    },
    /// Merge commit message format differs.
    MergeCommitMessage {
        /// Current value on GitHub.
        current: MergeCommitMessage,
        /// Desired value in configuration.
        desired: MergeCommitMessage,
    },
}

impl std::fmt::Display for SettingDiff {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllowMergeCommit { current, desired } => {
                write!(
                    formatter,
                    "allow_merge_commit: current '{current}' != desired '{desired}'"
                )
            }
            Self::AllowSquashMerge { current, desired } => {
                write!(
                    formatter,
                    "allow_squash_merge: current '{current}' != desired '{desired}'"
                )
            }
            Self::AllowRebaseMerge { current, desired } => {
                write!(
                    formatter,
                    "allow_rebase_merge: current '{current}' != desired '{desired}'"
                )
            }
            Self::DeleteBranchOnMerge { current, desired } => {
                write!(
                    formatter,
                    "delete_branch_on_merge: current '{current}' != desired '{desired}'"
                )
            }
            Self::MergeCommitTitle { current, desired } => {
                write!(
                    formatter,
                    "merge_commit_title: current '{current}' != desired '{desired}'"
                )
            }
            Self::MergeCommitMessage { current, desired } => {
                write!(
                    formatter,
                    "merge_commit_message: current '{current}' != desired '{desired}'"
                )
            }
        }
    }
}

/// Grouped repository settings differences containing all field-level differences.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SettingsDiff {
    diffs: Vec<SettingDiff>,
}

impl SettingsDiff {
    /// Creates a new group of settings differences.
    #[must_use]
    pub const fn new(diffs: Vec<SettingDiff>) -> Self {
        Self { diffs }
    }

    /// The list of setting differences.
    #[must_use]
    pub fn diffs(&self) -> &[SettingDiff] {
        &self.diffs
    }

    /// Whether there are any setting differences.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }

    /// The number of setting differences.
    #[must_use]
    pub fn len(&self) -> usize {
        self.diffs.len()
    }

    /// Converts the changed settings into a minimal JSON PATCH payload map.
    #[must_use]
    pub fn to_patch_payload(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for diff in &self.diffs {
            match diff {
                SettingDiff::AllowMergeCommit { desired, .. } => {
                    map.insert("allow_merge_commit".into(), (*desired).into());
                }
                SettingDiff::AllowSquashMerge { desired, .. } => {
                    map.insert("allow_squash_merge".into(), (*desired).into());
                }
                SettingDiff::AllowRebaseMerge { desired, .. } => {
                    map.insert("allow_rebase_merge".into(), (*desired).into());
                }
                SettingDiff::DeleteBranchOnMerge { desired, .. } => {
                    map.insert("delete_branch_on_merge".into(), (*desired).into());
                }
                SettingDiff::MergeCommitTitle { desired, .. } => {
                    map.insert("merge_commit_title".into(), desired.to_string().into());
                }
                SettingDiff::MergeCommitMessage { desired, .. } => {
                    map.insert("merge_commit_message".into(), desired.to_string().into());
                }
            }
        }
        serde_json::Value::Object(map)
    }
}

impl std::ops::Deref for SettingsDiff {
    type Target = [SettingDiff];

    fn deref(&self) -> &Self::Target {
        &self.diffs
    }
}

impl<'a> IntoIterator for &'a SettingsDiff {
    type Item = &'a SettingDiff;
    type IntoIter = std::slice::Iter<'a, SettingDiff>;

    fn into_iter(self) -> Self::IntoIter {
        self.diffs.iter()
    }
}

impl RepoSettings {
    /// Computes differences between current settings (`self`) and `desired` settings.
    #[must_use]
    pub fn diff(&self, desired: &Self) -> SettingsDiff {
        let mut diffs = Vec::new();
        if self.allow_merge_commit != desired.allow_merge_commit {
            diffs.push(SettingDiff::AllowMergeCommit {
                current: self.allow_merge_commit,
                desired: desired.allow_merge_commit,
            });
        }
        if self.allow_squash_merge != desired.allow_squash_merge {
            diffs.push(SettingDiff::AllowSquashMerge {
                current: self.allow_squash_merge,
                desired: desired.allow_squash_merge,
            });
        }
        if self.allow_rebase_merge != desired.allow_rebase_merge {
            diffs.push(SettingDiff::AllowRebaseMerge {
                current: self.allow_rebase_merge,
                desired: desired.allow_rebase_merge,
            });
        }
        if self.delete_branch_on_merge != desired.delete_branch_on_merge {
            diffs.push(SettingDiff::DeleteBranchOnMerge {
                current: self.delete_branch_on_merge,
                desired: desired.delete_branch_on_merge,
            });
        }
        if self.merge_commit_title != desired.merge_commit_title {
            diffs.push(SettingDiff::MergeCommitTitle {
                current: self.merge_commit_title,
                desired: desired.merge_commit_title,
            });
        }
        if self.merge_commit_message != desired.merge_commit_message {
            diffs.push(SettingDiff::MergeCommitMessage {
                current: self.merge_commit_message,
                desired: desired.merge_commit_message,
            });
        }
        SettingsDiff::new(diffs)
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
        assert!(matches!(
            &diffs[0],
            SettingDiff::AllowSquashMerge { current: true, desired: false }
        ));
        assert!(matches!(
            &diffs[1],
            SettingDiff::DeleteBranchOnMerge { current: false, desired: true }
        ));
        assert!(matches!(
            &diffs[2],
            SettingDiff::MergeCommitTitle {
                current: MergeCommitTitle::MergeMessage,
                desired: MergeCommitTitle::PrTitle,
            }
        ));
        assert!(matches!(
            &diffs[3],
            SettingDiff::MergeCommitMessage {
                current: MergeCommitMessage::PrTitle,
                desired: MergeCommitMessage::PrBody,
            }
        ));

        let payload = diffs.to_patch_payload();
        assert_eq!(payload["allow_squash_merge"], false);
        assert_eq!(payload["delete_branch_on_merge"], true);
        assert_eq!(payload["merge_commit_title"], "PR_TITLE");
        assert_eq!(payload["merge_commit_message"], "PR_BODY");
        assert!(payload.get("allow_merge_commit").is_none());
    }
}
