//! The audit use case and the ports it reads the repository through: the
//! issues, the Project and the text of issues and pull requests.

use std::fmt;

use crate::application::{ProjectState, SyncError};
use crate::config::RulesConfig;
use crate::domain::{IssueTree, Members, Survey, Texts, Violations};

/// Why an audit could not run.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// The repository's issues could not be read.
    #[error("cannot read the issues of the repository: {reason}")]
    Issues {
        /// What went wrong, in the words of the adapter.
        reason: String,
    },
    /// The Project could not be read.
    #[error("cannot read the Project: {reason}")]
    Project {
        /// What went wrong, in the words of the adapter.
        reason: String,
    },
    /// The text of the issues and pull requests could not be read.
    #[error("cannot read the text of the issues and pull requests: {reason}")]
    Text {
        /// What went wrong, in the words of the adapter.
        reason: String,
    },
}

/// The repository's issues as the audit sees them.
///
/// It has no method that writes anything, so the audit cannot change an issue.
/// The method is synchronous for the reason the sync's port is.
pub trait RepositoryIssues {
    /// Reads every issue of the repository, open and closed, or fails. It never
    /// returns part of the data.
    ///
    /// # Errors
    ///
    /// Returns [`AuditError::Issues`] when the issues cannot be read in full.
    fn issues(&self) -> Result<IssueTree, AuditError>;
}

/// The text of the repository's issues and pull requests as the audit sees it.
///
/// It has no method that writes anything, like the other ports of the audit.
pub trait RepositoryText {
    /// Reads every title, body, comment, review body and review-thread comment
    /// of every issue and pull request, open and closed, or fails. It never
    /// returns part of the data.
    ///
    /// # Errors
    ///
    /// Returns [`AuditError::Text`] when the text cannot be read in full.
    fn texts(&self) -> Result<Texts, AuditError>;
}

/// What an audit found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    violations: Violations,
}

impl Report {
    /// A report of these findings.
    #[must_use]
    pub const fn new(violations: Violations) -> Self {
        Self { violations }
    }

    /// The exit code of the run: 0 when nothing was found and 1 otherwise.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        u8::from(!self.violations.is_empty())
    }
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_report(&self.violations, formatter)
    }
}

fn write_report(violations: &Violations, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    if violations.is_empty() {
        return writeln!(formatter, "no violations found");
    }
    write!(formatter, "{violations}")?;
    match violations.len() {
        1 => writeln!(formatter, "1 violation"),
        count => writeln!(formatter, "{count} violations"),
    }
}

/// Reads the repository, the Project and the text, applies the rules and
/// reports.
#[derive(Debug)]
pub struct Audit<I, P, T> {
    issues: I,
    project: Option<P>,
    texts: T,
    rules: RulesConfig,
}

impl<I: RepositoryIssues, P: ProjectState, T: RepositoryText> Audit<I, P, T> {
    /// An audit over these sources.
    #[must_use]
    pub const fn new(issues: I, project: Option<P>, texts: T, rules: RulesConfig) -> Self {
        Self {
            issues,
            project,
            texts,
            rules,
        }
    }

    /// Reads every source and reports every rule broken. It writes nothing.
    ///
    /// # Errors
    ///
    /// Returns [`AuditError`] when the issues, the Project or the text cannot
    /// be read.
    pub fn run(&self) -> Result<Report, AuditError> {
        let issues = self.issues.issues()?;
        let members = if let Some(project) = &self.project {
            let snapshot = project.snapshot().map_err(project_failure)?;
            let numbers = snapshot.cards().map(crate::domain::Card::number).collect();
            Members::new(numbers)
        } else {
            Members::default()
        };
        let survey = Survey::new(issues, members, self.texts.texts()?);
        Ok(Report::new(survey.violations(&self.rules)))
    }
}

fn project_failure(error: SyncError) -> AuditError {
    let reason = match error {
        SyncError::Read { reason } => reason,
        other @ SyncError::Write { .. } => other.to_string(),
    };
    AuditError::Project { reason }
}
