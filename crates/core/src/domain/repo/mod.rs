//! Repository merge and branch settings, labels, and milestones enforced on GitHub.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

/// Repository label representation on GitHub and in configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoLabel {
    /// Label name (e.g. `type:feat`).
    pub name: String,
    /// Hex color without `#` (e.g. `1d76db`).
    #[serde(deserialize_with = "deserialize_hex_color")]
    pub color: String,
    /// Label description.
    #[serde(default)]
    pub description: String,
}

pub(crate) fn normalize_hex_color(color: &str) -> String {
    color.trim_start_matches('#').to_ascii_lowercase()
}

fn deserialize_hex_color<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(normalize_hex_color(&s))
}

impl RepoLabel {
    /// Creates a new label with normalized hex color.
    #[must_use]
    pub fn new(name: impl Into<String>, color: &str, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color: normalize_hex_color(color),
            description: description.into(),
        }
    }

    /// Computes differences between current labels and desired label configuration.
    #[must_use]
    pub fn diff(
        current: &[Self],
        desired: &[Self],
        deprecated: &[String],
        prune_unmanaged: bool,
    ) -> Vec<LabelDiff> {
        let mut diffs = Vec::new();
        let desired_by_name: HashMap<String, &Self> = desired
            .iter()
            .map(|item| (item.name.to_ascii_lowercase(), item))
            .collect();
        let deprecated_names: HashSet<String> = deprecated
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();

        // 1. Check desired labels against current
        for item in desired {
            if let Some(existing) = current
                .iter()
                .find(|l| l.name.eq_ignore_ascii_case(&item.name))
            {
                let name_differs = existing.name != item.name;
                let color_differs = existing.color != item.color;
                let desc_differs = existing.description != item.description;
                if name_differs || color_differs || desc_differs {
                    diffs.push(LabelDiff::Update {
                        current: existing.clone(),
                        desired: item.clone(),
                    });
                }
            } else {
                diffs.push(LabelDiff::Create(item.clone()));
            }
        }

        // 2. Check current labels for deprecated or unmanaged
        for label in current {
            let key = label.name.to_ascii_lowercase();
            if deprecated_names.contains(&key) {
                diffs.push(LabelDiff::Delete {
                    name: label.name.clone(),
                    reason: "deprecated",
                });
            } else if !desired_by_name.contains_key(&key) {
                if prune_unmanaged {
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

/// A difference or required action for repository labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelDiff {
    /// Label exists desired in config but is missing on GitHub.
    Create(RepoLabel),
    /// Label exists on GitHub but attributes differ.
    Update {
        /// Current label state on GitHub.
        current: RepoLabel,
        /// Desired label state.
        desired: RepoLabel,
    },
    /// Label is marked deprecated (or unmanaged with prune enabled) and should be deleted.
    Delete {
        /// Label name.
        name: String,
        /// Reason for deletion.
        reason: &'static str,
    },
    /// Label exists on GitHub but is unmanaged (reported when pruning is disabled).
    Unmanaged {
        /// Label name.
        name: String,
    },
}

impl LabelDiff {
    /// Whether this difference requires an action or indicates a configuration drift.
    #[must_use]
    pub const fn is_actionable(&self) -> bool {
        !matches!(self, Self::Unmanaged { .. })
    }
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
            Self::Update { current, desired } => {
                let mut changes = Vec::new();
                if current.name != desired.name {
                    changes.push(format!("name '{}' -> '{}'", current.name, desired.name));
                }
                if current.color != desired.color {
                    changes.push(format!("color '{}' -> '{}'", current.color, desired.color));
                }
                if current.description != desired.description {
                    changes.push(format!(
                        "description '{}' -> '{}'",
                        current.description, desired.description
                    ));
                }
                write!(formatter, "label '{}': update {}", desired.name, changes.join(", "))
            }
            Self::Delete { name, reason } => {
                write!(formatter, "label '{name}': delete ({reason})")
            }
            Self::Unmanaged { name } => {
                write!(
                    formatter,
                    "label '{name}': unmanaged (exists on GitHub but not in tools.toml)"
                )
            }
        }
    }
}

/// Repository milestone state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MilestoneState {
    /// Active, open milestone.
    #[default]
    #[serde(alias = "OPEN")]
    Open,
    /// Closed milestone.
    #[serde(alias = "CLOSED")]
    Closed,
}

impl std::fmt::Display for MilestoneState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(formatter, "open"),
            Self::Closed => write!(formatter, "closed"),
        }
    }
}

/// Desired repository milestone definition in configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RepoMilestoneSpec {
    /// Milestone title (e.g. `v0`).
    pub title: String,
    /// Milestone description.
    #[serde(default)]
    pub description: Option<String>,
    /// Milestone state (`open` or `closed`).
    #[serde(default)]
    pub state: MilestoneState,
}

/// Repository milestone entity on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoMilestone {
    /// GitHub milestone number.
    pub number: u64,
    /// Milestone title (e.g. `v0`).
    pub title: String,
    /// Milestone description.
    #[serde(default)]
    pub description: Option<String>,
    /// Milestone state (`open` or `closed`).
    #[serde(default)]
    pub state: MilestoneState,
}

impl RepoMilestone {
    /// Computes differences between current milestones and desired milestone configuration.
    #[must_use]
    pub fn diff(
        current: &[Self],
        desired: &[RepoMilestoneSpec],
        deprecated: &[String],
        prune_unmanaged: bool,
    ) -> Vec<MilestoneDiff> {
        let mut diffs = Vec::new();
        let desired_by_title: HashMap<&str, &RepoMilestoneSpec> =
            desired.iter().map(|m| (m.title.as_str(), m)).collect();
        let deprecated_titles: HashSet<&str> = deprecated.iter().map(String::as_str).collect();

        // 1. Check desired milestones against current
        for item in desired {
            if let Some(existing) = current.iter().find(|m| m.title == item.title) {
                let cur_desc = existing.description.as_deref().unwrap_or_default();
                let des_desc = item.description.as_deref().unwrap_or_default();
                let desc_differs = cur_desc != des_desc;
                let state_differs = existing.state != item.state;

                if desc_differs || state_differs {
                    diffs.push(MilestoneDiff::Update {
                        number: existing.number,
                        current: existing.clone(),
                        desired: item.clone(),
                    });
                }
            } else {
                diffs.push(MilestoneDiff::Create(item.clone()));
            }
        }

        // 2. Check current milestones for deprecated or unmanaged
        for milestone in current {
            let title = milestone.title.as_str();
            if deprecated_titles.contains(title) {
                diffs.push(MilestoneDiff::Delete {
                    number: milestone.number,
                    title: milestone.title.clone(),
                    reason: "deprecated",
                });
            } else if !desired_by_title.contains_key(title) {
                if prune_unmanaged {
                    diffs.push(MilestoneDiff::Delete {
                        number: milestone.number,
                        title: milestone.title.clone(),
                        reason: "unmanaged",
                    });
                } else {
                    diffs.push(MilestoneDiff::Unmanaged {
                        number: milestone.number,
                        title: milestone.title.clone(),
                    });
                }
            }
        }

        diffs
    }
}

/// A difference or required action for repository milestones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MilestoneDiff {
    /// Milestone exists in config but is missing on GitHub.
    Create(RepoMilestoneSpec),
    /// Milestone exists on GitHub but description or state differs.
    Update {
        /// Milestone number on GitHub.
        number: u64,
        /// Current milestone on GitHub.
        current: RepoMilestone,
        /// Desired milestone specification.
        desired: RepoMilestoneSpec,
    },
    /// Milestone is marked deprecated (or unmanaged with pruning enabled) and should be deleted.
    Delete {
        /// Milestone number on GitHub.
        number: u64,
        /// Milestone title.
        title: String,
        /// Reason for deletion.
        reason: &'static str,
    },
    /// Milestone exists on GitHub but is unmanaged (reported when pruning is disabled).
    Unmanaged {
        /// Milestone number on GitHub.
        number: u64,
        /// Milestone title.
        title: String,
    },
}

impl MilestoneDiff {
    /// Whether this difference requires an action or indicates a configuration drift.
    #[must_use]
    pub const fn is_actionable(&self) -> bool {
        !matches!(self, Self::Unmanaged { .. })
    }
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
                current,
                desired,
                ..
            } => {
                let mut changes = Vec::new();
                let cur_desc = current.description.as_deref().unwrap_or_default();
                let des_desc = desired.description.as_deref().unwrap_or_default();
                if cur_desc != des_desc {
                    changes.push(format!("description '{cur_desc}' -> '{des_desc}'"));
                }
                if current.state != desired.state {
                    changes.push(format!("state '{}' -> '{}'", current.state, desired.state));
                }
                write!(formatter, "milestone '{}': update {}", desired.title, changes.join(", "))
            }
            Self::Delete { title, reason, .. } => {
                write!(formatter, "milestone '{title}': delete ({reason})")
            }
            Self::Unmanaged { title, .. } => {
                write!(
                    formatter,
                    "milestone '{title}': unmanaged (exists on GitHub but not in tools.toml)"
                )
            }
        }
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
                name: "type:feat".into(),
                color: "1d76db".into(),
                description: "Old description".into(),
            },
            RepoLabel {
                name: "deprecated-label".into(),
                color: "cccccc".into(),
                description: "To remove".into(),
            },
            RepoLabel {
                name: "unmanaged-label".into(),
                color: "111111".into(),
                description: "Random label".into(),
            },
        ];

        let desired_items = vec![
            RepoLabel {
                name: "type:feat".into(),
                color: "1d76db".into(),
                description: "New description".into(),
            },
            RepoLabel {
                name: "type:fix".into(),
                color: "d73a4a".into(),
                description: "Fix bug".into(),
            },
        ];
        let deprecated = vec!["deprecated-label".into()];

        let diffs = RepoLabel::diff(&current, &desired_items, &deprecated, false);
        assert_eq!(diffs.len(), 4);

        // 1. type:feat updated description
        assert!(matches!(
            &diffs[0],
            LabelDiff::Update {
                current,
                desired,
            } if desired.name == "type:feat" && current.description == "Old description" && desired.description == "New description"
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

        // 4. unmanaged-label reported as unmanaged
        assert!(matches!(
            &diffs[3],
            LabelDiff::Unmanaged { name } if name == "unmanaged-label"
        ));
    }

    #[test]
    fn label_diff_matches_case_insensitively_and_updates_casing() {
        let current = vec![RepoLabel {
            name: "Type:Feat".into(),
            color: "1d76db".into(),
            description: "Feature".into(),
        }];

        let desired_items = vec![RepoLabel {
            name: "type:feat".into(),
            color: "1d76db".into(),
            description: "Feature".into(),
        }];

        let diffs = RepoLabel::diff(&current, &desired_items, &[], false);
        assert_eq!(diffs.len(), 1);
        match &diffs[0] {
            LabelDiff::Update { current, desired } => {
                assert_eq!(current.name, "Type:Feat");
                assert_eq!(desired.name, "type:feat");
            }
            other => panic!("expected LabelDiff::Update, got {:?}", other),
        }
    }

    #[test]
    fn milestone_diff_detects_creates_updates_deletes_and_drift() {
        let current = vec![
            RepoMilestone {
                number: 1,
                title: "v0".into(),
                description: Some("Old bootstrap".into()),
                state: MilestoneState::Open,
            },
            RepoMilestone {
                number: 2,
                title: "v0.9".into(),
                description: None,
                state: MilestoneState::Open,
            },
            RepoMilestone {
                number: 3,
                title: "historical".into(),
                description: None,
                state: MilestoneState::Closed,
            },
        ];

        let desired_items = vec![
            RepoMilestoneSpec {
                title: "v0".into(),
                description: Some("New bootstrap".into()),
                state: MilestoneState::Open,
            },
            RepoMilestoneSpec {
                title: "v1".into(),
                description: Some("Stable release".into()),
                state: MilestoneState::Open,
            },
        ];
        let deprecated = vec!["v0.9".into()];

        let diffs = RepoMilestone::diff(&current, &desired_items, &deprecated, false);
        assert_eq!(diffs.len(), 4);

        // 1. v0 updated description
        assert!(matches!(
            &diffs[0],
            MilestoneDiff::Update {
                number: 1,
                current,
                desired,
            } if desired.title == "v0" && current.description.as_deref() == Some("Old bootstrap") && desired.description.as_deref() == Some("New bootstrap")
        ));

        // 2. v1 created
        assert!(matches!(
            &diffs[1],
            MilestoneDiff::Create(m) if m.title == "v1"
        ));

        // 3. v0.9 deleted/deprecated
        assert!(matches!(
            &diffs[2],
            MilestoneDiff::Delete { number: 2, title, reason: "deprecated" } if title == "v0.9"
        ));

        // 4. historical unmanaged
        assert!(matches!(
            &diffs[3],
            MilestoneDiff::Unmanaged { number: 3, title } if title == "historical"
        ));
    }

    #[test]
    fn milestone_diff_prunes_unmanaged_with_unmanaged_reason() {
        let current = vec![RepoMilestone {
            number: 4,
            title: "unmanaged-m".into(),
            description: None,
            state: MilestoneState::Open,
        }];

        let diffs = RepoMilestone::diff(&current, &[], &[], true);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(
            &diffs[0],
            MilestoneDiff::Delete { number: 4, title, reason: "unmanaged" } if title == "unmanaged-m"
        ));
    }

    #[test]
    fn milestone_diff_preserves_closed_state_when_updating_description() {
        let current = vec![RepoMilestone {
            number: 5,
            title: "v0.8".into(),
            description: Some("Old text".into()),
            state: MilestoneState::Closed,
        }];

        let desired_items = vec![RepoMilestoneSpec {
            title: "v0.8".into(),
            description: Some("New updated text".into()),
            state: MilestoneState::Closed,
        }];

        let diffs = RepoMilestone::diff(&current, &desired_items, &[], false);
        assert_eq!(diffs.len(), 1);
        match &diffs[0] {
            MilestoneDiff::Update {
                number,
                current,
                desired,
            } => {
                assert_eq!(*number, 5);
                assert_eq!(current.number, 5);
                assert_eq!(current.state, MilestoneState::Closed);
                assert_eq!(desired.state, MilestoneState::Closed);
                assert_eq!(desired.description.as_deref(), Some("New updated text"));
            }
            other => panic!("expected MilestoneDiff::Update, got {:?}", other),
        }
    }
}
