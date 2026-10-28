//! The real [`RepositoryIssues`]: the repository's issues read through `gh`.

use crate::adapters::github::cursor_fields;
use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{IssuesPage, parse_issues};
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::application::{AuditError, RepositoryIssues};
use crate::domain::{IssueTree, TreeIssue};

/// The pages of issues read so far. It accepts them only when together they are
/// every issue the repository says it has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gathered {
    issues: Vec<TreeIssue>,
    total: u64,
}

impl Gathered {
    /// These pages with one more. The repository's total is the one of the
    /// latest page.
    #[must_use]
    pub fn with(self, page: &IssuesPage) -> Self {
        let mut issues = self.issues;
        issues.extend_from_slice(page.issues());
        Self {
            issues,
            total: page.total(),
        }
    }

    /// The tree of every issue gathered.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Incomplete`] when fewer issues were gathered than the
    /// repository says it has.
    pub fn into_tree(self) -> Result<IssueTree, GhError> {
        complete(self.issues, self.total)
    }
}

fn complete(issues: Vec<TreeIssue>, total: u64) -> Result<IssueTree, GhError> {
    let read = u64::try_from(issues.len()).unwrap_or(u64::MAX);
    if read < total {
        return Err(GhError::Incomplete {
            what: format!("read {read} of {total} issues"),
        });
    }
    Ok(IssueTree::new(issues))
}

/// Reads the issues of one repository through a `gh` runner.
#[derive(Debug, Clone)]
pub struct GhIssues {
    runner: GhRunner,
    target: Target,
}

impl GhIssues {
    /// The issues of this repository, reached through the installed `gh`.
    #[must_use]
    pub fn new(target: Target) -> Self {
        Self::with_runner(GhRunner::default(), target)
    }

    /// The issues of this repository, reached through this runner.
    #[must_use]
    pub const fn with_runner(runner: GhRunner, target: Target) -> Self {
        Self { runner, target }
    }

    fn read(&self) -> Result<IssueTree, GhError> {
        self.gather(None, Gathered::default())?.into_tree()
    }

    fn gather(&self, cursor: Option<&str>, gathered: Gathered) -> Result<Gathered, GhError> {
        let reply = self
            .runner
            .graphql(&query::issues(&self.target), &cursor_fields(cursor))?;
        let page = parse_issues(&reply, &self.target)?;
        let gathered = gathered.with(&page);
        match page.next() {
            Some(next) => self.gather(Some(next), gathered),
            None => Ok(gathered),
        }
    }
}

impl RepositoryIssues for GhIssues {
    fn issues(&self) -> Result<IssueTree, AuditError> {
        self.read().map_err(|error| AuditError::Issues {
            reason: error.to_string(),
        })
    }
}
