//! Configuration for repository target, repository settings, and audit rules, read from `tools.toml`.

use crate::adapters::github::target::Target;
pub use crate::domain::repo::{MilestoneState, RepoLabel, RepoMilestoneSpec, RepoSettings};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Repository configuration from `tools.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    /// GitHub configuration namespace.
    pub github: GithubConfig,
}

impl Config {
    /// Creates a Target from this configuration.
    #[must_use]
    pub fn target(&self) -> Target {
        Target::new(
            &self.github.repository.owner,
            &self.github.repository.name,
            self.github.project.as_ref().and_then(|p| p.number),
        )
    }

    /// The issues-check rules configuration.
    #[must_use]
    pub const fn rules(&self) -> &RulesConfig {
        &self.github.issues.rules
    }

    /// The repository settings configuration, if specified.
    #[must_use]
    pub const fn repo_settings(&self) -> Option<&RepositorySettingsConfig> {
        self.github.repository.settings.as_ref()
    }

    /// The repository labels configuration, if specified.
    #[must_use]
    pub const fn labels(&self) -> Option<&LabelsConfig> {
        self.github.labels.as_ref()
    }

    /// The repository milestones configuration, if specified.
    #[must_use]
    pub const fn milestones(&self) -> Option<&MilestonesConfig> {
        self.github.milestones.as_ref()
    }
}

/// GitHub configuration namespace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GithubConfig {
    /// Repository coordinates and settings.
    pub repository: RepositoryConfig,
    /// Project coordinates, if used.
    #[serde(default)]
    pub project: Option<ProjectConfig>,
    /// Issue rules configuration.
    #[serde(default)]
    pub issues: IssuesConfig,
    /// Repository labels configuration.
    #[serde(default)]
    pub labels: Option<LabelsConfig>,
    /// Repository milestones configuration.
    #[serde(default)]
    pub milestones: Option<MilestonesConfig>,
}

/// Repository coordinates and optional settings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RepositoryConfig {
    /// Repository owner (user or organization).
    pub owner: String,
    /// Repository name.
    #[serde(alias = "repository")]
    pub name: String,
    /// Target repository settings.
    #[serde(default)]
    pub settings: Option<RepositorySettingsConfig>,
}

/// Repository merge and branch settings enforced on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RepositorySettingsConfig {
    /// Whether merge commits are enabled.
    #[serde(default = "default_true")]
    pub allow_merge_commit: bool,
    /// Whether squash merges are enabled.
    #[serde(default)]
    pub allow_squash_merge: bool,
    /// Whether rebase merges are enabled.
    #[serde(default)]
    pub allow_rebase_merge: bool,
    /// Whether head branches are automatically deleted upon merge.
    #[serde(default = "default_true")]
    pub delete_branch_on_merge: bool,
    /// Default merge commit title format (e.g. `PR_TITLE` or `MERGE_MESSAGE`).
    pub merge_commit_title: String,
    /// Default merge commit message format (e.g. `PR_BODY`, `PR_TITLE`, or `BLANK`).
    pub merge_commit_message: String,
}

impl From<&RepositorySettingsConfig> for RepoSettings {
    fn from(config: &RepositorySettingsConfig) -> Self {
        Self {
            allow_merge_commit: config.allow_merge_commit,
            allow_squash_merge: config.allow_squash_merge,
            allow_rebase_merge: config.allow_rebase_merge,
            delete_branch_on_merge: config.delete_branch_on_merge,
            merge_commit_title: config.merge_commit_title.clone(),
            merge_commit_message: config.merge_commit_message.clone(),
        }
    }
}

/// Project parameters.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct ProjectConfig {
    /// Project number, if used.
    #[serde(default)]
    pub number: Option<u32>,
}

/// Settings for issues rules.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct IssuesConfig {
    /// Granular rule toggles.
    #[serde(default)]
    pub rules: RulesConfig,
}

/// Repository labels configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct LabelsConfig {
    /// List of label names to prune.
    #[serde(default)]
    pub deprecated: Vec<String>,
    /// Whether to prune unmanaged labels.
    #[serde(default)]
    pub prune_unmanaged: bool,
    /// Desired labels to enforce.
    #[serde(default)]
    pub items: Vec<RepoLabel>,
}

/// Repository milestones configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct MilestonesConfig {
    /// Deprecated milestone titles to close/prune.
    #[serde(default)]
    pub deprecated: Vec<String>,
    /// Whether to prune unmanaged milestones.
    #[serde(default)]
    pub prune_unmanaged: bool,
    /// Desired milestones to enforce.
    #[serde(default)]
    pub items: Vec<RepoMilestoneSpec>,
}

/// Granular boolean toggles for each issues-check rule.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RulesConfig {
    /// Cycle detection rule.
    #[serde(default = "default_true")]
    pub cycle: bool,
    /// Type and area label checks.
    #[serde(default = "default_true")]
    pub labels: bool,
    /// Project membership rule.
    #[serde(default = "default_true")]
    pub membership: bool,
    /// Milestone requirement rule.
    #[serde(default = "default_true")]
    pub milestone: bool,
    /// Parent requirement rule (every non-epic must have a parent).
    #[serde(default = "default_true", alias = "require_parent")]
    pub parent: bool,
    /// References / text rules (ADR number and SPDD path checks).
    #[serde(default = "default_true")]
    pub references: bool,
    /// Release issue rules.
    #[serde(default = "default_true")]
    pub release: bool,
    /// Text rules (alias / companion for references).
    #[serde(default = "default_true")]
    pub text: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            cycle: true,
            labels: true,
            membership: true,
            milestone: true,
            parent: true,
            references: true,
            release: true,
            text: true,
        }
    }
}

/// How loading the configuration can fail.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Reading the configuration file failed.
    #[error("failed to read config file at {path}: {source}")]
    Io {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// Parsing the configuration failed.
    #[error("failed to parse config file at {path}: {source}")]
    Parse {
        /// File path.
        path: PathBuf,
        /// Underlying TOML parse error.
        #[source]
        source: toml::de::Error,
    },
    /// `tools.toml` was not found.
    #[error("tools.toml not found in current directory or any parent")]
    NotFound,
}

/// Searches for `tools.toml` in `start_dir` and its parent directories.
#[must_use]
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        let candidate = dir.join("tools.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

/// Loads and parses configuration from the given file path.
///
/// # Errors
///
/// Returns [`ConfigError`] if the file cannot be read or parsed.
pub fn load_from_path(path: &Path) -> Result<Config, ConfigError> {
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Loads configuration from an explicit path or searches upwards from the current directory.
///
/// # Errors
///
/// Returns [`ConfigError`] if not found or if reading/parsing fails.
pub fn load_or_find(explicit_path: Option<&Path>) -> Result<Config, ConfigError> {
    if let Some(path) = explicit_path {
        load_from_path(path)
    } else {
        let cwd = std::env::current_dir().map_err(|source| ConfigError::Io {
            path: PathBuf::from("."),
            source,
        })?;
        let found = find_config(&cwd).ok_or(ConfigError::NotFound)?;
        load_from_path(&found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_domain_based_config() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.repository.settings]
            allow_merge_commit = true
            allow_squash_merge = false
            allow_rebase_merge = false
            delete_branch_on_merge = true
            merge_commit_title = "PR_TITLE"
            merge_commit_message = "PR_BODY"

            [github.project]
            number = 4

            [github.issues.rules]
            cycle = false
            labels = true
        "#;
        let config: Config = toml::from_str(toml_str).expect("should parse");
        let target = config.target();
        assert_eq!(target.owner(), "jaoppb");
        assert_eq!(target.repository(), "tools");
        assert_eq!(target.project(), Some(4));

        let settings = config.repo_settings().expect("settings should exist");
        assert!(settings.allow_merge_commit);
        assert!(!settings.allow_squash_merge);
        assert!(!settings.allow_rebase_merge);
        assert!(settings.delete_branch_on_merge);
        assert_eq!(settings.merge_commit_title, "PR_TITLE");
        assert_eq!(settings.merge_commit_message, "PR_BODY");

        assert!(!config.rules().cycle);
        assert!(config.rules().labels);
        assert!(config.rules().milestone);
    }

    #[test]
    fn parses_labels_and_milestones_config() {
        let toml_str = r##"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.labels]
            deprecated = ["bug", "epic"]
            prune_unmanaged = true
            items = [
                { name = "type:feat", color = "#1D76DB", description = "New feature" },
                { name = "type:fix", color = "d73a4a", description = "Bug fix" }
            ]

            [github.milestones]
            deprecated = ["v0.9"]
            items = [
                { title = "v0", description = "Bootstrap" },
                { title = "v1", description = "Stable", state = "closed" }
            ]
        "##;
        let config: Config = toml::from_str(toml_str).expect("should parse");
        let labels = config.labels().expect("labels should exist");
        assert_eq!(labels.deprecated, vec!["bug", "epic"]);
        assert!(labels.prune_unmanaged);
        assert_eq!(labels.items.len(), 2);
        assert_eq!(labels.items[0].name, "type:feat");
        assert_eq!(labels.items[0].color, "1d76db"); // normalized: no # and lowercase
        assert_eq!(labels.items[0].description, "New feature");

        let milestones = config.milestones().expect("milestones should exist");
        assert_eq!(milestones.deprecated, vec!["v0.9"]);
        assert_eq!(milestones.items.len(), 2);
        assert_eq!(milestones.items[0].title, "v0");
        assert_eq!(milestones.items[0].state, MilestoneState::Open); // default open
        assert_eq!(milestones.items[1].title, "v1");
        assert_eq!(milestones.items[1].state, MilestoneState::Closed);
    }
}
