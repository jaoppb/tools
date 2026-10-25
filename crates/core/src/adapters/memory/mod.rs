//! An in-memory Project, repository issues and text for tests.

use crate::application::{AuditError, ProjectState, RepositoryIssues, RepositoryText, SyncError};
use crate::domain::{IssueNumber, IssueTree, Snapshot, Status, Texts};

/// Holds a snapshot and applies Status changes to it.
///
/// Issues named in [`MemoryProject::refusing`] refuse their first write and
/// accept later ones, so a test can see a run stop and a later run finish.
#[derive(Debug, Clone)]
pub struct MemoryProject {
    snapshot: Snapshot,
    refusing: Vec<IssueNumber>,
}

impl MemoryProject {
    /// A Project holding this snapshot.
    #[must_use]
    pub const fn new(snapshot: Snapshot) -> Self {
        Self {
            snapshot,
            refusing: Vec::new(),
        }
    }

    /// This Project, where the first write to each of `issues` fails.
    #[must_use]
    pub fn refusing(self, issues: Vec<IssueNumber>) -> Self {
        Self {
            refusing: issues,
            ..self
        }
    }

    fn refuses(&mut self, number: IssueNumber) -> bool {
        let before = self.refusing.len();
        self.refusing.retain(|refused| *refused != number);
        self.refusing.len() != before
    }
}

impl ProjectState for MemoryProject {
    fn snapshot(&self) -> Result<Snapshot, SyncError> {
        Ok(self.snapshot.clone())
    }

    fn set_status(&mut self, number: IssueNumber, status: Status) -> Result<(), SyncError> {
        self.refuses(number)
            .then_some(())
            .map_or_else(|| Ok(()), |()| Err(refusal(number)))?;
        self.snapshot = self.snapshot.clone().with_status(number, status);
        Ok(())
    }
}

fn refusal(number: IssueNumber) -> SyncError {
    SyncError::Write {
        number,
        reason: "the in-memory Project refused the write".to_owned(),
    }
}

/// Holds an issue tree, or refuses to give it.
#[derive(Debug, Clone)]
pub struct MemoryIssues {
    issues: IssueTree,
    failure: Option<String>,
}

impl MemoryIssues {
    /// A repository holding these issues.
    #[must_use]
    pub const fn new(issues: IssueTree) -> Self {
        Self {
            issues,
            failure: None,
        }
    }

    /// A repository whose issues cannot be read, for this reason.
    #[must_use]
    pub fn failing(reason: &str) -> Self {
        Self {
            issues: IssueTree::default(),
            failure: Some(reason.to_owned()),
        }
    }
}

impl RepositoryIssues for MemoryIssues {
    fn issues(&self) -> Result<IssueTree, AuditError> {
        self.failure
            .as_deref()
            .map_or_else(|| Ok(self.issues.clone()), |reason| Err(unreadable(reason)))
    }
}

fn unreadable(reason: &str) -> AuditError {
    AuditError::Issues {
        reason: reason.to_owned(),
    }
}

/// Holds the texts of the issues and pull requests, or refuses to give them.
#[derive(Debug, Clone)]
pub struct MemoryText {
    texts: Texts,
    failure: Option<String>,
}

impl MemoryText {
    /// A repository holding these texts.
    #[must_use]
    pub const fn new(texts: Texts) -> Self {
        Self {
            texts,
            failure: None,
        }
    }

    /// A repository whose texts cannot be read, for this reason.
    #[must_use]
    pub fn failing(reason: &str) -> Self {
        Self {
            texts: Texts::default(),
            failure: Some(reason.to_owned()),
        }
    }
}

impl RepositoryText for MemoryText {
    fn texts(&self) -> Result<Texts, AuditError> {
        self.failure.as_deref().map_or_else(
            || Ok(self.texts.clone()),
            |reason| Err(unreadable_text(reason)),
        )
    }
}

fn unreadable_text(reason: &str) -> AuditError {
    AuditError::Text {
        reason: reason.to_owned(),
    }
}
