//! Repository milestone representation and diffing.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::domain::repo::{DeleteReason, ReconcileHandler, RepoPlanError, reconcile_by_key};

/// Repository milestone state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MilestoneState {
    /// Active, open milestone.
    #[default]
    Open,
    /// Closed milestone.
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
    pub description: String,
    /// Milestone state (`open` or `closed`).
    #[serde(default)]
    pub state: MilestoneState,
}

/// Repository milestones specification and reconciliation policy.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct RepoMilestonesSpec {
    /// Deprecated milestone titles to close/prune.
    #[serde(default)]
    pub deprecated: Vec<String>,
    /// Rename mappings from old milestone title to new milestone title.
    #[serde(default)]
    pub renames: BTreeMap<String, String>,
    /// Whether to prune unmanaged milestones.
    #[serde(default)]
    pub prune_unmanaged: bool,
    /// Desired milestones to enforce.
    #[serde(default)]
    pub items: Vec<RepoMilestoneSpec>,
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
    pub description: String,
    /// Milestone state (`open` or `closed`).
    #[serde(default)]
    pub state: MilestoneState,
}

impl RepoMilestone {
    /// Computes differences between current milestones and desired milestone configuration.
    ///
    /// # Errors
    ///
    /// Returns [`RepoPlanError::Validation`] on configuration errors or [`RepoPlanError::Collision`]
    /// on remote collisions.
    pub fn diff(
        current: &[Self],
        desired: &[RepoMilestoneSpec],
        deprecated: &[String],
        renames: &BTreeMap<String, String>,
        prune_unmanaged: bool,
    ) -> Result<Vec<MilestoneDiff>, RepoPlanError> {
        reconcile_by_key(
            current,
            desired,
            deprecated,
            renames,
            prune_unmanaged,
            MilestoneReconciler,
        )
    }
}

struct MilestoneReconciler;

impl ReconcileHandler<RepoMilestone, RepoMilestoneSpec, MilestoneDiff> for MilestoneReconciler {
    fn key_current<'a>(&self, item: &'a RepoMilestone) -> &'a str {
        &item.title
    }

    fn key_desired<'a>(&self, item: &'a RepoMilestoneSpec) -> &'a str {
        &item.title
    }

    fn diff(&self, current: &RepoMilestone, desired: &RepoMilestoneSpec) -> Option<MilestoneDiff> {
        let title_differs = current.title != desired.title;
        let desc_differs = current.description != desired.description;
        let state_differs = current.state != desired.state;
        if title_differs || desc_differs || state_differs {
            Some(MilestoneDiff::Update {
                current: current.clone(),
                desired: desired.clone(),
            })
        } else {
            None
        }
    }

    fn create(&self, desired: &RepoMilestoneSpec) -> MilestoneDiff {
        MilestoneDiff::Create(desired.clone())
    }

    fn delete(&self, current: &RepoMilestone, reason: DeleteReason) -> MilestoneDiff {
        MilestoneDiff::Delete {
            number: current.number,
            title: current.title.clone(),
            reason,
        }
    }

    fn unmanaged(&self, current: &RepoMilestone) -> MilestoneDiff {
        MilestoneDiff::Unmanaged {
            title: current.title.clone(),
        }
    }
}

/// A difference or required action for repository milestones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MilestoneDiff {
    /// Milestone exists in config but is missing on GitHub.
    Create(RepoMilestoneSpec),
    /// Milestone exists on GitHub but description or state differs.
    Update {
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
        reason: DeleteReason,
    },
    /// Milestone exists on GitHub but is unmanaged (reported when pruning is disabled).
    Unmanaged {
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
            Self::Update { current, desired } => {
                let mut changes = Vec::new();
                if current.title != desired.title {
                    changes.push(format!("title '{}' -> '{}'", current.title, desired.title));
                }
                if current.description != desired.description {
                    changes.push(format!(
                        "description '{}' -> '{}'",
                        current.description, desired.description
                    ));
                }
                if current.state != desired.state {
                    changes.push(format!("state '{}' -> '{}'", current.state, desired.state));
                }
                write!(
                    formatter,
                    "milestone '{}': update {}",
                    desired.title,
                    changes.join(", ")
                )
            }
            Self::Delete {
                number: _,
                title,
                reason,
            } => {
                write!(formatter, "milestone '{title}': delete ({reason})")
            }
            Self::Unmanaged { title } => {
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
    fn milestone_diff_detects_creates_updates_deletes_and_drift() {
        let current = vec![
            RepoMilestone {
                number: 1,
                title: "v0".into(),
                description: "Old bootstrap".into(),
                state: MilestoneState::Open,
            },
            RepoMilestone {
                number: 2,
                title: "v0.9".into(),
                description: String::new(),
                state: MilestoneState::Open,
            },
            RepoMilestone {
                number: 3,
                title: "historical".into(),
                description: String::new(),
                state: MilestoneState::Closed,
            },
        ];

        let desired_items = vec![
            RepoMilestoneSpec {
                title: "v0".into(),
                description: "New bootstrap".into(),
                state: MilestoneState::Open,
            },
            RepoMilestoneSpec {
                title: "v1".into(),
                description: "Stable release".into(),
                state: MilestoneState::Open,
            },
        ];
        let deprecated = vec!["v0.9".into()];
        let renames = BTreeMap::new();

        let diffs =
            RepoMilestone::diff(&current, &desired_items, &deprecated, &renames, false).unwrap();
        assert_eq!(diffs.len(), 4);

        // 1. v0 updated description
        assert!(matches!(
            &diffs[0],
            MilestoneDiff::Update {
                current,
                desired,
            } if current.number == 1 && desired.title == "v0" && current.description == "Old bootstrap" && desired.description == "New bootstrap"
        ));

        // 2. v1 created
        assert!(matches!(
            &diffs[1],
            MilestoneDiff::Create(m) if m.title == "v1"
        ));

        // 3. v0.9 deleted/deprecated
        assert!(matches!(
            &diffs[2],
            MilestoneDiff::Delete { number: 2, title, reason: DeleteReason::Deprecated } if title == "v0.9"
        ));

        // 4. historical unmanaged
        assert!(matches!(
            &diffs[3],
            MilestoneDiff::Unmanaged { title } if title == "historical"
        ));
    }

    #[test]
    fn milestone_diff_matches_case_insensitively_and_updates_casing() {
        let current = vec![RepoMilestone {
            number: 1,
            title: "V0".into(),
            description: "Bootstrap".into(),
            state: MilestoneState::Open,
        }];

        let desired_items = vec![RepoMilestoneSpec {
            title: "v0".into(),
            description: "Bootstrap".into(),
            state: MilestoneState::Open,
        }];
        let renames = BTreeMap::new();

        let diffs = RepoMilestone::diff(&current, &desired_items, &[], &renames, false).unwrap();
        assert_eq!(diffs.len(), 1);
        match &diffs[0] {
            MilestoneDiff::Update { current, desired } => {
                assert_eq!(current.number, 1);
                assert_eq!(current.title, "V0");
                assert_eq!(desired.title, "v0");
            }
            other => panic!("expected MilestoneDiff::Update, got {:?}", other),
        }
    }

    #[test]
    fn milestone_diff_prunes_unmanaged_with_unmanaged_reason() {
        let current = vec![RepoMilestone {
            number: 4,
            title: "unmanaged-m".into(),
            description: String::new(),
            state: MilestoneState::Open,
        }];
        let renames = BTreeMap::new();

        let diffs = RepoMilestone::diff(&current, &[], &[], &renames, true).unwrap();
        assert_eq!(diffs.len(), 1);
        assert!(matches!(
            &diffs[0],
            MilestoneDiff::Delete { number: 4, title, reason: DeleteReason::Unmanaged } if title == "unmanaged-m"
        ));
    }

    #[test]
    fn milestone_diff_reconciles_renames_in_place() {
        let current = vec![RepoMilestone {
            number: 10,
            title: "v0.9".into(),
            description: "Release candidate".into(),
            state: MilestoneState::Open,
        }];
        let desired = vec![RepoMilestoneSpec {
            title: "v1.0".into(),
            description: "General availability".into(),
            state: MilestoneState::Open,
        }];
        let mut renames = BTreeMap::new();
        renames.insert("v0.9".into(), "v1.0".into());

        let diffs = RepoMilestone::diff(&current, &desired, &[], &renames, false).unwrap();
        assert_eq!(diffs.len(), 1);
        match &diffs[0] {
            MilestoneDiff::Update { current, desired } => {
                assert_eq!(current.number, 10);
                assert_eq!(current.title, "v0.9");
                assert_eq!(desired.title, "v1.0");
                assert_eq!(desired.description, "General availability");
            }
            other => panic!("expected MilestoneDiff::Update, got {:?}", other),
        }
    }

    #[test]
    fn milestone_diff_preserves_closed_state_when_updating_description() {
        let current = vec![RepoMilestone {
            number: 5,
            title: "v0.8".into(),
            description: "Old text".into(),
            state: MilestoneState::Closed,
        }];

        let desired_items = vec![RepoMilestoneSpec {
            title: "v0.8".into(),
            description: "New updated text".into(),
            state: MilestoneState::Closed,
        }];
        let renames = BTreeMap::new();

        let diffs = RepoMilestone::diff(&current, &desired_items, &[], &renames, false).unwrap();
        assert_eq!(diffs.len(), 1);
        match &diffs[0] {
            MilestoneDiff::Update { current, desired } => {
                assert_eq!(current.number, 5);
                assert_eq!(current.state, MilestoneState::Closed);
                assert_eq!(desired.state, MilestoneState::Closed);
                assert_eq!(desired.description, "New updated text");
            }
            other => panic!("expected MilestoneDiff::Update, got {:?}", other),
        }
    }

    #[test]
    fn milestone_diff_ignores_deprecated_if_item_is_in_desired() {
        let current = vec![RepoMilestone {
            number: 1,
            title: "v0".into(),
            description: String::new(),
            state: MilestoneState::Open,
        }];
        let desired = vec![RepoMilestoneSpec {
            title: "v0".into(),
            description: String::new(),
            state: MilestoneState::Open,
        }];
        // "v0" mistakenly also in deprecated (with differing casing)
        let deprecated = vec!["V0".into()];
        let renames = BTreeMap::new();

        let diffs = RepoMilestone::diff(&current, &desired, &deprecated, &renames, false).unwrap();
        // Should NOT emit a Delete; desired takes precedence and it is in sync
        assert!(diffs.is_empty());
    }

    #[test]
    fn milestone_diff_detects_collision_when_both_source_and_target_exist() {
        let current = vec![
            RepoMilestone {
                number: 1,
                title: "v0.9".into(),
                description: String::new(),
                state: MilestoneState::Open,
            },
            RepoMilestone {
                number: 2,
                title: "v1.0".into(),
                description: String::new(),
                state: MilestoneState::Open,
            },
        ];
        let desired = vec![RepoMilestoneSpec {
            title: "v1.0".into(),
            description: String::new(),
            state: MilestoneState::Open,
        }];
        let mut renames = BTreeMap::new();
        renames.insert("v0.9".into(), "v1.0".into());

        let err = RepoMilestone::diff(&current, &desired, &[], &renames, false).unwrap_err();
        assert!(matches!(err, RepoPlanError::Collision(_)));
        assert!(err.to_string().contains("v0.9"));
        assert!(err.to_string().contains("v1.0"));
    }
}
