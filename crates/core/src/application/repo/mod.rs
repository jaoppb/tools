//! Synchronizing and auditing repository settings, labels, and milestones.

use std::fmt;

use crate::adapters::github::GhRepo;
use crate::adapters::github::repo::RepoEntities;
use crate::config::{LabelsConfig, MilestonesConfig};
use crate::domain::repo::{
    LabelDiff, MilestoneDiff, RepoLabel, RepoMilestone, RepoSettings, SettingDiff,
};

/// Execution mode for repository synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoSyncMode {
    /// Audit only: report differences and return an error if settings or entities drift.
    Check,
    /// Dry run: report differences without applying them, succeeding.
    Preview,
    /// Apply: update GitHub repository to match desired configuration.
    Apply,
}

/// Errors occurring during repository synchronization.
#[derive(Debug, thiserror::Error)]
pub enum RepoSyncError {
    /// Failed to read current repository state.
    #[error("failed to read repository state: {0}")]
    Read(String),
    /// Failed to apply updated state.
    #[error("failed to update repository: {0}")]
    Write(String),
    /// State drifted in check mode.
    #[error("repository drift detected:\n{0}")]
    Drift(String),
}

/// Outcome of a repository sync run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSyncOutcome {
    setting_diffs: Vec<SettingDiff>,
    label_diffs: Vec<LabelDiff>,
    milestone_diffs: Vec<MilestoneDiff>,
    mode: RepoSyncMode,
}

impl RepoSyncOutcome {
    /// New outcome with diffs under `mode`.
    #[must_use]
    pub const fn new(
        setting_diffs: Vec<SettingDiff>,
        label_diffs: Vec<LabelDiff>,
        milestone_diffs: Vec<MilestoneDiff>,
        mode: RepoSyncMode,
    ) -> Self {
        Self {
            setting_diffs,
            label_diffs,
            milestone_diffs,
            mode,
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
            || self.label_diffs.iter().any(|d| !matches!(d, LabelDiff::Unmanaged { .. }))
            || self.milestone_diffs.iter().any(|d| !matches!(d, MilestoneDiff::Unmanaged { .. }))
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

impl fmt::Display for RepoSyncOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        if !self.has_diffs() {
            return writeln!(formatter, "repository configuration is in sync");
        }

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

        match self.mode {
            RepoSyncMode::Check => {
                if self.has_actionable_drift() {
                    writeln!(formatter, "drift detected (check failed)")
                } else {
                    writeln!(formatter, "unmanaged items detected (pruning disabled)")
                }
            }
            RepoSyncMode::Preview => writeln!(formatter, "dry-run: no changes applied"),
            RepoSyncMode::Apply => writeln!(formatter, "repository successfully updated"),
        }
    }
}

/// Orchestrates repository audit and synchronization.
#[derive(Debug)]
pub struct RepoSync {
    repo: GhRepo,
    desired_settings: Option<RepoSettings>,
    desired_labels: Option<LabelsConfig>,
    desired_milestones: Option<MilestonesConfig>,
    entities: RepoEntities,
}

impl RepoSync {
    /// Creates a `RepoSync` handler with all desired configurations and selected entities.
    #[must_use]
    pub const fn new(
        repo: GhRepo,
        desired_settings: Option<RepoSettings>,
        desired_labels: Option<LabelsConfig>,
        desired_milestones: Option<MilestonesConfig>,
        entities: RepoEntities,
    ) -> Self {
        Self {
            repo,
            desired_settings,
            desired_labels,
            desired_milestones,
            entities,
        }
    }

    /// Runs repository sync under `mode`.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError`] if reading or writing fails, or if in [`RepoSyncMode::Check`] and actionable drift is found.
    pub fn run(&self, mode: RepoSyncMode) -> Result<RepoSyncOutcome, RepoSyncError> {
        let current_state = self
            .repo
            .get_state(self.entities)
            .map_err(|err| RepoSyncError::Read(err.to_string()))?;

        let mut setting_diffs = Vec::new();
        if self.entities.settings
            && let Some(ref desired) = self.desired_settings
            && let Some(ref current) = current_state.settings
        {
            setting_diffs = current.diff(desired);
        }

        let mut label_diffs = Vec::new();
        if self.entities.labels
            && let Some(ref desired) = self.desired_labels
        {
            label_diffs = RepoLabel::diff(&current_state.labels, desired);
        }

        let mut milestone_diffs = Vec::new();
        if self.entities.milestones
            && let Some(ref desired) = self.desired_milestones
        {
            milestone_diffs = RepoMilestone::diff(&current_state.milestones, desired);
        }

        let outcome = RepoSyncOutcome::new(setting_diffs, label_diffs, milestone_diffs, mode);

        if !outcome.has_diffs() {
            return Ok(outcome);
        }

        match mode {
            RepoSyncMode::Check => {
                if outcome.has_actionable_drift() {
                    let report = outcome.to_string();
                    Err(RepoSyncError::Drift(report))
                } else {
                    Ok(outcome)
                }
            }
            RepoSyncMode::Preview => Ok(outcome),
            RepoSyncMode::Apply => {
                // 1. Apply settings updates
                if !outcome.setting_diffs.is_empty()
                    && let Some(ref desired) = self.desired_settings
                {
                    self.repo
                        .update_settings(desired)
                        .map_err(|err| RepoSyncError::Write(err.to_string()))?;
                }

                // 2. Apply label updates
                for diff in outcome.label_diffs() {
                    match diff {
                        LabelDiff::Create(label) => {
                            self.repo
                                .create_label(label)
                                .map_err(|err| RepoSyncError::Write(err.to_string()))?;
                        }
                        LabelDiff::Update { desired, .. } => {
                            self.repo
                                .update_label(desired)
                                .map_err(|err| RepoSyncError::Write(err.to_string()))?;
                        }
                        LabelDiff::Delete { name, .. } => {
                            self.repo
                                .delete_label(name)
                                .map_err(|err| RepoSyncError::Write(err.to_string()))?;
                        }
                        LabelDiff::Unmanaged { .. } => {}
                    }
                }

                // 3. Apply milestone updates
                for diff in outcome.milestone_diffs() {
                    match diff {
                        MilestoneDiff::Create(milestone) => {
                            self.repo
                                .create_milestone(milestone)
                                .map_err(|err| RepoSyncError::Write(err.to_string()))?;
                        }
                        MilestoneDiff::Update { number, desired, .. } => {
                            self.repo
                                .update_milestone(*number, desired)
                                .map_err(|err| RepoSyncError::Write(err.to_string()))?;
                        }
                        MilestoneDiff::Delete { number, .. } => {
                            self.repo
                                .delete_milestone(*number)
                                .map_err(|err| RepoSyncError::Write(err.to_string()))?;
                        }
                        MilestoneDiff::Unmanaged { .. } => {}
                    }
                }

                Ok(outcome)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::SettingDiff;

    #[test]
    fn outcome_distinguishes_actionable_drift_from_unmanaged_items() {
        let unmanaged_label = LabelDiff::Unmanaged {
            name: "random-tag".into(),
        };
        let outcome = RepoSyncOutcome::new(
            Vec::new(),
            vec![unmanaged_label],
            Vec::new(),
            RepoSyncMode::Check,
        );
        assert!(outcome.has_diffs());
        assert!(!outcome.has_actionable_drift());

        let setting_diff = SettingDiff {
            name: "allow_merge_commit",
            current: "false".into(),
            desired: "true".into(),
        };
        let drift_outcome = RepoSyncOutcome::new(
            vec![setting_diff],
            Vec::new(),
            Vec::new(),
            RepoSyncMode::Check,
        );
        assert!(drift_outcome.has_diffs());
        assert!(drift_outcome.has_actionable_drift());
    }
}
