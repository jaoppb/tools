//! `tools issues check|migrate`: the issue workflow audit and the label migration.
//!
//! `check` exits with 0 when the issues break no rule and no label migration is pending, with 1
//! otherwise, and with 2 when the check could not run. `migrate` rewrites the labels.

pub mod cli;

use tools_core::adapters::github::{GhIssues, GhProject};
use tools_core::application::{
    Audit, AuditError, IssueMigrationMode, IssueMigrationOutcome, IssueMigrator, RepositoryIssues,
};
use tools_core::domain::audit::survey::IssueTree;

use crate::Exit;
use cli::{Arguments, ResolvedCheck};

/// Runs `tools issues check|migrate`.
///
/// # Errors
///
/// Returns an [`Exit`] with code 2 when the run cannot complete.
pub fn run(arguments: &Arguments) -> Result<Exit, Exit> {
    let resolved = arguments.resolve().map_err(Exit::failed)?;
    if arguments.is_migrate() {
        let gh_issues = GhIssues::new(resolved.target.clone());
        let tree = gh_issues.issues().map_err(Exit::failed)?;
        let outcome = migrate(
            &gh_issues,
            &resolved,
            &tree,
            IssueMigrationMode::Apply,
            arguments.default_area(),
            arguments.default_type(),
        )?;
        return Ok(Exit::new(outcome.to_string(), 0));
    }
    check(resolved)
}

/// Issues read once, so the audit and the migration preview see the same snapshot.
struct Fetched(IssueTree);

impl RepositoryIssues for Fetched {
    fn issues(&self) -> Result<IssueTree, AuditError> {
        Ok(self.0.clone())
    }
}

/// The audit, followed by the label migrations `migrate` would write.
fn check(resolved: ResolvedCheck) -> Result<Exit, Exit> {
    let gh_issues = GhIssues::new(resolved.target.clone());
    let tree = gh_issues.issues().map_err(Exit::failed)?;
    let pending = migrate(
        &gh_issues,
        &resolved,
        &tree,
        IssueMigrationMode::Preview,
        None,
        None,
    )?;
    let project = resolved
        .target
        .project()
        .map(|_| GhProject::new(resolved.target.clone()));
    let report = Audit::new(Fetched(tree), project, resolved.rules)
        .run()
        .map_err(Exit::failed)?;
    let mut text = report.to_string();
    let mut code = report.exit_code();
    if !pending.plan().is_empty() {
        text = format!("{}\n{pending}", text.trim_end());
        code = code.max(1);
    }
    Ok(Exit::new(text, code))
}

fn migrate(
    gh_issues: &GhIssues,
    resolved: &ResolvedCheck,
    tree: &IssueTree,
    mode: IssueMigrationMode,
    default_area: Option<&str>,
    default_type: Option<&str>,
) -> Result<IssueMigrationOutcome, Exit> {
    IssueMigrator::new(
        gh_issues,
        resolved.migrations.clone(),
        default_area.map(str::to_owned),
        default_type.map(str::to_owned),
    )
    .run(tree, mode)
    .map_err(Exit::failed)
}
