//! Synchronizing and auditing repository settings.

use std::fmt;

use crate::adapters::github::GhRepo;
use crate::domain::repo::{RepoSettings, SettingDiff};

/// Execution mode for repository synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoSyncMode {
    /// Audit only: report differences and return an error if settings drift.
    Check,
    /// Dry run: report differences without applying them, succeeding.
    Preview,
    /// Apply: update GitHub settings to match desired configuration.
    Apply,
}

/// Errors occurring during repository synchronization.
#[derive(Debug, thiserror::Error)]
pub enum RepoSyncError {
    /// Failed to read current settings.
    #[error("failed to read repository settings: {0}")]
    Read(String),
    /// Failed to apply updated settings.
    #[error("failed to update repository settings: {0}")]
    Write(String),
    /// Settings drifted in check mode.
    #[error("repository settings drift detected:\n{0}")]
    Drift(String),
}

/// Outcome of a repository sync run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSyncOutcome {
    diffs: Vec<SettingDiff>,
    mode: RepoSyncMode,
}

impl RepoSyncOutcome {
    /// New outcome with `diffs` under `mode`.
    #[must_use]
    pub const fn new(diffs: Vec<SettingDiff>, mode: RepoSyncMode) -> Self {
        Self { diffs, mode }
    }

    /// Whether any differences were detected.
    #[must_use]
    pub fn has_diffs(&self) -> bool {
        !self.diffs.is_empty()
    }

    /// The differences detected.
    #[must_use]
    pub fn diffs(&self) -> &[SettingDiff] {
        &self.diffs
    }
}

impl fmt::Display for RepoSyncOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.diffs.is_empty() {
            return writeln!(formatter, "repository settings are in sync");
        }
        for diff in &self.diffs {
            writeln!(formatter, "  • {diff}")?;
        }
        match self.mode {
            RepoSyncMode::Check => writeln!(formatter, "drift detected (check failed)"),
            RepoSyncMode::Preview => writeln!(formatter, "dry-run: no changes applied"),
            RepoSyncMode::Apply => writeln!(formatter, "settings successfully updated"),
        }
    }
}

/// Orchestrates repository settings audit and synchronization.
#[derive(Debug)]
pub struct RepoSync {
    repo: GhRepo,
    desired: RepoSettings,
}

impl RepoSync {
    /// Creates a new `RepoSync` handler.
    #[must_use]
    pub const fn new(repo: GhRepo, desired: RepoSettings) -> Self {
        Self { repo, desired }
    }

    /// Runs repository sync under `mode`.
    ///
    /// # Errors
    ///
    /// Returns [`RepoSyncError`] if reading or writing fails, or if in [`RepoSyncMode::Check`] and drift is found.
    pub fn run(&self, mode: RepoSyncMode) -> Result<RepoSyncOutcome, RepoSyncError> {
        let current = self
            .repo
            .get_settings()
            .map_err(|err| RepoSyncError::Read(err.to_string()))?;
        let diffs = current.diff(&self.desired);
        let outcome = RepoSyncOutcome::new(diffs, mode);

        if !outcome.has_diffs() {
            return Ok(outcome);
        }

        match mode {
            RepoSyncMode::Check => {
                let report = outcome.to_string();
                Err(RepoSyncError::Drift(report))
            }
            RepoSyncMode::Preview => Ok(outcome),
            RepoSyncMode::Apply => {
                self.repo
                    .update_settings(&self.desired)
                    .map_err(|err| RepoSyncError::Write(err.to_string()))?;
                Ok(outcome)
            }
        }
    }
}
