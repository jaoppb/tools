//! Repository merge and branch settings enforced on GitHub.

use serde::{Deserialize, Serialize};

use crate::config::{LabelsConfig, MilestonesConfig, RepositorySettingsConfig, default_open};

/// Repository settings enforced on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSettings {
    /// Whether merge commits are allowed.
    pub allow_merge_commit: bool,
    /// Whether squash merges are allowed.
    pub allow_squash_merge: bool,
    /// Whether rebase merges are allowed.
    pub allow_rebase_merge: bool,
    /// Whether head branches are automatically deleted upon merge.
    pub delete_branch_on_merge: bool,
    /// Default merge commit title format (e.g. `PR_TITLE` or `MERGE_MESSAGE`).
    pub merge_commit_title: String,
    /// Default merge commit message format (e.g. `PR_BODY`, `PR_TITLE`, or `BLANK`).
    pub merge_commit_message: String,
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
                current: self.merge_commit_title.clone(),
                desired: desired.merge_commit_title.clone(),
            });
        }
        if self.merge_commit_message != desired.merge_commit_message {
            diffs.push(SettingDiff {
                name: "merge_commit_message",
                current: self.merge_commit_message.clone(),
                desired: desired.merge_commit_message.clone(),
            });
        }
        diffs
    }
}

impl From<&RepositorySettingsConfig> for RepoSettings {
    fn from(config: &RepositorySettingsConfig) -> Self {
        Self {
            allow_merge_commit: config.allow_merge_commit,
            allow_squash_merge: config.allow_squash_merge,
            allow_rebase_merge: config.allow_rebase_merge,
            delete_branch_on_merge: config.delete_branch_on_merge,
            merge_commit_title: config.merge_commit_title.clone(),
            merge_commit_message: config.merge_commit_message.clone(),
        }
    }
}

/// Repository label representation on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoLabel {
    /// Label ID on GitHub (GraphQL ID if available).
    #[serde(default)]
    pub id: Option<String>,
    /// Label name (e.g. `type:feat`).
    pub name: String,
    /// Hex color without `#` (e.g. `1d76db`).
    pub color: String,
    /// Label description.
    #[serde(default)]
    pub description: String,
}

/// A difference or required action for repository labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelDiff {
    /// Label exists desired in config but is missing on GitHub.
    Create(RepoLabel),
    /// Label exists on GitHub but attributes differ.
    Update {
        /// Label name.
        name: String,
        /// Old color on GitHub.
        old_color: String,
        /// New desired color.
        new_color: String,
        /// Old description on GitHub.
        old_description: String,
        /// New desired description.
        new_description: String,
    },
    /// Label is marked deprecated (or unmanaged with prune enabled) and should be deleted.
    Delete {
        /// Label name.
        name: String,
        /// Reason for deletion.
        reason: &'static str,
    },
    /// Label exists on GitHub but is unmanaged (reported as drift in check mode).
    Unmanaged {
        /// Label name.
        name: String,
    },
}

impl std::fmt::Display for LabelDiff {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create(label) => {
                write!(
                    formatter,
                    "label '{}': missing (create with color '{}', description '{}')",
                    label.name, label.color, label.description
                )
            }
            Self::Update {
                name,
                old_color,
                new_color,
                old_description,
                new_description,
            } => {
                let mut changes = Vec::new();
                if old_color != new_color {
                    changes.push(format!("color '{old_color}' -> '{new_color}'"));
                }
                if old_description != new_description {
                    changes.push(format!("description '{old_description}' -> '{new_description}'"));
                }
                write!(formatter, "label '{name}': update {}", changes.join(", "))
            }
            Self::Delete { name, reason } => {
                write!(formatter, "label '{name}': delete ({reason})")
            }
            Self::Unmanaged { name } => {
                write!(
                    formatter,
                    "label '{name}': unmanaged drift (exists on GitHub but not in tools.toml)"
                )
            }
        }
    }
}

impl RepoLabel {
    /// Computes differences between current labels and desired label configuration.
    #[must_use]
    pub fn diff(current: &[Self], desired: &LabelsConfig) -> Vec<LabelDiff> {
        let mut diffs = Vec::new();

        // Check for missing or modified desired labels
        for item in &desired.items {
            let item_color = item.color.trim_start_matches('#').to_ascii_lowercase();
            if let Some(existing) = current.iter().find(|l| l.name == item.name) {
                let existing_color = existing.color.trim_start_matches('#').to_ascii_lowercase();
                if existing_color != item_color || existing.description != item.description {
                    diffs.push(LabelDiff::Update {
                        name: item.name.clone(),
                        old_color: existing_color,
                        new_color: item_color,
                        old_description: existing.description.clone(),
                        new_description: item.description.clone(),
                    });
                }
            } else {
                diffs.push(LabelDiff::Create(Self {
                    id: None,
                    name: item.name.clone(),
                    color: item_color,
                    description: item.description.clone(),
                }));
            }
        }

        // Check for deprecated or unmanaged labels on GitHub
        for label in current {
            let is_desired = desired.items.iter().any(|item| item.name == label.name);
            let is_deprecated = desired.deprecated.iter().any(|dep| dep == &label.name);

            if is_deprecated {
                diffs.push(LabelDiff::Delete {
                    name: label.name.clone(),
                    reason: "deprecated",
                });
            } else if !is_desired {
                if desired.prune_unmanaged {
                    diffs.push(LabelDiff::Delete {
                        name: label.name.clone(),
                        reason: "unmanaged",
                    });
                } else {
                    diffs.push(LabelDiff::Unmanaged {
                        name: label.name.clone(),
                    });
                }
            }
        }

        diffs
    }
}

/// Repository milestone representation on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoMilestone {
    /// GitHub milestone number.
    #[serde(default)]
    pub number: Option<u64>,
    /// Milestone title (e.g. `v0`).
    pub title: String,
    /// Milestone description.
    #[serde(default)]
    pub description: Option<String>,
    /// Milestone state (`open` or `closed`).
    #[serde(default = "default_open")]
    pub state: String,
}

/// A difference or required action for repository milestones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MilestoneDiff {
    /// Milestone exists in config but is missing on GitHub.
    Create(RepoMilestone),
    /// Milestone exists on GitHub but description or state differs.
    Update {
        /// Milestone number on GitHub.
        number: u64,
        /// Milestone title.
        title: String,
        /// Description change (old, new).
        description_change: Option<(String, String)>,
        /// State change (old, new).
        state_change: Option<(String, String)>,
    },
    /// Milestone is marked deprecated and should be deleted or closed.
    Delete {
        /// Milestone number on GitHub.
        number: u64,
        /// Milestone title.
        title: String,
    },
    /// Milestone exists on GitHub but is unmanaged (reported as drift in check mode).
    Unmanaged {
        /// Milestone number on GitHub.
        number: u64,
        /// Milestone title.
        title: String,
    },
}

impl std::fmt::Display for MilestoneDiff {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create(m) => {
                write!(
                    formatter,
                    "milestone '{}': missing (create state '{}')",
                    m.title, m.state
                )
            }
            Self::Update {
                title,
                description_change,
                state_change,
                ..
            } => {
                let mut changes = Vec::new();
                if let Some((old_d, new_d)) = description_change {
                    changes.push(format!("description '{old_d}' -> '{new_d}'"));
                }
                if let Some((old_s, new_s)) = state_change {
                    changes.push(format!("state '{old_s}' -> '{new_s}'"));
                }
                write!(formatter, "milestone '{title}': update {}", changes.join(", "))
            }
            Self::Delete { title, .. } => {
                write!(formatter, "milestone '{title}': delete/close (deprecated)")
            }
            Self::Unmanaged { title, .. } => {
                write!(
                    formatter,
                    "milestone '{title}': unmanaged drift (exists on GitHub but not in tools.toml)"
                )
            }
        }
    }
}

impl RepoMilestone {
    /// Computes differences between current milestones and desired milestone configuration.
    #[must_use]
    pub fn diff(current: &[Self], desired: &MilestonesConfig) -> Vec<MilestoneDiff> {
        let mut diffs = Vec::new();

        // Check for missing or modified desired milestones
        for item in &desired.items {
            let item_state = item.state.to_ascii_lowercase();
            let item_desc = item.description.clone().unwrap_or_default();

            if let Some(existing) = current.iter().find(|m| m.title == item.title) {
                let existing_number = existing.number.unwrap_or(0);
                let existing_state = existing.state.to_ascii_lowercase();
                let existing_desc = existing.description.clone().unwrap_or_default();

                let desc_change = if existing_desc != item_desc {
                    Some((existing_desc, item_desc))
                } else {
                    None
                };

                let state_change = if existing_state != item_state {
                    Some((existing_state, item_state))
                } else {
                    None
                };

                if desc_change.is_some() || state_change.is_some() {
                    diffs.push(MilestoneDiff::Update {
                        number: existing_number,
                        title: item.title.clone(),
                        description_change: desc_change,
                        state_change,
                    });
                }
            } else {
                diffs.push(MilestoneDiff::Create(Self {
                    number: None,
                    title: item.title.clone(),
                    description: item.description.clone(),
                    state: item_state,
                }));
            }
        }

        // Check for deprecated or unmanaged milestones on GitHub
        for milestone in current {
            let is_desired = desired.items.iter().any(|item| item.title == milestone.title);
            let is_deprecated = desired.deprecated.iter().any(|dep| dep == &milestone.title);
            let number = milestone.number.unwrap_or(0);

            if is_deprecated {
                diffs.push(MilestoneDiff::Delete {
                    number,
                    title: milestone.title.clone(),
                });
            } else if !is_desired {
                diffs.push(MilestoneDiff::Unmanaged {
                    number,
                    title: milestone.title.clone(),
                });
            }
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
            merge_commit_title: "MERGE_MESSAGE".into(),
            merge_commit_message: "PR_TITLE".into(),
        };
        let desired = RepoSettings {
            allow_merge_commit: true,
            allow_squash_merge: false,
            allow_rebase_merge: false,
            delete_branch_on_merge: true,
            merge_commit_title: "PR_TITLE".into(),
            merge_commit_message: "PR_BODY".into(),
        };

        let diffs = current.diff(&desired);
        assert_eq!(diffs.len(), 4);
        assert_eq!(diffs[0].name, "allow_squash_merge");
        assert_eq!(diffs[1].name, "delete_branch_on_merge");
        assert_eq!(diffs[2].name, "merge_commit_title");
        assert_eq!(diffs[3].name, "merge_commit_message");
    }

    #[test]
    fn label_diff_detects_creates_updates_deletes_and_drift() {
        let current = vec![
            RepoLabel {
                id: None,
                name: "type:feat".into(),
                color: "1d76db".into(),
                description: "Old description".into(),
            },
            RepoLabel {
                id: None,
                name: "deprecated-label".into(),
                color: "cccccc".into(),
                description: "To remove".into(),
            },
            RepoLabel {
                id: None,
                name: "unmanaged-label".into(),
                color: "111111".into(),
                description: "Random label".into(),
            },
        ];

        let desired = LabelsConfig {
            deprecated: vec!["deprecated-label".into()],
            prune_unmanaged: false,
            items: vec![
                crate::config::LabelItemConfig {
                    name: "type:feat".into(),
                    color: "1d76db".into(),
                    description: "New description".into(),
                },
                crate::config::LabelItemConfig {
                    name: "type:fix".into(),
                    color: "d73a4a".into(),
                    description: "Fix bug".into(),
                },
            ],
        };

        let diffs = RepoLabel::diff(&current, &desired);
        assert_eq!(diffs.len(), 4);

        // 1. type:feat updated description
        assert!(matches!(
            &diffs[0],
            LabelDiff::Update {
                name,
                old_description,
                new_description,
                ..
            } if name == "type:feat" && old_description == "Old description" && new_description == "New description"
        ));

        // 2. type:fix created
        assert!(matches!(
            &diffs[1],
            LabelDiff::Create(label) if label.name == "type:fix"
        ));

        // 3. deprecated-label deleted
        assert!(matches!(
            &diffs[2],
            LabelDiff::Delete { name, reason: "deprecated" } if name == "deprecated-label"
        ));

        // 4. unmanaged-label reported as drift
        assert!(matches!(
            &diffs[3],
            LabelDiff::Unmanaged { name } if name == "unmanaged-label"
        ));
    }

    #[test]
    fn milestone_diff_detects_creates_updates_deletes_and_drift() {
        let current = vec![
            RepoMilestone {
                number: Some(1),
                title: "v0".into(),
                description: Some("Old bootstrap".into()),
                state: "open".into(),
            },
            RepoMilestone {
                number: Some(2),
                title: "v0.9".into(),
                description: None,
                state: "open".into(),
            },
            RepoMilestone {
                number: Some(3),
                title: "historical".into(),
                description: None,
                state: "closed".into(),
            },
        ];

        let desired = MilestonesConfig {
            deprecated: vec!["v0.9".into()],
            items: vec![
                crate::config::MilestoneItemConfig {
                    title: "v0".into(),
                    description: Some("New bootstrap".into()),
                    state: "open".into(),
                },
                crate::config::MilestoneItemConfig {
                    title: "v1".into(),
                    description: Some("Stable release".into()),
                    state: "open".into(),
                },
            ],
        };

        let diffs = RepoMilestone::diff(&current, &desired);
        assert_eq!(diffs.len(), 4);

        // 1. v0 updated description
        assert!(matches!(
            &diffs[0],
            MilestoneDiff::Update {
                number: 1,
                title,
                description_change: Some((old_d, new_d)),
                ..
            } if title == "v0" && old_d == "Old bootstrap" && new_d == "New bootstrap"
        ));

        // 2. v1 created
        assert!(matches!(
            &diffs[1],
            MilestoneDiff::Create(m) if m.title == "v1"
        ));

        // 3. v0.9 deleted/deprecated
        assert!(matches!(
            &diffs[2],
            MilestoneDiff::Delete { number: 2, title } if title == "v0.9"
        ));

        // 4. historical unmanaged drift
        assert!(matches!(
            &diffs[3],
            MilestoneDiff::Unmanaged { number: 3, title } if title == "historical"
        ));
    }
}
