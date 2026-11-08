//! Synchronizing and auditing repository settings, labels, and milestones.

use std::fmt;

use crate::domain::repo::{
    LabelDiff, MilestoneDiff, ProjectDiff, RepoPlan, RepoSpec, RepoState, SettingDiff,
};

/// Port for querying and modifying repository configuration on GitHub.
pub trait RepoPort {
    /// Reads current repository state for requested specification.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Read`] if reading fails.
    fn get_state(&self, spec: &RepoSpec) -> Result<RepoState, RepoSyncError>;

    /// Updates repository settings.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if applying updates fails.
    fn update_settings(&self, settings: &[SettingDiff]) -> Result<(), RepoSyncError>;

    /// Applies a repository label difference (create, update, or delete).
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if applying the difference fails.
    fn apply_label(&self, diff: &LabelDiff) -> Result<(), RepoSyncError>;

    /// Applies a repository milestone difference (create, update, or delete).
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if applying the difference fails.
    fn apply_milestone(&self, diff: &MilestoneDiff) -> Result<(), RepoSyncError>;

    /// Applies a repository Project V2 difference (e.g. title update).
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError::Write`] if applying the difference fails.
    fn apply_project(&self, diff: &ProjectDiff) -> Result<(), RepoSyncError>;
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
    #[error("{0}")]
    Read(String),
    /// Failed to apply updated state.
    #[error("{0}")]
    Write(String),
    /// State drifted in check mode, or an undeclared item at severity `error` remains after sync.
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

        let unmanaged_failure = self.plan.has_unmanaged_errors();
        match self.mode {
            RepoSyncMode::Check => {
                if self.plan.has_actionable_drift() {
                    writeln!(formatter, "drift detected (check failed)")
                } else if unmanaged_failure {
                    writeln!(formatter, "undeclared items detected (check failed)")
                } else {
                    writeln!(formatter, "undeclared items reported (warnings only)")
                }
            }
            RepoSyncMode::Preview => writeln!(formatter, "dry-run: no changes applied"),
            RepoSyncMode::Apply => {
                if self.plan.has_actionable_drift() {
                    writeln!(formatter, "repository successfully updated")?;
                } else {
                    writeln!(formatter, "no changes applied")?;
                }
                if unmanaged_failure {
                    writeln!(formatter, "undeclared items remain (sync failed)")?;
                }
                Ok(())
            }
        }
    }
}

/// Orchestrates repository audit and synchronization.
#[derive(Debug)]
pub struct RepoSync<P> {
    port: P,
    desired: RepoSpec,
}

impl<P: RepoPort> RepoSync<P> {
    /// Creates a `RepoSync` handler with desired configuration specification.
    #[must_use]
    pub const fn new(port: P, desired: RepoSpec) -> Self {
        Self { port, desired }
    }

    /// Runs repository sync under `mode`.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError`] if reading or writing fails, if in [`RepoSyncMode::Check`] actionable drift is found,
    /// or if an undeclared item whose policy is `error` remains (after applying, in [`RepoSyncMode::Apply`]).
    pub fn run(&self, mode: RepoSyncMode) -> Result<RepoSyncOutcome, RepoSyncError> {
        let current_state = self.port.get_state(&self.desired)?;
        let plan = current_state
            .plan(&self.desired)
            .map_err(|err| RepoSyncError::Read(err.to_string()))?;

        let outcome = RepoSyncOutcome::new(plan, mode);
        if !outcome.plan().has_diffs() {
            return Ok(outcome);
        }

        match mode {
            RepoSyncMode::Check => {
                if outcome.plan().has_actionable_drift() || outcome.plan().has_unmanaged_errors() {
                    Err(RepoSyncError::Drift(Box::new(outcome)))
                } else {
                    Ok(outcome)
                }
            }
            RepoSyncMode::Preview => Ok(outcome),
            RepoSyncMode::Apply => {
                self.apply(outcome.plan())?;
                if outcome.plan().has_unmanaged_errors() {
                    Err(RepoSyncError::Drift(Box::new(outcome)))
                } else {
                    Ok(outcome)
                }
            }
        }
    }

    fn apply(&self, plan: &RepoPlan) -> Result<(), RepoSyncError> {
        if !plan.setting_diffs().is_empty() {
            self.port.update_settings(plan.setting_diffs())?;
        }
        for diff in plan.label_diffs().iter().filter(|d| d.is_actionable()) {
            self.port.apply_label(diff)?;
        }
        for diff in plan.milestone_diffs().iter().filter(|d| d.is_actionable()) {
            self.port.apply_milestone(diff)?;
        }
        for diff in plan.project_diffs().iter().filter(|d| d.is_actionable()) {
            self.port.apply_project(diff)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::{
        RepoLabel, RepoLabelsSpec, RepoProject, RepoProjectSpec, RepoSettings, SettingDiff,
        Severity, Unmanaged,
    };
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockRepoPort {
        state: RepoState,
        applied_settings: Mutex<Vec<Vec<SettingDiff>>>,
        applied_labels: Mutex<Vec<LabelDiff>>,
        applied_milestones: Mutex<Vec<MilestoneDiff>>,
        applied_projects: Mutex<Vec<ProjectDiff>>,
    }

    impl MockRepoPort {
        fn new(state: RepoState) -> Self {
            Self {
                state,
                applied_settings: Mutex::new(Vec::new()),
                applied_labels: Mutex::new(Vec::new()),
                applied_milestones: Mutex::new(Vec::new()),
                applied_projects: Mutex::new(Vec::new()),
            }
        }
    }

    impl RepoPort for MockRepoPort {
        fn get_state(&self, _spec: &RepoSpec) -> Result<RepoState, RepoSyncError> {
            Ok(self.state.clone())
        }

        fn update_settings(&self, settings: &[SettingDiff]) -> Result<(), RepoSyncError> {
            self.applied_settings
                .lock()
                .unwrap()
                .push(settings.to_vec());
            Ok(())
        }

        fn apply_label(&self, diff: &LabelDiff) -> Result<(), RepoSyncError> {
            self.applied_labels.lock().unwrap().push(diff.clone());
            Ok(())
        }

        fn apply_milestone(&self, diff: &MilestoneDiff) -> Result<(), RepoSyncError> {
            self.applied_milestones.lock().unwrap().push(diff.clone());
            Ok(())
        }

        fn apply_project(&self, diff: &ProjectDiff) -> Result<(), RepoSyncError> {
            self.applied_projects.lock().unwrap().push(diff.clone());
            Ok(())
        }
    }

    #[test]
    fn outcome_distinguishes_actionable_drift_from_unmanaged_items() {
        let unmanaged_label = LabelDiff::Unmanaged {
            name: "random-tag".into(),
            severity: Severity::Warn,
        };
        let plan = RepoPlan::new(Vec::new(), vec![unmanaged_label], Vec::new(), Vec::new());
        let outcome = RepoSyncOutcome::new(plan.clone(), RepoSyncMode::Check);
        assert!(outcome.plan().has_diffs());
        assert!(!outcome.plan().has_actionable_drift());
        assert!(
            outcome
                .to_string()
                .contains("undeclared items reported (warnings only)")
        );

        let apply_unmanaged = RepoSyncOutcome::new(plan, RepoSyncMode::Apply);
        assert!(apply_unmanaged.to_string().contains("no changes applied"));

        let setting_diff = SettingDiff::AllowMergeCommit {
            current: false,
            desired: true,
        };
        let drift_plan = RepoPlan::new(vec![setting_diff], Vec::new(), Vec::new(), Vec::new());
        let drift_outcome = RepoSyncOutcome::new(drift_plan.clone(), RepoSyncMode::Check);
        assert!(drift_outcome.plan().has_diffs());
        assert!(drift_outcome.plan().has_actionable_drift());
        assert!(
            drift_outcome
                .to_string()
                .contains("drift detected (check failed)")
        );

        let apply_drift = RepoSyncOutcome::new(drift_plan, RepoSyncMode::Apply);
        assert!(
            apply_drift
                .to_string()
                .contains("repository successfully updated")
        );
    }

    #[test]
    fn sync_runs_check_mode_and_reports_drift() {
        let port = MockRepoPort::new(RepoState::new(
            Some(RepoSettings {
                allow_merge_commit: false,
                allow_squash_merge: false,
                allow_rebase_merge: false,
                delete_branch_on_merge: true,
                merge_commit_title: Default::default(),
                merge_commit_message: Default::default(),
            }),
            Some(Vec::new()),
            Some(Vec::new()),
            None,
        ));
        let desired = RepoSettings {
            allow_merge_commit: true,
            allow_squash_merge: false,
            allow_rebase_merge: false,
            delete_branch_on_merge: true,
            merge_commit_title: Default::default(),
            merge_commit_message: Default::default(),
        };
        let sync = RepoSync::new(port, RepoSpec::new(Some(desired), None, None, None));

        let err = sync.run(RepoSyncMode::Check).unwrap_err();
        assert!(matches!(err, RepoSyncError::Drift(_)));
    }

    #[test]
    fn sync_applies_changes_to_port() {
        let current_project = RepoProject::new("PVT_1", 4, "Old Roadmap");
        let port = MockRepoPort::new(RepoState::new(
            None,
            Some(Vec::new()),
            None,
            Some(current_project),
        ));
        let desired_labels = RepoLabelsSpec {
            deprecated: vec!["old-label".into()],
            renames: Default::default(),
            unmanaged: Unmanaged::Warn,
            migrations: Default::default(),
            items: vec![RepoLabel::new("type:feat", "1d76db", "Feature")],
        };
        let desired_project = RepoProjectSpec::new(4, "Roadmap");
        let sync = RepoSync::new(
            port,
            RepoSpec::new(None, Some(desired_labels), None, Some(desired_project)),
        );

        let outcome = sync.run(RepoSyncMode::Apply).unwrap();
        assert!(outcome.plan().has_diffs());
        assert_eq!(outcome.mode(), RepoSyncMode::Apply);
        assert_eq!(sync.port.applied_labels.lock().unwrap().len(), 1);
        assert_eq!(sync.port.applied_projects.lock().unwrap().len(), 1);
    }

    fn milestone_state(titles: &[&str]) -> RepoState {
        let milestones = titles
            .iter()
            .zip(1u64..)
            .map(|(title, number)| crate::domain::repo::RepoMilestone {
                number,
                title: (*title).into(),
                description: String::new(),
                state: crate::domain::repo::MilestoneState::Open,
            })
            .collect();
        RepoState::new(None, None, Some(milestones), None)
    }

    fn milestone_spec(policy: Unmanaged) -> RepoSpec {
        let milestones = crate::domain::repo::RepoMilestonesSpec {
            unmanaged: policy,
            items: vec![crate::domain::repo::RepoMilestoneSpec {
                title: "v0.3.0".into(),
                description: String::new(),
                state: crate::domain::repo::MilestoneState::Open,
            }],
            ..Default::default()
        };
        RepoSpec::new(None, None, Some(milestones), None)
    }

    #[test]
    fn check_fails_on_an_undeclared_milestone_and_names_it() {
        let sync = RepoSync::new(
            MockRepoPort::new(milestone_state(&["v0.3.0", "v0.4.0"])),
            milestone_spec(Unmanaged::Error),
        );
        let Err(RepoSyncError::Drift(outcome)) = sync.run(RepoSyncMode::Check) else {
            panic!("expected the check to fail");
        };
        let text = outcome.to_string();
        assert!(text.contains("'v0.4.0'"), "{text}");
        assert!(
            text.contains("undeclared items detected (check failed)"),
            "{text}"
        );
    }

    #[test]
    fn check_passes_when_every_milestone_is_declared() {
        let sync = RepoSync::new(
            MockRepoPort::new(milestone_state(&["v0.3.0"])),
            milestone_spec(Unmanaged::Error),
        );
        assert!(sync.run(RepoSyncMode::Check).is_ok());
    }

    #[test]
    fn check_only_warns_under_warn_and_says_nothing_under_ignore() {
        let warn = RepoSync::new(
            MockRepoPort::new(milestone_state(&["v0.3.0", "v0.4.0"])),
            milestone_spec(Unmanaged::Warn),
        );
        let outcome = warn.run(RepoSyncMode::Check).unwrap();
        assert!(outcome.to_string().contains("'v0.4.0'"));
        let ignore = RepoSync::new(
            MockRepoPort::new(milestone_state(&["v0.3.0", "v0.4.0"])),
            milestone_spec(Unmanaged::Ignore),
        );
        let outcome = ignore.run(RepoSyncMode::Check).unwrap();
        assert!(!outcome.plan().has_diffs());
    }

    #[test]
    fn sync_prunes_under_prune_and_leaves_no_failure() {
        let sync = RepoSync::new(
            MockRepoPort::new(milestone_state(&["v0.3.0", "v0.4.0"])),
            milestone_spec(Unmanaged::Prune),
        );
        sync.run(RepoSyncMode::Apply).unwrap();
        assert_eq!(sync.port.applied_milestones.lock().unwrap().len(), 1);
    }

    #[test]
    fn sync_applies_the_rest_then_fails_while_an_error_item_remains() {
        let mut spec = milestone_spec(Unmanaged::Error);
        spec.labels = Some(RepoLabelsSpec {
            items: vec![RepoLabel::new("type:feat", "1d76db", "Feature")],
            ..Default::default()
        });
        let state = RepoState::new(
            None,
            Some(Vec::new()),
            milestone_state(&["v0.3.0", "v0.4.0"]).milestones,
            None,
        );
        let sync = RepoSync::new(MockRepoPort::new(state), spec);
        let Err(RepoSyncError::Drift(outcome)) = sync.run(RepoSyncMode::Apply) else {
            panic!("expected sync to fail");
        };
        assert_eq!(sync.port.applied_labels.lock().unwrap().len(), 1);
        let text = outcome.to_string();
        assert!(text.contains("'v0.4.0'"), "{text}");
        assert!(
            text.contains("undeclared items remain (sync failed)"),
            "{text}"
        );
    }
}
