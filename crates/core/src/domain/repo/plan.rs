//! Reconciliation planning and state snapshots for repository configuration.

use crate::domain::repo::{
    LabelDiff, MilestoneDiff, RepoLabel, RepoLabelsSpec, RepoMilestone, RepoMilestonesSpec,
    RepoSettings, SettingDiff,
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
}

/// Desired configuration specification for repository entities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoSpec {
    /// Desired repository settings, if configured.
    pub settings: Option<RepoSettings>,
    /// Desired repository labels, if configured.
    pub labels: Option<RepoLabelsSpec>,
    /// Desired repository milestones, if configured.
    pub milestones: Option<RepoMilestonesSpec>,
}

impl RepoSpec {
    /// Creates a new desired repository specification.
    #[must_use]
    pub const fn new(
        settings: Option<RepoSettings>,
        labels: Option<RepoLabelsSpec>,
        milestones: Option<RepoMilestonesSpec>,
    ) -> Self {
        Self {
            settings,
            labels,
            milestones,
        }
    }

    /// Entities targeted by this desired specification.
    #[must_use]
    pub const fn entities(&self) -> RepoEntities {
        RepoEntities {
            settings: self.settings.is_some(),
            labels: self.labels.is_some(),
            milestones: self.milestones.is_some(),
        }
    }

    /// Whether any entity is configured.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.settings.is_none() && self.labels.is_none() && self.milestones.is_none()
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
    pub labels: Option<Vec<RepoLabel>>,
    /// Current repository milestones, if queried.
    pub milestones: Option<Vec<RepoMilestone>>,
}

impl RepoState {
    /// Creates a new repository state snapshot.
    #[must_use]
    pub const fn new(
        settings: Option<RepoSettings>,
        labels: Option<Vec<RepoLabel>>,
        milestones: Option<Vec<RepoMilestone>>,
    ) -> Self {
        Self {
            settings,
            labels,
            milestones,
        }
    }

    /// Computes the reconciliation plan against desired configuration specification.
    #[must_use]
    pub fn plan(&self, desired: &RepoSpec) -> RepoPlan {
        let setting_diffs = desired
            .settings
            .as_ref()
            .and_then(|desired| self.settings.as_ref().map(|current| current.diff(desired)))
            .unwrap_or_default();

        let label_diffs = desired
            .labels
            .as_ref()
            .and_then(|desired| {
                self.labels.as_ref().map(|current| {
                    RepoLabel::diff(
                        current,
                        &desired.items,
                        &desired.deprecated,
                        desired.prune_unmanaged,
                    )
                })
            })
            .unwrap_or_default();

        let milestone_diffs = desired
            .milestones
            .as_ref()
            .and_then(|desired| {
                self.milestones.as_ref().map(|current| {
                    RepoMilestone::diff(
                        current,
                        &desired.items,
                        &desired.deprecated,
                        desired.prune_unmanaged,
                    )
                })
            })
            .unwrap_or_default();

        RepoPlan::new(setting_diffs, label_diffs, milestone_diffs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::{MergeCommitMessage, MergeCommitTitle, MilestoneState};

    #[test]
    fn repo_state_plan_gathers_selected_entity_diffs() {
        let current_state = RepoState::new(
            Some(RepoSettings {
                allow_merge_commit: false,
                allow_squash_merge: false,
                allow_rebase_merge: false,
                delete_branch_on_merge: true,
                merge_commit_title: MergeCommitTitle::PrTitle,
                merge_commit_message: MergeCommitMessage::PrBody,
            }),
            Some(vec![RepoLabel::new("type:feat", "1d76db", "Feature")]),
            Some(vec![RepoMilestone {
                number: 1,
                title: "v0".into(),
                description: String::new(),
                state: MilestoneState::Open,
            }]),
        );

        let desired_settings = RepoSettings {
            allow_merge_commit: true,
            allow_squash_merge: false,
            allow_rebase_merge: false,
            delete_branch_on_merge: true,
            merge_commit_title: MergeCommitTitle::PrTitle,
            merge_commit_message: MergeCommitMessage::PrBody,
        };

        let desired_labels = RepoLabelsSpec {
            deprecated: Vec::new(),
            prune_unmanaged: false,
            items: vec![
                RepoLabel::new("type:feat", "1d76db", "Feature"),
                RepoLabel::new("type:fix", "d73a4a", "Bug fix"),
            ],
        };

        // When only settings selected
        let spec_settings = RepoSpec::new(Some(desired_settings.clone()), None, None);
        let plan_settings = current_state.plan(&spec_settings);
        assert_eq!(plan_settings.setting_diffs().len(), 1);
        assert!(plan_settings.label_diffs().is_empty());
        assert!(plan_settings.milestone_diffs().is_empty());
        assert!(plan_settings.has_actionable_drift());

        // When settings and labels selected
        let spec_all = RepoSpec::new(Some(desired_settings), Some(desired_labels), None);
        let plan_all = current_state.plan(&spec_all);
        assert_eq!(plan_all.setting_diffs().len(), 1);
        assert_eq!(plan_all.label_diffs().len(), 1);
        assert!(plan_all.has_actionable_drift());
    }
}
