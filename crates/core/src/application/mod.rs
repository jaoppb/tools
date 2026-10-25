//! The sync use case and the port it reads and writes through.

use std::fmt;

pub mod audit;

pub use crate::application::audit::{Audit, AuditError, Report, RepositoryIssues, RepositoryText};

use crate::domain::{IssueNumber, Move, Plan, Snapshot, Status};

/// Why a sync failed.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// The Project could not be read.
    #[error("cannot read the Project: {reason}")]
    Read {
        /// What went wrong, in the words of the adapter.
        reason: String,
    },
    /// One Status could not be written.
    #[error("cannot set the status of {number}: {reason}")]
    Write {
        /// The issue whose Status was not set.
        number: IssueNumber,
        /// What went wrong, in the words of the adapter.
        reason: String,
    },
}

/// The Project as the sync sees it.
///
/// It has no method that writes Priority or Effort, so the sync cannot touch
/// them. The methods are synchronous: the tool is a blocking command-line
/// program with no executor.
pub trait ProjectState {
    /// Reads the whole current state.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Read`] when the Project cannot be read.
    fn snapshot(&self) -> Result<Snapshot, SyncError>;

    /// Sets the Status of one issue.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Write`] when the Status cannot be set.
    fn set_status(&mut self, number: IssueNumber, status: Status) -> Result<(), SyncError>;
}

/// Whether a run writes its moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Plan and report, write nothing.
    Preview,
    /// Plan, report and write each move.
    Apply,
}

/// What a run planned, and whether it wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    plan: Plan,
    mode: Mode,
}

impl Outcome {
    /// The outcome of a run in this mode.
    #[must_use]
    pub const fn new(plan: Plan, mode: Mode) -> Self {
        Self { plan, mode }
    }

    /// The plan the run produced.
    #[must_use]
    pub const fn plan(&self) -> &Plan {
        &self.plan
    }

    fn summary(&self) -> String {
        let count = self.plan.moves().len();
        match (count, self.mode) {
            (0, _) => "nothing to do".to_owned(),
            (1, Mode::Apply) => "1 move applied".to_owned(),
            (1, Mode::Preview) => "1 move (dry run, nothing written)".to_owned(),
            (_, Mode::Apply) => format!("{count} moves applied"),
            (_, Mode::Preview) => format!("{count} moves (dry run, nothing written)"),
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.plan
            .moves()
            .iter()
            .try_for_each(|planned| writeln!(formatter, "{planned}"))?;
        self.plan
            .gaps()
            .iter()
            .try_for_each(|gap| writeln!(formatter, "{gap}"))?;
        writeln!(formatter, "{}", self.summary())
    }
}

/// Plans the moves from the current state and, in [`Mode::Apply`], writes them.
#[derive(Debug)]
pub struct Sync<S> {
    state: S,
}

impl<S: ProjectState> Sync<S> {
    /// A sync over this Project.
    #[must_use]
    pub const fn new(state: S) -> Self {
        Self { state }
    }

    /// Reads the Project, plans, and writes the moves in [`Mode::Apply`],
    /// stopping at the first move that cannot be written.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError`] when the Project cannot be read or a Status cannot
    /// be set. Moves written before a failure stay written.
    pub fn run(&mut self, mode: Mode) -> Result<Outcome, SyncError> {
        let plan = self.state.snapshot()?.plan();
        match mode {
            Mode::Apply => self.apply(&plan)?,
            Mode::Preview => (),
        }
        Ok(Outcome::new(plan, mode))
    }

    fn apply(&mut self, plan: &Plan) -> Result<(), SyncError> {
        plan.moves()
            .iter()
            .try_for_each(|planned| write(&mut self.state, planned))
    }
}

fn write<S: ProjectState>(state: &mut S, planned: &Move) -> Result<(), SyncError> {
    state.set_status(planned.number(), planned.destination())
}
