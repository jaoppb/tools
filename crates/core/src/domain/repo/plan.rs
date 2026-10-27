//! Reconciliation planning and state snapshots for repository configuration.

use crate::domain::repo::{
    LabelDiff, MilestoneDiff, RepoLabel, RepoMilestone, RepoSettings, SettingDiff,
};

/// Reason why a label or milestone is marked for deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteReason {
    /// Item is explicitly configured in `deprecated` list.
    Deprecated,
    /// Item exists remotely but is not in `tools.toml` and `prune_unmanaged` is true.
    Unmanaged,
}

impl std::fmt::Display for DeleteReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deprecated => write!(formatter, "deprecated"),
            Self::Unmanaged => write!(formatter, "unmanaged"),
        }
    }
}

/// Selection of entity types to query or reconcile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoEntities {
    /// Whether repository settings are selected.
    pub settings: bool,
    /// Whether repository labels are selected.
    pub labels: bool,
    /// Whether repository milestones are selected.
    pub milestones: bool,
}

impl Default for RepoEntities {
    fn default() -> Self {
        Self::ALL
    }
}

impl RepoEntities {
    /// Select all entities (settings, labels, milestones).
    pub const ALL: Self = Self {
        settings: true,
        labels: true,
        milestones: true,
    };

    /// Whether any entity is selected.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.settings || self.labels || self.milestones
    }
}

/// Complete reconciliation plan for repository settings, labels, and milestones.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoPlan {
    setting_diffs: Vec<SettingDiff>,
    label_diffs: Vec<LabelDiff>,
    milestone_diffs: Vec<MilestoneDiff>,
}

impl RepoPlan {
    /// Creates a new plan from diffs.
    #[must_use]
    pub const fn new(
        setting_diffs: Vec<SettingDiff>,
        label_diffs: Vec<LabelDiff>,
        milestone_diffs: Vec<MilestoneDiff>,
    ) -> Self {
        Self {
            setting_diffs,
            label_diffs,
            milestone_diffs,
        }
    }

    /// Whether any differences were detected across any entity.
    #[must_use]
    pub fn has_diffs(&self) -> bool {
        !self.setting_diffs.is_empty()
            || !self.label_diffs.is_empty()
            || !self.milestone_diffs.is_empty()
    }

    /// Whether actionable drift was detected (settings drift or items to create, update, or delete).
    #[must_use]
    pub fn has_actionable_drift(&self) -> bool {
        !self.setting_diffs.is_empty()
            || self.label_diffs.iter().any(LabelDiff::is_actionable)
            || self.milestone_diffs.iter().any(MilestoneDiff::is_actionable)
    }

    /// Setting differences detected.
    #[must_use]
    pub fn setting_diffs(&self) -> &[SettingDiff] {
        &self.setting_diffs
    }

    /// Label differences detected.
    #[must_use]
    pub fn label_diffs(&self) -> &[LabelDiff] {
        &self.label_diffs
    }

    /// Milestone differences detected.
    #[must_use]
    pub fn milestone_diffs(&self) -> &[MilestoneDiff] {
        &self.milestone_diffs
    }
}

impl std::fmt::Display for RepoPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.setting_diffs.is_empty() {
            writeln!(formatter, "Settings:")?;
            for diff in &self.setting_diffs {
                writeln!(formatter, "  • {diff}")?;
            }
        }

        if !self.label_diffs.is_empty() {
            writeln!(formatter, "Labels:")?;
            for diff in &self.label_diffs {
                writeln!(formatter, "  • {diff}")?;
            }
        }

        if !self.milestone_diffs.is_empty() {
            writeln!(formatter, "Milestones:")?;
            for diff in &self.milestone_diffs {
                writeln!(formatter, "  • {diff}")?;
            }
        }

        Ok(())
    }
}

/// Consolidated repository state read from GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoState {
    /// Current repository settings, if queried.
    pub settings: Option<RepoSettings>,
    /// Current repository labels, if queried.
    pub labels: Vec<RepoLabel>,
    /// Current repository milestones, if queried.
    pub milestones: Vec<RepoMilestone>,
}

impl RepoState {
    /// Creates a new repository state snapshot.
    #[must_use]
    pub const fn new(
        settings: Option<RepoSettings>,
        labels: Vec<RepoLabel>,
        milestones: Vec<RepoMilestone>,
    ) -> Self {
        Self {
            settings,
            labels,
            milestones,
        }
    }

    /// Computes the reconciliation plan against desired configurations for selected entities.
    #[must_use]
    pub fn plan(
        &self,
        desired_settings: Option<&RepoSettings>,
        desired_labels: Option<&crate::config::LabelsConfig>,
        desired_milestones: Option<&crate::config::MilestonesConfig>,
        entities: RepoEntities,
    ) -> RepoPlan {
        let setting_diffs = if entities.settings
            && let Some(desired) = desired_settings
            && let Some(ref current) = self.settings
        {
            current.diff(desired)
        } else {
            Vec::new()
        };

        let label_diffs = if entities.labels
            && let Some(desired) = desired_labels
        {
            RepoLabel::diff(
                &self.labels,
                &desired.items,
                &desired.deprecated,
                desired.prune_unmanaged,
            )
        } else {
            Vec::new()
        };

        let milestone_diffs = if entities.milestones
            && let Some(desired) = desired_milestones
        {
            crate::domain::repo::RepoMilestone::diff(
                &self.milestones,
                &desired.items,
                &desired.deprecated,
                desired.prune_unmanaged,
            )
        } else {
            Vec::new()
        };

        RepoPlan::new(setting_diffs, label_diffs, milestone_diffs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::MilestoneState;

    #[test]
    fn repo_state_plan_gathers_selected_entity_diffs() {
        let current_state = RepoState::new(
            Some(RepoSettings {
                allow_merge_commit: false,
                allow_squash_merge: false,
                allow_rebase_merge: false,
                delete_branch_on_merge: true,
                merge_commit_title: "PR_TITLE".into(),
                merge_commit_message: "PR_BODY".into(),
            }),
            vec![RepoLabel::new("type:feat", "1d76db", "Feature")],
            vec![RepoMilestone {
                number: 1,
                title: "v0".into(),
                description: None,
                state: MilestoneState::Open,
            }],
        );

        let desired_settings = RepoSettings {
            allow_merge_commit: true,
            allow_squash_merge: false,
            allow_rebase_merge: false,
            delete_branch_on_merge: true,
            merge_commit_title: "PR_TITLE".into(),
            merge_commit_message: "PR_BODY".into(),
        };

        let desired_labels = crate::config::LabelsConfig {
            deprecated: Vec::new(),
            prune_unmanaged: false,
            items: vec![
                RepoLabel::new("type:feat", "1d76db", "Feature"),
                RepoLabel::new("type:fix", "d73a4a", "Bug fix"),
            ],
        };

        // When only settings selected
        let plan_settings = current_state.plan(
            Some(&desired_settings),
            Some(&desired_labels),
            None,
            RepoEntities {
                settings: true,
                labels: false,
                milestones: false,
            },
        );
        assert_eq!(plan_settings.setting_diffs().len(), 1);
        assert!(plan_settings.label_diffs().is_empty());
        assert!(plan_settings.milestone_diffs().is_empty());
        assert!(plan_settings.has_actionable_drift());

        // When all selected
        let plan_all = current_state.plan(
            Some(&desired_settings),
            Some(&desired_labels),
            None,
            RepoEntities::ALL,
        );
        assert_eq!(plan_all.setting_diffs().len(), 1);
        assert_eq!(plan_all.label_diffs().len(), 1);
        assert!(plan_all.has_actionable_drift());
    }
}
