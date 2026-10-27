//! Repository milestone representation and diffing.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::domain::repo::DeleteReason;

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
    pub description: Option<String>,
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
        let current_by_title: HashMap<&str, &Self> =
            current.iter().map(|m| (m.title.as_str(), m)).collect();
        let desired_by_title: HashMap<&str, &RepoMilestoneSpec> =
            desired.iter().map(|m| (m.title.as_str(), m)).collect();
        let deprecated_titles: HashSet<&str> = deprecated.iter().map(String::as_str).collect();

        // 1. Reconcile desired milestones against current (O(M))
        for item in desired {
            if let Some(existing) = current_by_title.get(item.title.as_str()) {
                let cur_desc = existing.description.as_deref().unwrap_or_default();
                let des_desc = item.description.as_deref().unwrap_or_default();
                let desc_differs = cur_desc != des_desc;
                let state_differs = existing.state != item.state;

                if desc_differs || state_differs {
                    diffs.push(MilestoneDiff::Update {
                        current: (*existing).clone(),
                        desired: item.clone(),
                    });
                }
            } else {
                diffs.push(MilestoneDiff::Create(item.clone()));
            }
        }

        // 2. Reconcile current milestones for deprecated or unmanaged (O(N))
        for milestone in current {
            let title = milestone.title.as_str();
            if desired_by_title.contains_key(title) {
                // Already reconciled in Step 1. Desired takes precedence.
                continue;
            }

            if deprecated_titles.contains(title) {
                diffs.push(MilestoneDiff::Delete {
                    number: milestone.number,
                    title: milestone.title.clone(),
                    reason: DeleteReason::Deprecated,
                });
            } else if prune_unmanaged {
                diffs.push(MilestoneDiff::Delete {
                    number: milestone.number,
                    title: milestone.title.clone(),
                    reason: DeleteReason::Unmanaged,
                });
            } else {
                diffs.push(MilestoneDiff::Unmanaged {
                    title: milestone.title.clone(),
                });
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
                current,
                desired,
            } if current.number == 1 && desired.title == "v0" && current.description.as_deref() == Some("Old bootstrap") && desired.description.as_deref() == Some("New bootstrap")
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
            MilestoneDiff::Delete { number: 4, title, reason: DeleteReason::Unmanaged } if title == "unmanaged-m"
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
                current,
                desired,
            } => {
                assert_eq!(current.number, 5);
                assert_eq!(current.state, MilestoneState::Closed);
                assert_eq!(desired.state, MilestoneState::Closed);
                assert_eq!(desired.description.as_deref(), Some("New updated text"));
            }
            other => panic!("expected MilestoneDiff::Update, got {:?}", other),
        }
    }

    #[test]
    fn milestone_diff_ignores_deprecated_if_item_is_in_desired() {
        let current = vec![RepoMilestone {
            number: 1,
            title: "v0".into(),
            description: None,
            state: MilestoneState::Open,
        }];
        let desired = vec![RepoMilestoneSpec {
            title: "v0".into(),
            description: None,
            state: MilestoneState::Open,
        }];
        // "v0" mistakenly also in deprecated
        let deprecated = vec!["v0".into()];

        let diffs = RepoMilestone::diff(&current, &desired, &deprecated, false);
        // Should NOT emit a Delete; desired takes precedence and it is in sync
        assert!(diffs.is_empty());
    }
}
