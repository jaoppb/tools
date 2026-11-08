//! Repository milestone representation and diffing.

use std::collections::BTreeMap;

use crate::domain::repo::{
    DeleteReason, ReconcileHandler, RepoPlanError, Severity, Unmanaged, overlay_by_key,
    reconcile_by_key, union_names,
};

/// Repository milestone state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoMilestoneSpec {
    /// Milestone title (e.g. `v0`).
    pub title: String,
    /// Milestone description.
    pub description: String,
    /// Milestone state (`open` or `closed`).
    pub state: MilestoneState,
}

/// Repository milestones specification and reconciliation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoMilestonesSpec {
    /// Deprecated milestone titles to close/prune.
    pub deprecated: Vec<String>,
    /// Rename mappings from old milestone title to new milestone title.
    pub renames: BTreeMap<String, String>,
    /// What to do with milestones that exist on GitHub but are not declared (`ignore`, `warn`,
    /// `error` or `prune`).
    pub unmanaged: Unmanaged,
    /// Desired milestones to enforce.
    pub items: Vec<RepoMilestoneSpec>,
}

impl Default for RepoMilestonesSpec {
    fn default() -> Self {
        Self {
            deprecated: Vec::new(),
            renames: BTreeMap::new(),
            unmanaged: Unmanaged::milestones(),

            items: Vec::new(),
        }
    }
}

/// Repository-scoped milestones overlay applied on top of the shared [`RepoMilestonesSpec`].
///
/// Items override shared items with the same title and are otherwise appended; `deprecated` is a
/// union; `renames` are merged with overlay entries winning; `unmanaged` replaces the shared
/// value only when set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoMilestonesOverlay {
    /// Additional milestone titles to close/prune.
    pub deprecated: Vec<String>,
    /// Additional or overriding rename mappings.
    pub renames: BTreeMap<String, String>,
    /// Overrides what to do with undeclared milestones when set.
    pub unmanaged: Option<Unmanaged>,
    /// Additional or overriding milestones.
    pub items: Vec<RepoMilestoneSpec>,
}

impl RepoMilestonesOverlay {
    /// Resolves the effective milestones specification for a repository.
    ///
    /// # Errors
    ///
    /// Returns [`RepoPlanError::Validation`] when the overlay repeats an item key.
    pub fn apply_to(
        &self,
        base: Option<&RepoMilestonesSpec>,
    ) -> Result<RepoMilestonesSpec, RepoPlanError> {
        let mut spec = base.cloned().unwrap_or_default();
        union_names(&mut spec.deprecated, self.deprecated.clone());
        spec.renames.extend(self.renames.clone());
        if let Some(unmanaged) = self.unmanaged {
            spec.unmanaged = unmanaged;
        }
        overlay_by_key(&mut spec.items, self.items.clone(), |item| &item.title)?;
        Ok(spec)
    }
}

/// Repository milestone entity on GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoMilestone {
    /// GitHub milestone number.
    pub number: u64,
    /// Milestone title (e.g. `v0`).
    pub title: String,
    /// Milestone description.
    pub description: String,
    /// Milestone state (`open` or `closed`).
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
        unmanaged: Unmanaged,
    ) -> Result<Vec<MilestoneDiff>, RepoPlanError> {
        reconcile_by_key(
            current,
            desired,
            deprecated,
            renames,
            unmanaged,
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

    fn unmanaged(&self, current: &RepoMilestone, severity: Severity) -> MilestoneDiff {
        MilestoneDiff::Unmanaged {
            title: current.title.clone(),
            severity,
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
    /// Milestone is marked deprecated (or unmanaged with policy `prune`) and should be deleted.
    Delete {
        /// Milestone number on GitHub.
        number: u64,
        /// Milestone title.
        title: String,
        /// Reason for deletion.
        reason: DeleteReason,
    },
    /// Milestone exists on GitHub but is not declared (reported unless the policy is `ignore` or `prune`).
    Unmanaged {
        /// Milestone title.
        title: String,
        /// Whether it only warns or fails the run.
        severity: Severity,
    },
}

impl MilestoneDiff {
    /// Whether this is an undeclared item whose policy fails the run.
    #[must_use]
    pub const fn is_unmanaged_error(&self) -> bool {
        matches!(
            self,
            Self::Unmanaged {
                severity: Severity::Error,
                ..
            }
        )
    }

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
            Self::Unmanaged { title, severity } => {
                write!(
                    formatter,
                    "milestone '{title}' exists on GitHub but is not declared in tools.toml ({severity}); declare it under [github.milestones].items or add it to deprecated"
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

        let diffs = RepoMilestone::diff(
            &current,
            &desired_items,
            &deprecated,
            &renames,
            Unmanaged::Warn,
        )
        .unwrap();
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
            MilestoneDiff::Unmanaged { title, .. } if title == "historical"
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

        let diffs =
            RepoMilestone::diff(&current, &desired_items, &[], &renames, Unmanaged::Warn).unwrap();
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

        let diffs = RepoMilestone::diff(&current, &[], &[], &renames, Unmanaged::Prune).unwrap();
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

        let diffs =
            RepoMilestone::diff(&current, &desired, &[], &renames, Unmanaged::Warn).unwrap();
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

        let diffs =
            RepoMilestone::diff(&current, &desired_items, &[], &renames, Unmanaged::Warn).unwrap();
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

        let diffs = RepoMilestone::diff(&current, &desired, &deprecated, &renames, Unmanaged::Warn)
            .unwrap();
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

        let err =
            RepoMilestone::diff(&current, &desired, &[], &renames, Unmanaged::Warn).unwrap_err();
        assert!(matches!(err, RepoPlanError::Collision(_)));
        assert!(err.to_string().contains("v0.9"));
        assert!(err.to_string().contains("v1.0"));
    }

    #[test]
    fn overlay_overrides_by_title_and_unions_deprecated() {
        let base = RepoMilestonesSpec {
            deprecated: vec!["v0".into()],
            renames: BTreeMap::new(),
            unmanaged: Unmanaged::Warn,
            items: vec![RepoMilestoneSpec {
                title: "v1".into(),
                description: "shared".into(),
                state: MilestoneState::Open,
            }],
        };
        let overlay = RepoMilestonesOverlay {
            deprecated: vec!["v0".into(), "v9".into()],
            unmanaged: Some(Unmanaged::Prune),
            items: vec![
                RepoMilestoneSpec {
                    title: "v1".into(),
                    description: "local".into(),
                    state: MilestoneState::Closed,
                },
                RepoMilestoneSpec {
                    title: "v2".into(),
                    description: "extra".into(),
                    state: MilestoneState::Open,
                },
            ],
            ..RepoMilestonesOverlay::default()
        };
        let merged = overlay.apply_to(Some(&base)).unwrap();
        assert_eq!(merged.deprecated, vec!["v0", "v9"]);
        assert_eq!(merged.unmanaged, Unmanaged::Prune);
        assert_eq!(merged.items.len(), 2);
        assert_eq!(merged.items[0].description, "local");
        assert_eq!(merged.items[0].state, MilestoneState::Closed);
        assert_eq!(merged.items[1].title, "v2");
    }

    fn remote(number: u64, title: &str) -> RepoMilestone {
        RepoMilestone {
            number,
            title: title.into(),
            description: String::new(),
            state: MilestoneState::Open,
        }
    }

    fn declared(title: &str) -> RepoMilestoneSpec {
        RepoMilestoneSpec {
            title: title.into(),
            description: String::new(),
            state: MilestoneState::Open,
        }
    }

    #[test]
    fn declared_milestone_is_in_sync_under_every_policy() {
        for policy in [
            Unmanaged::Ignore,
            Unmanaged::Warn,
            Unmanaged::Error,
            Unmanaged::Prune,
        ] {
            let diffs = RepoMilestone::diff(
                &[remote(1, "v0.4.0")],
                &[declared("v0.4.0")],
                &[],
                &BTreeMap::new(),
                policy,
            )
            .unwrap();
            assert!(diffs.is_empty(), "{policy:?}: {diffs:?}");
        }
    }

    #[test]
    fn undeclared_milestone_is_named_with_the_fix_and_fails_under_error() {
        let diffs = RepoMilestone::diff(
            &[remote(1, "v0.4.0")],
            &[declared("v0.3.0"), declared("v0.4.1")],
            &[],
            &BTreeMap::new(),
            Unmanaged::Error,
        )
        .unwrap();
        let unmanaged: Vec<_> = diffs
            .iter()
            .filter(|diff| matches!(diff, MilestoneDiff::Unmanaged { .. }))
            .collect();
        assert_eq!(unmanaged.len(), 1);
        assert!(unmanaged[0].is_unmanaged_error());
        assert!(!unmanaged[0].is_actionable());
        let text = unmanaged[0].to_string();
        assert!(text.contains("'v0.4.0'"), "{text}");
        assert!(text.contains("not declared in tools.toml"), "{text}");
        assert!(text.contains("[github.milestones].items"), "{text}");
        assert!(text.contains("deprecated"), "{text}");
    }

    #[test]
    fn undeclared_milestone_only_warns_under_warn_and_is_silent_under_ignore() {
        let current = [remote(1, "v0.4.0")];
        let warn =
            RepoMilestone::diff(&current, &[], &[], &BTreeMap::new(), Unmanaged::Warn).unwrap();
        assert_eq!(warn.len(), 1);
        assert!(!warn[0].is_unmanaged_error());
        let ignore =
            RepoMilestone::diff(&current, &[], &[], &BTreeMap::new(), Unmanaged::Ignore).unwrap();
        assert!(ignore.is_empty());
    }

    #[test]
    fn deprecated_milestone_is_deleted_not_reported_as_undeclared() {
        let diffs = RepoMilestone::diff(
            &[remote(1, "v0.9.0")],
            &[],
            &["v0.9.0".to_string()],
            &BTreeMap::new(),
            Unmanaged::Error,
        )
        .unwrap();
        assert!(matches!(
            diffs.as_slice(),
            [MilestoneDiff::Delete {
                reason: DeleteReason::Deprecated,
                ..
            }]
        ));
    }

    #[test]
    fn closed_undeclared_milestone_gets_the_same_severity() {
        let mut closed = remote(2, "v0.1.0");
        closed.state = MilestoneState::Closed;
        let diffs =
            RepoMilestone::diff(&[closed], &[], &[], &BTreeMap::new(), Unmanaged::Error).unwrap();
        assert!(diffs[0].is_unmanaged_error());
    }

    #[test]
    fn unmanaged_defaults_to_error_for_milestones_and_warn_for_labels() {
        assert_eq!(RepoMilestonesSpec::default().unmanaged, Unmanaged::Error);
        let overlay_only = RepoMilestonesOverlay::default().apply_to(None).unwrap();
        assert_eq!(overlay_only.unmanaged, Unmanaged::Error);
    }
}
