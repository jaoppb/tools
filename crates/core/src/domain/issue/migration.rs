//! Issue label migration planning according to taxonomy rules.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::domain::audit::survey::issue::TreeIssue;
use crate::domain::issue::IssueNumber;

/// A proposed label migration action for a single issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueMigrationAction {
    issue: IssueNumber,
    removed: Vec<String>,
    added: Vec<String>,
    target_labels: Vec<String>,
}

impl IssueMigrationAction {
    /// The issue coordinate.
    #[must_use]
    pub const fn issue(&self) -> &IssueNumber {
        &self.issue
    }

    /// Labels that will be removed from the issue.
    #[must_use]
    pub fn removed(&self) -> &[String] {
        &self.removed
    }

    /// Labels that will be added to the issue.
    #[must_use]
    pub fn added(&self) -> &[String] {
        &self.added
    }

    /// The resulting target labels for the issue.
    #[must_use]
    pub fn target_labels(&self) -> &[String] {
        &self.target_labels
    }

    /// Whether this action changes any labels on the issue.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.removed.is_empty() || !self.added.is_empty()
    }
}

impl fmt::Display for IssueMigrationAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.issue)?;
        for r in &self.removed {
            write!(f, " -{r}")?;
        }
        for a in &self.added {
            write!(f, " +{a}")?;
        }
        Ok(())
    }
}

/// A collection of migration actions across issues in a repository or organization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssueMigrationPlan {
    actions: Vec<IssueMigrationAction>,
}

impl IssueMigrationPlan {
    /// Creates a new migration plan with the given actions.
    #[must_use]
    pub const fn new(actions: Vec<IssueMigrationAction>) -> Self {
        Self { actions }
    }

    /// Evaluates one issue against migration mappings, default area, and default type, returning an action if labels change.
    #[must_use]
    pub fn plan_issue(
        issue: &TreeIssue,
        migrations: &BTreeMap<String, String>,
        default_area: Option<&str>,
        default_type: Option<&str>,
    ) -> Option<IssueMigrationAction> {
        let current_set: BTreeSet<String> = issue.labels().names().map(String::from).collect();
        let mut target_set: BTreeSet<String> = current_set
            .iter()
            .map(|name| migrations.get(name).unwrap_or(name).clone())
            .collect();

        ensure_prefixed_label(&mut target_set, "area:", default_area);
        ensure_prefixed_label(&mut target_set, "type:", default_type);

        let added: Vec<String> = target_set.difference(&current_set).cloned().collect();
        let removed: Vec<String> = current_set.difference(&target_set).cloned().collect();

        if added.is_empty() && removed.is_empty() {
            None
        } else {
            Some(IssueMigrationAction {
                issue: issue.number().clone(),
                removed,
                added,
                target_labels: target_set.into_iter().collect(),
            })
        }
    }

    /// Builds a migration plan for an entire collection of issues.
    #[must_use]
    pub fn build<'a>(
        issues: impl IntoIterator<Item = &'a TreeIssue>,
        migrations: &BTreeMap<String, String>,
        default_area: Option<&str>,
        default_type: Option<&str>,
    ) -> Self {
        let actions = issues
            .into_iter()
            .filter_map(|issue| Self::plan_issue(issue, migrations, default_area, default_type))
            .collect();
        Self { actions }
    }

    /// Migration actions in the plan.
    #[must_use]
    pub fn actions(&self) -> &[IssueMigrationAction] {
        &self.actions
    }

    /// Whether there are no actions to apply.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Number of issues that require migration.
    #[must_use]
    pub fn len(&self) -> usize {
        self.actions.len()
    }
}

fn ensure_prefixed_label(target_set: &mut BTreeSet<String>, prefix: &str, fallback: Option<&str>) {
    if let Some(trimmed) = fallback.map(str::trim).filter(|s| !s.is_empty())
        && !target_set.iter().any(|label| label.starts_with(prefix))
    {
        let canonical = if trimmed.starts_with(prefix) {
            trimmed.to_string()
        } else {
            format!("{prefix}{trimmed}")
        };
        target_set.insert(canonical);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::facts::Labels;
    use crate::domain::issue::{IssueNumber, Lifecycle};
    use crate::domain::repo::Repo;

    fn make_issue(repo: &str, number: u64, labels: &[&str]) -> TreeIssue {
        let num = IssueNumber::new(Repo::new("org", repo), number);
        let labels = Labels::from_names(labels.iter().copied());
        TreeIssue::new(num, Lifecycle::Open).with_labels(labels)
    }

    #[test]
    fn plans_replacement_of_legacy_labels() {
        let mut migrations = BTreeMap::new();
        migrations.insert("bug".to_string(), "type:fix".to_string());
        migrations.insert("enhancement".to_string(), "type:feat".to_string());

        let issue = make_issue("web", 1, &["bug", "area:web"]);
        let action = IssueMigrationPlan::plan_issue(&issue, &migrations, None, None).unwrap();

        assert_eq!(action.issue().to_string(), "web#1");
        assert_eq!(action.removed(), &["bug"]);
        assert_eq!(action.added(), &["type:fix"]);
        assert_eq!(action.target_labels(), &["area:web", "type:fix"]);
        assert_eq!(action.to_string(), "web#1: -bug +type:fix");
    }

    #[test]
    fn adds_default_area_when_issue_lacks_area() {
        let mut migrations = BTreeMap::new();
        migrations.insert("bug".to_string(), "type:fix".to_string());

        let issue = make_issue("api", 42, &["bug"]);
        let action =
            IssueMigrationPlan::plan_issue(&issue, &migrations, Some("api"), None).unwrap();

        assert_eq!(action.issue().to_string(), "api#42");
        assert_eq!(action.removed(), &["bug"]);
        assert_eq!(action.added(), &["area:api", "type:fix"]);
        assert_eq!(action.target_labels(), &["area:api", "type:fix"]);
    }

    #[test]
    fn adds_default_type_when_issue_lacks_type() {
        let migrations = BTreeMap::new();
        let issue = make_issue("api", 42, &["area:api"]);
        let action =
            IssueMigrationPlan::plan_issue(&issue, &migrations, None, Some("chore")).unwrap();

        assert_eq!(action.issue().to_string(), "api#42");
        assert!(action.removed().is_empty());
        assert_eq!(action.added(), &["type:chore"]);
        assert_eq!(action.target_labels(), &["area:api", "type:chore"]);
    }

    #[test]
    fn adds_both_default_area_and_default_type_to_unlabeled_issue() {
        let migrations = BTreeMap::new();
        let issue = make_issue("core", 99, &[]);
        let action = IssueMigrationPlan::plan_issue(
            &issue,
            &migrations,
            Some("area:core"),
            Some("type:feat"),
        )
        .unwrap();

        assert_eq!(action.issue().to_string(), "core#99");
        assert!(action.removed().is_empty());
        assert_eq!(action.added(), &["area:core", "type:feat"]);
        assert_eq!(action.target_labels(), &["area:core", "type:feat"]);
    }

    #[test]
    fn leaves_compliant_issue_untouched() {
        let mut migrations = BTreeMap::new();
        migrations.insert("bug".to_string(), "type:fix".to_string());

        let issue = make_issue("app", 5, &["type:feat", "area:mobile"]);
        let action =
            IssueMigrationPlan::plan_issue(&issue, &migrations, Some("app"), Some("chore"));
        assert!(action.is_none());
    }

    #[test]
    fn deduplicates_when_mapped_label_already_present() {
        let mut migrations = BTreeMap::new();
        migrations.insert("enhancement".to_string(), "type:feat".to_string());

        let issue = make_issue("web", 10, &["type:feat", "enhancement", "area:web"]);
        let action = IssueMigrationPlan::plan_issue(&issue, &migrations, None, None).unwrap();

        assert_eq!(action.removed(), &["enhancement"]);
        assert!(action.added().is_empty());
        assert_eq!(action.target_labels(), &["area:web", "type:feat"]);
    }

    #[test]
    fn preserves_existing_or_mapped_type_without_applying_default_type() {
        let mut migrations = BTreeMap::new();
        migrations.insert("bug".to_string(), "type:fix".to_string());

        let issue = make_issue("web", 10, &["bug", "area:web"]);
        let action =
            IssueMigrationPlan::plan_issue(&issue, &migrations, None, Some("chore")).unwrap();

        assert_eq!(action.removed(), &["bug"]);
        assert_eq!(action.added(), &["type:fix"]);
        assert_eq!(action.target_labels(), &["area:web", "type:fix"]);
    }
}
