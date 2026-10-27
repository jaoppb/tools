//! Synchronizing and auditing repository settings, labels, and milestones.

use std::fmt;

use crate::config::{LabelsConfig, MilestonesConfig};
use crate::domain::repo::{
    LabelDiff, MilestoneDiff, RepoEntities, RepoLabel, RepoMilestoneSpec, RepoPlan, RepoSettings,
    RepoState,
};

/// Port for querying and modifying repository configuration on GitHub.
pub trait RepoPort {
    /// Reads current repository state for selected entities.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Read`] if reading fails.
    fn get_state(&self, entities: RepoEntities) -> Result<RepoState, RepoSyncError>;

    /// Updates repository settings.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if applying updates fails.
    fn update_settings(&self, settings: &RepoSettings) -> Result<(), RepoSyncError>;

    /// Creates a repository label.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if creation fails.
    fn create_label(&self, label: &RepoLabel) -> Result<(), RepoSyncError>;

    /// Updates a repository label.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if update fails.
    fn update_label(&self, current_name: &str, label: &RepoLabel) -> Result<(), RepoSyncError>;

    /// Deletes a repository label by name.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if deletion fails.
    fn delete_label(&self, name: &str) -> Result<(), RepoSyncError>;

    /// Creates a repository milestone.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if creation fails.
    fn create_milestone(&self, milestone: &RepoMilestoneSpec) -> Result<(), RepoSyncError>;

    /// Updates an existing repository milestone.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if update fails.
    fn update_milestone(
        &self,
        number: u64,
        milestone: &RepoMilestoneSpec,
    ) -> Result<(), RepoSyncError>;

    /// Deletes a repository milestone by number.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if deletion fails.
    fn delete_milestone(&self, number: u64) -> Result<(), RepoSyncError>;
}

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
    #[error("{0}")]
    Drift(Box<RepoSyncOutcome>),
}

/// Outcome of a repository sync run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSyncOutcome {
    plan: RepoPlan,
    mode: RepoSyncMode,
}

impl RepoSyncOutcome {
    /// New outcome with diffs under `mode`.
    #[must_use]
    pub const fn new(plan: RepoPlan, mode: RepoSyncMode) -> Self {
        Self { plan, mode }
    }

    /// The reconciliation plan produced.
    #[must_use]
    pub const fn plan(&self) -> &RepoPlan {
        &self.plan
    }

    /// Execution mode of the run.
    #[must_use]
    pub const fn mode(&self) -> RepoSyncMode {
        self.mode
    }
}

impl fmt::Display for RepoSyncOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        if !self.plan.has_diffs() {
            return writeln!(formatter, "repository configuration is in sync");
        }

        write!(formatter, "{}", self.plan)?;

        match self.mode {
            RepoSyncMode::Check => {
                if self.plan.has_actionable_drift() {
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
pub struct RepoSync<P> {
    port: P,
    desired_settings: Option<RepoSettings>,
    desired_labels: Option<LabelsConfig>,
    desired_milestones: Option<MilestonesConfig>,
}

impl<P: RepoPort> RepoSync<P> {
    /// Creates a `RepoSync` handler with all desired configurations.
    #[must_use]
    pub const fn new(
        port: P,
        desired_settings: Option<RepoSettings>,
        desired_labels: Option<LabelsConfig>,
        desired_milestones: Option<MilestonesConfig>,
    ) -> Self {
        Self {
            port,
            desired_settings,
            desired_labels,
            desired_milestones,
        }
    }

    /// Runs repository sync under `mode`.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError`] if reading or writing fails, or if in [`RepoSyncMode::Check`] and actionable drift is found.
    pub fn run(&self, mode: RepoSyncMode) -> Result<RepoSyncOutcome, RepoSyncError> {
        let entities = RepoEntities::from_presence(
            self.desired_settings.is_some(),
            self.desired_labels.is_some(),
            self.desired_milestones.is_some(),
        );

        let current_state = self.port.get_state(entities)?;
        let plan = current_state.plan(
            self.desired_settings.as_ref(),
            self.desired_labels.as_ref(),
            self.desired_milestones.as_ref(),
        );

        let outcome = RepoSyncOutcome::new(plan, mode);
        if !outcome.plan().has_diffs() {
            return Ok(outcome);
        }

        match mode {
            RepoSyncMode::Check => {
                if outcome.plan().has_actionable_drift() {
                    Err(RepoSyncError::Drift(Box::new(outcome)))
                } else {
                    Ok(outcome)
                }
            }
            RepoSyncMode::Preview => Ok(outcome),
            RepoSyncMode::Apply => {
                self.apply(outcome.plan())?;
                Ok(outcome)
            }
        }
    }

    fn apply(&self, plan: &RepoPlan) -> Result<(), RepoSyncError> {
        // 1. Apply settings updates
        if !plan.setting_diffs().is_empty()
            && let Some(ref desired) = self.desired_settings
        {
            self.port.update_settings(desired)?;
        }

        // 2. Apply label updates
        for diff in plan.label_diffs() {
            match diff {
                LabelDiff::Create(label) => {
                    self.port.create_label(label)?;
                }
                LabelDiff::Update { current, desired } => {
                    self.port.update_label(&current.name, desired)?;
                }
                LabelDiff::Delete { name, .. } => {
                    self.port.delete_label(name)?;
                }
                LabelDiff::Unmanaged { .. } => {}
            }
        }

        // 3. Apply milestone updates
        for diff in plan.milestone_diffs() {
            match diff {
                MilestoneDiff::Create(milestone) => {
                    self.port.create_milestone(milestone)?;
                }
                MilestoneDiff::Update { current, desired } => {
                    self.port.update_milestone(current.number, desired)?;
                }
                MilestoneDiff::Delete { number, .. } => {
                    self.port.delete_milestone(*number)?;
                }
                MilestoneDiff::Unmanaged { .. } => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::SettingDiff;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockRepoPort {
        state: RepoState,
        applied_settings: Mutex<Vec<RepoSettings>>,
        created_labels: Mutex<Vec<RepoLabel>>,
        deleted_labels: Mutex<Vec<String>>,
    }

    impl MockRepoPort {
        fn new(state: RepoState) -> Self {
            Self {
                state,
                applied_settings: Mutex::new(Vec::new()),
                created_labels: Mutex::new(Vec::new()),
                deleted_labels: Mutex::new(Vec::new()),
            }
        }
    }

    impl RepoPort for MockRepoPort {
        fn get_state(&self, _entities: RepoEntities) -> Result<RepoState, RepoSyncError> {
            Ok(self.state.clone())
        }

        fn update_settings(&self, settings: &RepoSettings) -> Result<(), RepoSyncError> {
            self.applied_settings.lock().unwrap().push(settings.clone());
            Ok(())
        }

        fn create_label(&self, label: &RepoLabel) -> Result<(), RepoSyncError> {
            self.created_labels.lock().unwrap().push(label.clone());
            Ok(())
        }

        fn update_label(
            &self,
            _current_name: &str,
            _label: &RepoLabel,
        ) -> Result<(), RepoSyncError> {
            Ok(())
        }

        fn delete_label(&self, name: &str) -> Result<(), RepoSyncError> {
            self.deleted_labels.lock().unwrap().push(name.to_owned());
            Ok(())
        }

        fn create_milestone(&self, _milestone: &RepoMilestoneSpec) -> Result<(), RepoSyncError> {
            Ok(())
        }

        fn update_milestone(
            &self,
            _number: u64,
            _milestone: &RepoMilestoneSpec,
        ) -> Result<(), RepoSyncError> {
            Ok(())
        }

        fn delete_milestone(&self, _number: u64) -> Result<(), RepoSyncError> {
            Ok(())
        }
    }

    #[test]
    fn outcome_distinguishes_actionable_drift_from_unmanaged_items() {
        let unmanaged_label = LabelDiff::Unmanaged {
            name: "random-tag".into(),
        };
        let plan = RepoPlan::new(Vec::new(), vec![unmanaged_label], Vec::new());
        let outcome = RepoSyncOutcome::new(plan, RepoSyncMode::Check);
        assert!(outcome.plan().has_diffs());
        assert!(!outcome.plan().has_actionable_drift());

        let setting_diff = SettingDiff {
            name: "allow_merge_commit",
            current: "false".into(),
            desired: "true".into(),
        };
        let drift_plan = RepoPlan::new(vec![setting_diff], Vec::new(), Vec::new());
        let drift_outcome = RepoSyncOutcome::new(drift_plan, RepoSyncMode::Check);
        assert!(drift_outcome.plan().has_diffs());
        assert!(drift_outcome.plan().has_actionable_drift());
    }

    #[test]
    fn sync_runs_check_mode_and_reports_drift() {
        let port = MockRepoPort::new(RepoState::new(
            Some(RepoSettings {
                allow_merge_commit: false,
                allow_squash_merge: false,
                allow_rebase_merge: false,
                delete_branch_on_merge: true,
                merge_commit_title: "PR_TITLE".into(),
                merge_commit_message: "PR_BODY".into(),
            }),
            Vec::new(),
            Vec::new(),
        ));
        let desired = RepoSettings {
            allow_merge_commit: true,
            allow_squash_merge: false,
            allow_rebase_merge: false,
            delete_branch_on_merge: true,
            merge_commit_title: "PR_TITLE".into(),
            merge_commit_message: "PR_BODY".into(),
        };
        let sync = RepoSync::new(port, Some(desired), None, None);

        let err = sync.run(RepoSyncMode::Check).unwrap_err();
        assert!(matches!(err, RepoSyncError::Drift(_)));
    }

    #[test]
    fn sync_applies_changes_to_port() {
        let port = MockRepoPort::new(RepoState::new(None, Vec::new(), Vec::new()));
        let desired_labels = LabelsConfig {
            deprecated: vec!["old-label".into()],
            prune_unmanaged: false,
            items: vec![RepoLabel::new("type:feat", "1d76db", "Feature")],
        };
        let sync = RepoSync::new(
            port,
            None,
            Some(desired_labels),
            None,
        );

        let outcome = sync.run(RepoSyncMode::Apply).unwrap();
        assert!(outcome.plan().has_diffs());
        assert_eq!(outcome.mode(), RepoSyncMode::Apply);
        assert_eq!(sync.port.created_labels.lock().unwrap().len(), 1);
    }
}
