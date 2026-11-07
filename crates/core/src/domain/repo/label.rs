//! Repository label representation and diffing.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::domain::repo::{
    DeleteReason, ReconcileHandler, RepoPlanError, overlay_by_key, reconcile_by_key, union_names,
};

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
    /// Rename mappings from old label name to new label name.
    #[serde(default)]
    pub renames: BTreeMap<String, String>,
    /// Whether to prune unmanaged labels.
    #[serde(default)]
    pub prune_unmanaged: bool,
    /// Mappings from legacy issue labels to canonical labels.
    #[serde(default)]
    pub migrations: BTreeMap<String, String>,
    /// Desired labels to enforce.
    #[serde(default)]
    pub items: Vec<RepoLabel>,
}

/// Repository-scoped labels overlay applied on top of the shared [`RepoLabelsSpec`].
///
/// Items override shared items with the same name and are otherwise appended; `deprecated` is a
/// union; `renames` and `migrations` are merged with overlay entries winning; `prune_unmanaged`
/// replaces the shared value only when set.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoLabelsOverlay {
    /// Additional label names to prune.
    #[serde(default)]
    pub deprecated: Vec<String>,
    /// Additional or overriding rename mappings.
    #[serde(default)]
    pub renames: BTreeMap<String, String>,
    /// Overrides whether to prune unmanaged labels when set.
    #[serde(default)]
    pub prune_unmanaged: Option<bool>,
    /// Additional or overriding migration mappings.
    #[serde(default)]
    pub migrations: BTreeMap<String, String>,
    /// Additional or overriding labels.
    #[serde(default)]
    pub items: Vec<RepoLabel>,
}

impl RepoLabelsOverlay {
    /// Resolves the effective labels specification for a repository.
    ///
    /// # Errors
    ///
    /// Returns [`RepoPlanError::Validation`] when the overlay repeats an item key.
    pub fn apply_to(&self, base: Option<&RepoLabelsSpec>) -> Result<RepoLabelsSpec, RepoPlanError> {
        let mut spec = base.cloned().unwrap_or_default();
        union_names(&mut spec.deprecated, self.deprecated.clone());
        spec.renames.extend(self.renames.clone());
        spec.migrations.extend(self.migrations.clone());
        if let Some(prune) = self.prune_unmanaged {
            spec.prune_unmanaged = prune;
        }
        overlay_by_key(&mut spec.items, self.items.clone(), |label| &label.name)?;
        Ok(spec)
    }
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
    ///
    /// # Errors
    ///
    /// Returns [`RepoPlanError::Validation`] on configuration errors or [`RepoPlanError::Collision`]
    /// on remote collisions.
    pub fn diff(
        current: &[Self],
        desired: &[Self],
        deprecated: &[String],
        renames: &BTreeMap<String, String>,
        prune_unmanaged: bool,
    ) -> Result<Vec<LabelDiff>, RepoPlanError> {
        reconcile_by_key(
            current,
            desired,
            deprecated,
            renames,
            prune_unmanaged,
            LabelReconciler,
        )
    }
}

struct LabelReconciler;

impl ReconcileHandler<RepoLabel, RepoLabel, LabelDiff> for LabelReconciler {
    fn key_current<'a>(&self, item: &'a RepoLabel) -> &'a str {
        &item.name
    }

    fn key_desired<'a>(&self, item: &'a RepoLabel) -> &'a str {
        &item.name
    }

    fn diff(&self, current: &RepoLabel, desired: &RepoLabel) -> Option<LabelDiff> {
        let name_differs = current.name != desired.name;
        let color_differs = current.color != desired.color;
        let desc_differs = current.description != desired.description;
        if name_differs || color_differs || desc_differs {
            Some(LabelDiff::Update {
                current: current.clone(),
                desired: desired.clone(),
            })
        } else {
            None
        }
    }

    fn create(&self, desired: &RepoLabel) -> LabelDiff {
        LabelDiff::Create(desired.clone())
    }

    fn delete(&self, current: &RepoLabel, reason: DeleteReason) -> LabelDiff {
        LabelDiff::Delete {
            name: current.name.clone(),
            reason,
        }
    }

    fn unmanaged(&self, current: &RepoLabel) -> LabelDiff {
        LabelDiff::Unmanaged {
            name: current.name.clone(),
        }
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
                write!(
                    formatter,
                    "label '{}': update {}",
                    desired.name,
                    changes.join(", ")
                )
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
        let renames = BTreeMap::new();

        let diffs =
            RepoLabel::diff(&current, &desired_items, &deprecated, &renames, false).unwrap();
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
        let renames = BTreeMap::new();

        let diffs = RepoLabel::diff(&current, &desired_items, &[], &renames, false).unwrap();
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
        let renames = BTreeMap::new();

        let diffs = RepoLabel::diff(&current, &desired, &deprecated, &renames, false).unwrap();
        // Should NOT emit a Delete; desired takes precedence and it is in sync
        assert!(diffs.is_empty());
    }

    #[test]
    fn label_diff_reconciles_renames_in_place() {
        let current = vec![RepoLabel {
            name: "enhancement".into(),
            color: "a2eeef".into(),
            description: "Old feature description".into(),
        }];
        let desired = vec![RepoLabel {
            name: "type:feat".into(),
            color: "1d76db".into(),
            description: "New behaviour or capability".into(),
        }];
        let mut renames = BTreeMap::new();
        renames.insert("enhancement".into(), "type:feat".into());

        let diffs = RepoLabel::diff(&current, &desired, &[], &renames, false).unwrap();
        assert_eq!(diffs.len(), 1);
        match &diffs[0] {
            LabelDiff::Update { current, desired } => {
                assert_eq!(current.name, "enhancement");
                assert_eq!(desired.name, "type:feat");
                assert_eq!(desired.color, "1d76db");
            }
            other => panic!("expected LabelDiff::Update, got {:?}", other),
        }
        assert_eq!(
            diffs[0].to_string(),
            "label 'type:feat': update name 'enhancement' -> 'type:feat', color 'a2eeef' -> '1d76db', description 'Old feature description' -> 'New behaviour or capability'"
        );
    }

    #[test]
    fn label_diff_detects_collision_when_both_source_and_target_exist() {
        let current = vec![
            RepoLabel {
                name: "enhancement".into(),
                color: "a2eeef".into(),
                description: String::new(),
            },
            RepoLabel {
                name: "type:feat".into(),
                color: "1d76db".into(),
                description: String::new(),
            },
        ];
        let desired = vec![RepoLabel {
            name: "type:feat".into(),
            color: "1d76db".into(),
            description: String::new(),
        }];
        let mut renames = BTreeMap::new();
        renames.insert("enhancement".into(), "type:feat".into());

        let err = RepoLabel::diff(&current, &desired, &[], &renames, false).unwrap_err();
        assert!(matches!(err, RepoPlanError::Collision(_)));
        assert!(err.to_string().contains("enhancement"));
        assert!(err.to_string().contains("type:feat"));
    }

    #[test]
    fn label_diff_rejects_cycle_or_chain() {
        let desired = vec![RepoLabel {
            name: "c".into(),
            color: "111111".into(),
            description: String::new(),
        }];
        let mut renames = BTreeMap::new();
        renames.insert("a".into(), "b".into());
        renames.insert("b".into(), "c".into());

        let err = RepoLabel::diff(&[], &desired, &[], &renames, false).unwrap_err();
        assert!(matches!(err, RepoPlanError::Validation(_)));
        assert!(err.to_string().contains("cycle or chain"));
    }

    #[test]
    fn overlay_overrides_by_name_appends_and_merges() {
        let base = RepoLabelsSpec {
            deprecated: vec!["bug".into()],
            renames: BTreeMap::from([("a".into(), "x".into())]),
            prune_unmanaged: true,
            migrations: BTreeMap::new(),
            items: vec![
                RepoLabel::new("type:feat", "111111", "shared"),
                RepoLabel::new("area:core", "222222", "core"),
            ],
        };
        let overlay = RepoLabelsOverlay {
            deprecated: vec!["bug".into(), "wontfix".into()],
            renames: BTreeMap::from([("b".into(), "y".into())]),
            prune_unmanaged: None,
            migrations: BTreeMap::from([("ui".into(), "area:ssr".into())]),
            items: vec![
                RepoLabel::new("type:feat", "#ABCDEF", "local"),
                RepoLabel::new("area:ssr", "333333", "ssr"),
            ],
        };
        let merged = overlay.apply_to(Some(&base)).unwrap();
        assert_eq!(merged.deprecated, vec!["bug", "wontfix"]);
        assert_eq!(merged.renames.len(), 2);
        assert_eq!(merged.migrations.get("ui"), Some(&"area:ssr".to_string()));
        assert!(merged.prune_unmanaged);
        let names: Vec<_> = merged.items.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["type:feat", "area:core", "area:ssr"]);
        assert_eq!(merged.items[0].color, "abcdef");
        assert_eq!(merged.items[0].description, "local");
    }

    #[test]
    fn spec_has_no_built_in_migrations() {
        assert!(RepoLabelsSpec::default().migrations.is_empty());
    }

    #[test]
    fn overlay_can_override_prune_and_works_without_base() {
        let overlay = RepoLabelsOverlay {
            prune_unmanaged: Some(false),
            items: vec![RepoLabel::new("area:db", "444444", "db")],
            ..RepoLabelsOverlay::default()
        };
        let base = RepoLabelsSpec {
            prune_unmanaged: true,
            ..RepoLabelsSpec::default()
        };
        assert!(!overlay.apply_to(Some(&base)).unwrap().prune_unmanaged);
        let alone = overlay.apply_to(None).unwrap();
        assert_eq!(alone.items.len(), 1);
        assert!(!alone.prune_unmanaged);
    }
}
