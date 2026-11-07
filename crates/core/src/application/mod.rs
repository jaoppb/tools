//! The sync use case and the port it reads and writes through.

use std::fmt;

pub mod audit;
pub mod migration;
pub mod repo;

pub use crate::application::audit::{Audit, AuditError, Report, RepositoryIssues};
pub use crate::application::migration::{
    IssueMigrationMode, IssueMigrationOutcome, IssueMigrationPort, IssueMigrator, MigrationError,
};
pub use crate::application::repo::{
    RepoPort, RepoSync, RepoSyncError, RepoSyncMode, RepoSyncOutcome,
};

use crate::domain::{IssueNumber, Snapshot, StatusName};
use crate::workflow::{Decision, Engine, Evaluation, Outcome as Decided, ParentResolver, Workflow};

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
    fn set_status(&mut self, number: IssueNumber, status: &StatusName) -> Result<(), SyncError>;
}

/// Whether a run writes its moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Evaluate and report, write nothing.
    Preview,
    /// Evaluate, report and write each move.
    Apply,
}

/// What a run decided for every card, and whether it wrote the moves.
#[derive(Debug, Clone)]
pub struct Outcome {
    evaluation: Evaluation,
    mode: Mode,
}

impl Outcome {
    /// The outcome of a run in this mode.
    #[must_use]
    pub const fn new(evaluation: Evaluation, mode: Mode) -> Self {
        Self { evaluation, mode }
    }

    /// The decision of every card, with its trace.
    #[must_use]
    pub const fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }

    /// The cards that change Status.
    #[must_use]
    pub fn moved(&self) -> usize {
        self.count(|decided| matches!(decided, Decided::Move { .. }))
    }

    /// The cards that stay.
    #[must_use]
    pub fn kept(&self) -> usize {
        self.count(|decided| matches!(decided, Decided::Keep { .. }))
    }

    /// The cards that could not be evaluated.
    #[must_use]
    pub fn frozen(&self) -> usize {
        self.count(|decided| matches!(decided, Decided::Frozen { .. }))
    }

    /// Whether anything needs attention: a card to move or a card frozen.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.moved() + self.frozen() > 0
    }

    fn count(&self, wanted: impl Fn(&Decided) -> bool) -> usize {
        self.evaluation
            .iter()
            .filter(|decision| wanted(&decision.outcome()))
            .count()
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for decision in self.evaluation.iter() {
            match decision.outcome() {
                Decided::Move { from, to, rule } => {
                    writeln!(formatter, "{} {from} -> {to} ({rule})", decision.number())?;
                }
                Decided::Frozen { cause } => {
                    writeln!(formatter, "{} frozen: {cause}", decision.number())?;
                }
                Decided::Keep { .. } => (),
            }
        }
        let suffix = match self.mode {
            Mode::Apply => "",
            Mode::Preview => " (check, nothing written)",
        };
        writeln!(
            formatter,
            "{} kept, {} moved, {} frozen{suffix}",
            self.kept(),
            self.moved(),
            self.frozen()
        )
    }
}

/// Evaluates the workflow over the Project and, in [`Mode::Apply`], writes the moves.
#[derive(Debug)]
pub struct Sync<S> {
    state: S,
    workflow: Workflow,
}

/// The Project is also the [`ParentResolver`] of its cards: a parent outside it
/// is read through the same adapter.
impl<S: ProjectState + ParentResolver> Sync<S> {
    /// A sync of this Project under this workflow.
    #[must_use]
    pub const fn new(state: S, workflow: Workflow) -> Self {
        Self { state, workflow }
    }

    /// Reads the Project, evaluates every card, and writes the moves in
    /// [`Mode::Apply`], stopping at the first move that cannot be written.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError`] when the Project cannot be read or a Status cannot
    /// be set. Moves written before a failure stay written.
    pub fn run(&mut self, mode: Mode) -> Result<Outcome, SyncError> {
        let snapshot = self.state.snapshot()?;
        let evaluation = Engine::new(&self.workflow, &self.state).evaluate(&snapshot);
        if mode == Mode::Apply {
            self.apply(&evaluation)?;
        }
        Ok(Outcome::new(evaluation, mode))
    }

    fn apply(&mut self, evaluation: &Evaluation) -> Result<(), SyncError> {
        evaluation
            .iter()
            .try_for_each(|decision| write(&mut self.state, decision))
    }
}

fn write<S: ProjectState>(state: &mut S, decision: &Decision) -> Result<(), SyncError> {
    match decision.outcome() {
        Decided::Move { to, .. } => state.set_status(decision.number().clone(), &to),
        Decided::Keep { .. } | Decided::Frozen { .. } => Ok(()),
    }
}
