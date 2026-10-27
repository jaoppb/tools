//! Repository label representation and diffing.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::domain::repo::DeleteReason;

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

/// Repository labels specification and reconciliation policy.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct RepoLabelsSpec {
    /// List of label names to prune.
    #[serde(default)]
    pub deprecated: Vec<String>,
    /// Whether to prune unmanaged labels.
    #[serde(default)]
    pub prune_unmanaged: bool,
    /// Desired labels to enforce.
    #[serde(default)]
    pub items: Vec<RepoLabel>,
}

fn normalize_hex_color(color: &str) -> String {
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
        let current_by_name: HashMap<String, &Self> = current
            .iter()
            .map(|item| (item.name.to_ascii_lowercase(), item))
            .collect();
        let desired_by_name: HashMap<String, &Self> = desired
            .iter()
            .map(|item| (item.name.to_ascii_lowercase(), item))
            .collect();
        let deprecated_names: HashSet<String> = deprecated
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();

        // 1. Reconcile desired labels against current (O(M))
        for item in desired {
            let key = item.name.to_ascii_lowercase();
            if let Some(existing) = current_by_name.get(&key) {
                let name_differs = existing.name != item.name;
                let color_differs = existing.color != item.color;
                let desc_differs = existing.description != item.description;
                if name_differs || color_differs || desc_differs {
                    diffs.push(LabelDiff::Update {
                        current: (*existing).clone(),
                        desired: item.clone(),
                    });
                }
            } else {
                diffs.push(LabelDiff::Create(item.clone()));
            }
        }

        // 2. Reconcile current labels for deprecated or unmanaged (O(N))
        for label in current {
            let key = label.name.to_ascii_lowercase();
            if desired_by_name.contains_key(&key) {
                // Already reconciled in Step 1 (in sync or updated). Desired takes precedence.
                continue;
            }

            if deprecated_names.contains(&key) {
                diffs.push(LabelDiff::Delete {
                    name: label.name.clone(),
                    reason: DeleteReason::Deprecated,
                });
            } else if prune_unmanaged {
                diffs.push(LabelDiff::Delete {
                    name: label.name.clone(),
                    reason: DeleteReason::Unmanaged,
                });
            } else {
                diffs.push(LabelDiff::Unmanaged {
                    name: label.name.clone(),
                });
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
        reason: DeleteReason,
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

#[cfg(test)]
mod tests {
    use super::*;

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
            LabelDiff::Delete { name, reason: DeleteReason::Deprecated } if name == "deprecated-label"
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
    fn label_diff_ignores_deprecated_if_item_is_in_desired() {
        let current = vec![RepoLabel {
            name: "type:feat".into(),
            color: "1d76db".into(),
            description: "Feature".into(),
        }];
        let desired = vec![RepoLabel {
            name: "type:feat".into(),
            color: "1d76db".into(),
            description: "Feature".into(),
        }];
        // "type:feat" mistakenly also in deprecated
        let deprecated = vec!["type:feat".into()];

        let diffs = RepoLabel::diff(&current, &desired, &deprecated, false);
        // Should NOT emit a Delete; desired takes precedence and it is in sync
        assert!(diffs.is_empty());
    }
}
