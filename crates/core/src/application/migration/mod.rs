//! Issue label migration application service and port.

use std::collections::BTreeMap;
use std::fmt;

use crate::domain::audit::survey::IssueTree;
use crate::domain::issue::IssueNumber;
use crate::domain::issue::migration::IssueMigrationPlan;

/// Migration execution mode: preview changes or apply them to GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueMigrationMode {
    /// Preview label differences without mutating remote state.
    Preview,
    /// Apply label updates to GitHub issues.
    Apply,
}

/// Why issue migration failed.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// An issue's labels could not be updated.
    #[error("cannot update labels on {issue}: {reason}")]
    Update {
        /// The issue coordinate.
        issue: IssueNumber,
        /// What went wrong.
        reason: String,
    },
}

/// Port for mutating issue labels on GitHub.
pub trait IssueMigrationPort {
    /// Sets all labels of an issue, replacing any existing labels.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError::Update`] if the mutation fails.
    fn set_labels(&self, issue: &IssueNumber, labels: &[String]) -> Result<(), MigrationError>;
}

impl<T: IssueMigrationPort> IssueMigrationPort for &T {
    fn set_labels(&self, issue: &IssueNumber, labels: &[String]) -> Result<(), MigrationError> {
        (**self).set_labels(issue, labels)
    }
}

/// Outcome of running issue label migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueMigrationOutcome {
    mode: IssueMigrationMode,
    plan: IssueMigrationPlan,
}

impl IssueMigrationOutcome {
    /// Creates a new outcome.
    #[must_use]
    pub const fn new(mode: IssueMigrationMode, plan: IssueMigrationPlan) -> Self {
        Self { mode, plan }
    }

    /// The migration execution mode.
    #[must_use]
    pub const fn mode(&self) -> IssueMigrationMode {
        self.mode
    }

    /// The migration plan.
    #[must_use]
    pub const fn plan(&self) -> &IssueMigrationPlan {
        &self.plan
    }
}

impl fmt::Display for IssueMigrationOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.plan.is_empty() {
            return write!(f, "All issues already match the configured label taxonomy.");
        }

        let count = self.plan.len();
        let issues_str = if count == 1 { "issue" } else { "issues" };

        match self.mode {
            IssueMigrationMode::Preview => {
                writeln!(f, "Issue migration plan ({count} {issues_str} to update):")?;
                for action in self.plan.actions() {
                    writeln!(f, "  {action}")?;
                }
                write!(f, "Run `tools issues migrate` to execute these changes.")
            }
            IssueMigrationMode::Apply => {
                writeln!(f, "Applied label migrations to {count} {issues_str}:")?;
                for action in self.plan.actions() {
                    writeln!(f, "  {action}")?;
                }
                Ok(())
            }
        }
    }
}

/// Orchestrates issue label migration across a tree of issues.
pub struct IssueMigrator<P> {
    port: P,
    migrations: BTreeMap<String, String>,
    default_area: Option<String>,
    default_type: Option<String>,
}

impl<P: IssueMigrationPort> IssueMigrator<P> {
    /// Creates a new migrator.
    #[must_use]
    pub const fn new(
        port: P,
        migrations: BTreeMap<String, String>,
        default_area: Option<String>,
        default_type: Option<String>,
    ) -> Self {
        Self {
            port,
            migrations,
            default_area,
            default_type,
        }
    }

    /// Evaluates issues and applies migrations if running in Apply mode.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError`] if updating an issue's labels fails.
    pub fn run(
        &self,
        issues: &IssueTree,
        mode: IssueMigrationMode,
    ) -> Result<IssueMigrationOutcome, MigrationError> {
        let plan = IssueMigrationPlan::build(
            issues.iter(),
            &self.migrations,
            self.default_area.as_deref(),
            self.default_type.as_deref(),
        );

        if mode == IssueMigrationMode::Apply {
            for action in plan.actions() {
                self.port
                    .set_labels(action.issue(), action.target_labels())?;
            }
        }

        Ok(IssueMigrationOutcome::new(mode, plan))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TreeIssue;
    use crate::domain::facts::Labels;
    use crate::domain::issue::{IssueNumber, Lifecycle};
    use crate::domain::repo::Repo;
    use std::sync::Mutex;

    struct MockPort {
        calls: Mutex<Vec<(IssueNumber, Vec<String>)>>,
    }

    impl MockPort {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn recorded_calls(&self) -> Vec<(IssueNumber, Vec<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl IssueMigrationPort for MockPort {
        fn set_labels(&self, issue: &IssueNumber, labels: &[String]) -> Result<(), MigrationError> {
            self.calls
                .lock()
                .unwrap()
                .push((issue.clone(), labels.to_vec()));
            Ok(())
        }
    }

    fn make_tree() -> IssueTree {
        let num1 = IssueNumber::new(Repo::new("org", "web"), 1);
        let labels1 = Labels::from_names(["bug", "area:web"]);
        let issue1 = TreeIssue::new(num1, Lifecycle::Open).with_labels(labels1);

        let num2 = IssueNumber::new(Repo::new("org", "api"), 2);
        let labels2 = Labels::from_names(["type:feat", "area:api"]);
        let issue2 = TreeIssue::new(num2, Lifecycle::Open).with_labels(labels2);

        IssueTree::new(vec![issue1, issue2])
    }

    #[test]
    fn preview_does_not_mutate_port() {
        let port = MockPort::new();
        let mut migrations = BTreeMap::new();
        migrations.insert("bug".to_string(), "type:fix".to_string());

        let migrator = IssueMigrator::new(&port, migrations, None, None);
        let tree = make_tree();
        let outcome = migrator.run(&tree, IssueMigrationMode::Preview).unwrap();

        assert_eq!(outcome.mode(), IssueMigrationMode::Preview);
        assert_eq!(outcome.plan().len(), 1);
        assert!(port.recorded_calls().is_empty());
        assert!(outcome.to_string().contains("tools issues migrate"));
    }

    #[test]
    fn apply_updates_labels_via_port() {
        let port = MockPort::new();
        let mut migrations = BTreeMap::new();
        migrations.insert("bug".to_string(), "type:fix".to_string());

        let migrator = IssueMigrator::new(&port, migrations, None, None);
        let tree = make_tree();
        let outcome = migrator.run(&tree, IssueMigrationMode::Apply).unwrap();

        assert_eq!(outcome.mode(), IssueMigrationMode::Apply);
        let calls = port.recorded_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0.to_string(), "web#1");
        assert_eq!(calls[0].1, vec!["area:web", "type:fix"]);
        assert!(outcome.to_string().contains("Applied label migrations"));
    }

    #[test]
    fn applies_default_type_and_area_to_unlabeled_issue() {
        let port = MockPort::new();
        let migrations = BTreeMap::new();

        let migrator = IssueMigrator::new(
            &port,
            migrations,
            Some("area:infra".to_string()),
            Some("type:chore".to_string()),
        );

        let num = IssueNumber::new(Repo::new("org", "infra"), 100);
        let issue = TreeIssue::new(num, Lifecycle::Open);
        let tree = IssueTree::new(vec![issue]);

        let outcome = migrator.run(&tree, IssueMigrationMode::Apply).unwrap();
        assert_eq!(outcome.plan().len(), 1);
        let calls = port.recorded_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0.to_string(), "infra#100");
        assert_eq!(calls[0].1, vec!["area:infra", "type:chore"]);
    }
}
