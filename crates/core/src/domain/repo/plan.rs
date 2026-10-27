//! Reconciliation planning and state snapshots for repository configuration.

use crate::domain::repo::{
    LabelDiff, MilestoneDiff, RepoLabel, RepoLabelsSpec, RepoMilestone, RepoMilestonesSpec,
    RepoSettings, SettingsDiff,
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

/// An error computing the reconciliation plan from repository state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoPlanError {
    /// Settings requested in desired specification but absent from repository state snapshot.
    #[error("missing repository settings state: settings requested in desired specification but absent from snapshot")]
    MissingSettings,
    /// Labels requested in desired specification but absent from repository state snapshot.
    #[error("missing repository labels state: labels requested in desired specification but absent from snapshot")]
    MissingLabels,
    /// Milestones requested in desired specification but absent from repository state snapshot.
    #[error("missing repository milestones state: milestones requested in desired specification but absent from snapshot")]
    MissingMilestones,
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

    /// Whether any entity is configured.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.settings.is_none() && self.labels.is_none() && self.milestones.is_none()
    }
}

/// Complete reconciliation plan for repository settings, labels, and milestones.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoPlan {
    setting_diffs: SettingsDiff,
    label_diffs: Vec<LabelDiff>,
    milestone_diffs: Vec<MilestoneDiff>,
}

impl RepoPlan {
    /// Creates a new plan from diffs.
    #[must_use]
    pub const fn new(
        setting_diffs: SettingsDiff,
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
    pub fn setting_diffs(&self) -> &SettingsDiff {
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
    ///
    /// # Errors
    ///
    /// Returns [`RepoPlanError`] when an entity is requested in `desired` but its snapshot is missing (`None`) in `self`.
    pub fn plan(&self, desired: &RepoSpec) -> Result<RepoPlan, RepoPlanError> {
        let setting_diffs = match desired.settings.as_ref() {
            Some(desired_settings) => {
                let current_settings = self.settings.as_ref().ok_or(RepoPlanError::MissingSettings)?;
                current_settings.diff(desired_settings)
            }
            None => SettingsDiff::default(),
        };

        let label_diffs = match desired.labels.as_ref() {
            Some(desired_labels) => {
                let current_labels = self.labels.as_ref().ok_or(RepoPlanError::MissingLabels)?;
                RepoLabel::diff(
                    current_labels,
                    &desired_labels.items,
                    &desired_labels.deprecated,
                    desired_labels.prune_unmanaged,
                )
            }
            None => Vec::new(),
        };

        let milestone_diffs = match desired.milestones.as_ref() {
            Some(desired_milestones) => {
                let current_milestones = self.milestones.as_ref().ok_or(RepoPlanError::MissingMilestones)?;
                RepoMilestone::diff(
                    current_milestones,
                    &desired_milestones.items,
                    &desired_milestones.deprecated,
                    desired_milestones.prune_unmanaged,
                )
            }
            None => Vec::new(),
        };

        Ok(RepoPlan::new(setting_diffs, label_diffs, milestone_diffs))
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
        let plan_settings = current_state.plan(&spec_settings).unwrap();
        assert_eq!(plan_settings.setting_diffs().len(), 1);
        assert!(plan_settings.label_diffs().is_empty());
        assert!(plan_settings.milestone_diffs().is_empty());
        assert!(plan_settings.has_actionable_drift());

        // When settings and labels selected
        let spec_all = RepoSpec::new(Some(desired_settings), Some(desired_labels), None);
        let plan_all = current_state.plan(&spec_all).unwrap();
        assert_eq!(plan_all.setting_diffs().len(), 1);
        assert_eq!(plan_all.label_diffs().len(), 1);
        assert!(plan_all.has_actionable_drift());
    }

    #[test]
    fn repo_state_plan_fails_loud_when_requested_entity_state_is_missing() {
        let current_empty = RepoState::default();

        let desired_settings = RepoSpec::new(
            Some(RepoSettings {
                allow_merge_commit: true,
                allow_squash_merge: false,
                allow_rebase_merge: false,
                delete_branch_on_merge: true,
                merge_commit_title: MergeCommitTitle::PrTitle,
                merge_commit_message: MergeCommitMessage::PrBody,
            }),
            None,
            None,
        );
        assert_eq!(
            current_empty.plan(&desired_settings).unwrap_err(),
            RepoPlanError::MissingSettings
        );

        let desired_labels = RepoSpec::new(
            None,
            Some(RepoLabelsSpec {
                deprecated: Vec::new(),
                prune_unmanaged: false,
                items: vec![RepoLabel::new("type:feat", "1d76db", "Feature")],
            }),
            None,
        );
        assert_eq!(
            current_empty.plan(&desired_labels).unwrap_err(),
            RepoPlanError::MissingLabels
        );

        let desired_milestones = RepoSpec::new(
            None,
            None,
            Some(RepoMilestonesSpec {
                deprecated: Vec::new(),
                prune_unmanaged: false,
                items: Vec::new(),
            }),
        );
        assert_eq!(
            current_empty.plan(&desired_milestones).unwrap_err(),
            RepoPlanError::MissingMilestones
        );
    }
}
