//! The real [`RepositoryIssues`]: the repository's issues read through `gh`.

use crate::adapters::github::cursor_fields;
use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{IssuesPage, parse_issues_for};
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::application::{AuditError, IssueMigrationPort, MigrationError, RepositoryIssues};
use crate::domain::{IssueTree, Repo, TreeIssue};

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
        let mut all_issues = Vec::new();
        for repo in self.target.repos() {
            let gathered = self.gather_for_repo(repo, None, Gathered::default())?;
            let tree = gathered.into_tree()?;
            all_issues.extend(tree.into_issues());
        }
        Ok(IssueTree::new(all_issues))
    }

    fn gather_for_repo(
        &self,
        repo: &Repo,
        cursor: Option<&str>,
        gathered: Gathered,
    ) -> Result<Gathered, GhError> {
        let query_str = query::issues_for_repo(&self.target, repo);
        let reply = self.runner.graphql(&query_str, &cursor_fields(cursor))?;
        let page = parse_issues_for(&reply, repo)?;
        let gathered = gathered.with(&page);
        match page.next() {
            Some(next) => self.gather_for_repo(repo, Some(next), gathered),
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

impl IssueMigrationPort for GhIssues {
    fn set_labels(
        &self,
        issue: &crate::domain::IssueNumber,
        labels: &[String],
    ) -> Result<(), MigrationError> {
        let repo_owner = if issue.repo().owner().is_empty() {
            self.target.owner()
        } else {
            issue.repo().owner()
        };
        let repo_name = if issue.repo().name().is_empty() {
            self.target.primary_repo()
        } else {
            issue.repo().name()
        };
        let endpoint = format!(
            "repos/{repo_owner}/{repo_name}/issues/{}/labels",
            issue.value()
        );
        let body = serde_json::json!({ "labels": labels }).to_string();
        self.runner
            .rest_put(&endpoint, &body)
            .map_err(|error| MigrationError::Update {
                issue: issue.clone(),
                reason: error.to_string(),
            })?;
        Ok(())
    }
}
