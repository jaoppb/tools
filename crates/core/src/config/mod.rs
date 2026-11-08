//! Configuration for repository target, repository settings, and audit rules, read from `tools.toml`.

use crate::adapters::github::target::Target;
pub use crate::domain::repo::{
    MergeCommitMessage, MergeCommitTitle, MilestoneState, ProjectDiff, RepoLabel,
    RepoLabelsOverlay, RepoLabelsSpec, RepoMilestone, RepoMilestoneSpec, RepoMilestonesOverlay,
    RepoMilestonesSpec, RepoPlanError, RepoProject, RepoProjectSpec, RepoSettings, RepoSpec,
    Unmanaged,
};
use crate::workflow::{LoadError, Workflow, WorkflowConfig};
pub mod migrate;
mod milestone_format;
mod schema;
pub use milestone_format::{MilestoneFormat, TitlePattern};
use schema::{
    LabelsConfig, LabelsOverlayConfig, MilestonesConfig, MilestonesOverlayConfig,
    RepositorySettingsConfig,
};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Repository configuration from `tools.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Config {
    /// GitHub configuration namespace.
    github: GithubConfig,
    /// The `[workflow]` block, when the file has one.
    #[serde(default)]
    workflow: Option<WorkflowConfig>,
    /// Deprecations of the loaded file, for the commands that read the deprecated keys to print.
    #[serde(skip)]
    warnings: Vec<String>,
}

impl Config {
    /// Creates a Target from this configuration.
    #[must_use]
    pub fn target(&self) -> Target {
        let project = self.github.project.as_ref().and_then(|p| p.number);
        if self.github.repositories.is_empty() {
            Target::single(
                &self.github.repository.owner,
                &self.github.repository.name,
                project,
            )
        } else {
            Target::multi(
                &self.github.repository.owner,
                self.github.repositories.clone(),
                project,
            )
        }
    }

    /// The compiled `[workflow]` block, `None` when the file has none.
    ///
    /// Compiles on every call (milliseconds); call it once and keep the result.
    ///
    /// # Errors
    ///
    /// A [`LoadError`]; a file read by the `load_*` functions has been
    /// validated already, so this only fails for a `Config` built another way.
    pub fn workflow(&self) -> Result<Option<Workflow>, LoadError> {
        self.workflow
            .as_ref()
            .map(|block| block.compile(None))
            .transpose()
    }

    /// The issues-check rules configuration.
    #[must_use]
    pub const fn rules(&self) -> &RulesConfig {
        &self.github.issues.rules
    }

    /// The legacy-to-canonical label mappings of `[github.labels].migrations`, empty when unset.
    #[must_use]
    pub fn label_migrations(&self) -> std::collections::BTreeMap<String, String> {
        self.github
            .labels
            .as_ref()
            .map(|labels| labels.migrations().clone())
            .unwrap_or_default()
    }

    /// The repository owner (user or organization).
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.github.repository.owner
    }

    /// The repository name of `[github.repository]`.
    #[must_use]
    pub fn repository_name(&self) -> &str {
        &self.github.repository.name
    }

    /// The multi-repository targets of `[github].repositories`, empty when unset.
    #[must_use]
    pub fn repositories(&self) -> &[String] {
        &self.github.repositories
    }

    /// The project number of `[github.project]`, if set.
    #[must_use]
    pub fn project_number(&self) -> Option<u32> {
        self.github
            .project
            .as_ref()
            .and_then(|project| project.number)
    }

    /// The `[github.issue_templates]` table, if declared.
    #[must_use]
    pub const fn issue_templates(&self) -> Option<&IssueTemplatesConfig> {
        self.github.issue_templates.as_ref()
    }

    /// Deprecations of the loaded file, for the commands that read the deprecated keys to print.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Resolves the desired specification for `repo_name`: the shared labels and milestones with
    /// that repository's `[github.overrides.<repo>]` overlay applied on top, if any.
    ///
    /// # Errors
    ///
    /// Returns [`RepoPlanError::Validation`] if project title is configured without a project number.
    pub fn spec_for(&self, repo_name: &str) -> Result<RepoSpec, RepoPlanError> {
        let project = match &self.github.project {
            Some(p) => match (p.number, p.title.as_deref()) {
                (Some(number), Some(title)) => Some(RepoProjectSpec::new(number, title)),
                (None, Some(_)) => {
                    return Err(RepoPlanError::Validation(
                        "project number is required in [github.project] when title is configured"
                            .into(),
                    ));
                }
                _ => None,
            },
            None => None,
        };
        let overlay = self.github.overrides.get(repo_name);
        let labels = match overlay.and_then(|o| o.labels.as_ref()) {
            Some(over) => {
                let base = self.github.labels.clone().map(RepoLabelsSpec::from);
                Some(RepoLabelsOverlay::from(over.clone()).apply_to(base.as_ref())?)
            }
            None => self.github.labels.clone().map(RepoLabelsSpec::from),
        };
        let milestones = match overlay.and_then(|o| o.milestones.as_ref()) {
            Some(over) => {
                let base = self.github.milestones.clone().map(RepoMilestonesSpec::from);
                Some(RepoMilestonesOverlay::from(over.clone()).apply_to(base.as_ref())?)
            }
            None => self.github.milestones.clone().map(RepoMilestonesSpec::from),
        };
        Ok(RepoSpec::new(
            self.github
                .repository
                .settings
                .clone()
                .map(RepoSettings::from),
            labels,
            milestones,
            project,
        ))
    }
}

/// Per-repository overlay of the shared labels and milestones, from `[github.overrides.<repo>]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoOverrideConfig {
    /// Labels overlay.
    #[serde(default)]
    labels: Option<LabelsOverlayConfig>,
    /// Milestones overlay.
    #[serde(default)]
    milestones: Option<MilestonesOverlayConfig>,
}

/// GitHub configuration namespace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GithubConfig {
    /// Repository coordinates and settings.
    repository: RepositoryConfig,
    /// Multi-repository targets, if configured.
    #[serde(default)]
    repositories: Vec<String>,
    /// Project coordinates, if used.
    #[serde(default)]
    project: Option<ProjectConfig>,
    /// Issue rules configuration.
    #[serde(default)]
    issues: IssuesConfig,
    /// Repository labels configuration.
    #[serde(default)]
    labels: Option<LabelsConfig>,
    /// Repository milestones configuration.
    #[serde(default)]
    milestones: Option<MilestonesConfig>,
    /// Per-repository label and milestone overlays, keyed by repository name.
    #[serde(default)]
    overrides: std::collections::BTreeMap<String, RepoOverrideConfig>,
    /// The issue templates and the labels they apply, if declared.
    #[serde(default)]
    issue_templates: Option<IssueTemplatesConfig>,
}

/// The `[github.issue_templates]` table: which template applies which label.
///
/// `tools repo check` compares it with `.github/ISSUE_TEMPLATE/` offline.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueTemplatesConfig {
    /// Whether every template file must have an entry (and so a linked label).
    #[serde(default = "default_require_label")]
    pub require_label: bool,
    /// The declared templates.
    #[serde(default)]
    pub items: Vec<IssueTemplate>,
}

const fn default_require_label() -> bool {
    true
}

/// One issue template: its file, the single label it applies, and its title prefix.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueTemplate {
    /// File name inside `.github/ISSUE_TEMPLATE/`.
    pub file: String,
    /// The label the template applies; it must be declared in `[github.labels]`.
    pub label: String,
    /// What the template `title:` must start with.
    pub title: String,
}

/// Repository coordinates and optional settings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RepositoryConfig {
    /// Repository owner (user or organization).
    owner: String,
    /// Repository name.
    #[serde(alias = "repository")]
    name: String,
    /// Target repository settings.
    #[serde(default)]
    settings: Option<RepositorySettingsConfig>,
}

/// Project parameters.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct ProjectConfig {
    /// Project number, if used.
    #[serde(default)]
    number: Option<u32>,
    /// Project title, if specified.
    #[serde(default)]
    title: Option<String>,
}

/// Settings for issues rules.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct IssuesConfig {
    /// Granular rule toggles.
    #[serde(default)]
    rules: RulesConfig,
}

/// Granular boolean toggles for each issues-check rule.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
    /// Release issue rules.
    #[serde(default = "default_true")]
    pub release: bool,
    /// Priority requirement rule for Ready gate.
    #[serde(default)]
    pub priority: bool,
    /// Effort requirement rule for Ready gate.
    #[serde(default)]
    pub effort: bool,
    /// Configurable area label prefixes.
    #[serde(default = "default_area_prefixes")]
    pub area_prefixes: Vec<String>,
    /// What a milestone title must look like: `semver`, `any` or a regex.
    #[serde(default)]
    pub milestone_format: MilestoneFormat,
}

fn default_area_prefixes() -> Vec<String> {
    vec!["area:".to_string()]
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
            release: true,
            priority: false,
            effort: false,
            area_prefixes: default_area_prefixes(),
            milestone_format: MilestoneFormat::default(),
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
    /// The `[workflow]` block is invalid.
    #[error("invalid [workflow] block in {path}: {source}")]
    Workflow {
        /// File path.
        path: PathBuf,
        /// What is wrong, with its line and column.
        #[source]
        source: Box<LoadError>,
    },
    /// A key that no longer exists is still set.
    #[error(
        "{path}: `prune_unmanaged` in [{table}] was removed; set `unmanaged = \"prune\"` (was true) or `unmanaged = \"warn\"` (was false)"
    )]
    RemovedPruneUnmanaged {
        /// File path.
        path: PathBuf,
        /// The table that still sets it, for example `github.labels`.
        table: String,
    },
    /// The removed `[github.project.status_mapping]` is still set.
    #[error(
        "{path}: [github.project.status_mapping] was removed; board columns must equal [workflow].statuses. Run `tools config migrate` to rewrite the file"
    )]
    RemovedStatusMapping {
        /// File path.
        path: PathBuf,
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

/// Loads and parses configuration from a string.
///
/// # Errors
///
/// Returns [`ConfigError`] if parsing fails.
pub fn load_from_str(contents: &str) -> Result<Config, ConfigError> {
    parse(contents, Path::new("<memory>"))
}

/// Parses `contents` and validates its `[workflow]` block.
fn parse(contents: &str, path: &Path) -> Result<Config, ConfigError> {
    reject_prune_unmanaged(contents, path)?;
    reject_status_mapping(contents, path)?;
    let mut config: Config = toml::from_str(contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    if let Some(block) = &config.workflow {
        block
            .compile(Some(contents))
            .map_err(|source| ConfigError::Workflow {
                path: path.to_path_buf(),
                source: Box::new(source),
            })?;
    }
    config.warnings = deprecations(contents);
    Ok(config)
}

/// Fails, naming `tools config migrate`, when the removed `[github.project.status_mapping]` is
/// still set.
fn reject_status_mapping(contents: &str, path: &Path) -> Result<(), ConfigError> {
    let set = contents.parse::<toml::Table>().ok().is_some_and(|root| {
        root.get("github")
            .and_then(|github| github.get("project"))
            .is_some_and(|project| project.get("status_mapping").is_some())
    });
    if set {
        Err(ConfigError::RemovedStatusMapping {
            path: path.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

/// The deprecations of a file that still loads.
fn deprecations(contents: &str) -> Vec<String> {
    let Ok(root) = contents.parse::<toml::Table>() else {
        return Vec::new();
    };
    let has_rules = root
        .get("github")
        .and_then(|github| github.get("issues"))
        .is_some_and(|issues| issues.get("rules").is_some());
    match (has_rules, root.contains_key("workflow")) {
        (false, _) => Vec::new(),
        (true, true) => vec![
            "[github.issues.rules] is deprecated and will be removed with #59 (the issue audit still reads it)"
                .to_owned(),
        ],
        (true, false) => vec![
            "[github.issues.rules] is deprecated and will be removed with #59; run `tools config migrate` to add the [workflow] equivalent"
                .to_owned(),
        ],
    }
}

/// Fails with a message naming the replacement when a table still sets the removed
/// `prune_unmanaged`, which the strict tables would otherwise report as an unknown field.
fn reject_prune_unmanaged(contents: &str, path: &Path) -> Result<(), ConfigError> {
    let Ok(root) = contents.parse::<toml::Table>() else {
        return Ok(()); // the typed parse reports the syntax error
    };
    let Some(github) = root.get("github").and_then(toml::Value::as_table) else {
        return Ok(());
    };
    let mut tables: Vec<(String, &toml::Table)> = Vec::new();
    for kind in ["labels", "milestones"] {
        if let Some(table) = github.get(kind).and_then(toml::Value::as_table) {
            tables.push((format!("github.{kind}"), table));
        }
        for (repo, over) in github
            .get("overrides")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flatten()
        {
            if let Some(table) = over.get(kind).and_then(toml::Value::as_table) {
                tables.push((format!("github.overrides.{repo}.{kind}"), table));
            }
        }
    }
    match tables
        .into_iter()
        .find(|(_, table)| table.contains_key("prune_unmanaged"))
    {
        Some((table, _)) => Err(ConfigError::RemovedPruneUnmanaged {
            path: path.to_path_buf(),
            table,
        }),
        None => Ok(()),
    }
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
    parse(&contents, path)
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
        if let Some(found) = find_config(&cwd) {
            load_from_path(&found)
        } else {
            Err(ConfigError::NotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_templates_default_to_requiring_a_label() {
        let config: Config = toml::from_str(
            r#"
            [github.repository]
            owner = "o"
            name = "n"

            [github.issue_templates]
            items = [{ file = "feat.yml", label = "type:feat", title = "feat: " }]
            "#,
        )
        .unwrap();
        let templates = config.github.issue_templates.unwrap();
        assert!(templates.require_label);
        assert_eq!(templates.items[0].file, "feat.yml");
    }

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

        let settings = config
            .spec_for("tools")
            .unwrap()
            .settings
            .expect("settings should exist");
        assert!(settings.allow_merge_commit);
        assert!(!settings.allow_squash_merge);
        assert!(!settings.allow_rebase_merge);
        assert!(settings.delete_branch_on_merge);
        assert_eq!(settings.merge_commit_title, MergeCommitTitle::PrTitle);
        assert_eq!(settings.merge_commit_message, MergeCommitMessage::PrBody);

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
            unmanaged = "prune"
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
        let labels = config
            .spec_for("tools")
            .unwrap()
            .labels
            .expect("labels should exist");
        assert_eq!(labels.deprecated, vec!["bug", "epic"]);
        assert_eq!(labels.unmanaged, Unmanaged::Prune);
        assert_eq!(labels.items.len(), 2);
        assert_eq!(labels.items[0].name, "type:feat");
        assert_eq!(labels.items[0].color, "1d76db"); // normalized: no # and lowercase
        assert_eq!(labels.items[0].description, "New feature");

        let milestones = config
            .spec_for("tools")
            .unwrap()
            .milestones
            .expect("milestones should exist");
        assert_eq!(milestones.deprecated, vec!["v0.9"]);
        assert_eq!(milestones.items.len(), 2);
        assert_eq!(milestones.items[0].title, "v0");
        assert_eq!(milestones.items[0].description, "Bootstrap");
        assert_eq!(milestones.items[0].state, MilestoneState::Open); // default open
        assert_eq!(milestones.items[1].title, "v1");
        assert_eq!(milestones.items[1].description, "Stable");
        assert_eq!(milestones.items[1].state, MilestoneState::Closed);
    }

    #[test]
    fn parses_renames_and_project_title_config() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.project]
            number = 4
            title = "Tools Roadmap"

            [github.labels]
            renames = { "enhancement" = "type:feat" }
            items = [
                { name = "type:feat", color = "1d76db", description = "Feature" }
            ]

            [github.milestones]
            renames = { "v0.9" = "v1" }
            items = [
                { title = "v1", description = "Release" }
            ]
        "#;
        let config: Config = toml::from_str(toml_str).expect("should parse");
        let project = config
            .github
            .project
            .as_ref()
            .expect("project should exist");
        assert_eq!(project.number, Some(4));
        assert_eq!(project.title.as_deref(), Some("Tools Roadmap"));

        let labels = config
            .spec_for("tools")
            .unwrap()
            .labels
            .expect("labels should exist");
        assert_eq!(
            labels.renames.get("enhancement"),
            Some(&"type:feat".to_string())
        );

        let milestones = config
            .spec_for("tools")
            .unwrap()
            .milestones
            .expect("milestones should exist");
        assert_eq!(milestones.renames.get("v0.9"), Some(&"v1".to_string()));

        let repo_spec = config.spec_for("tools").unwrap();
        let proj_spec = repo_spec.project.expect("project spec should exist");
        assert_eq!(proj_spec.number, 4);
        assert_eq!(proj_spec.title, "Tools Roadmap");
    }

    #[test]
    fn parses_multi_repo_target_config() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github]
            repositories = ["tools", "web", "api"]

            [github.project]
            number = 4
        "#;
        let config: Config = toml::from_str(toml_str).expect("should parse");
        let target = config.target();
        assert_eq!(target.repos().len(), 3);
        assert_eq!(target.owner(), "jaoppb");
        assert_eq!(target.project(), Some(4));
        assert!(target.contains_repo("tools"));
        assert!(target.contains_repo("web"));
        assert!(target.contains_repo("api"));
        assert!(!target.contains_repo("other"));
    }

    #[test]
    fn parses_label_migrations_config() {
        let toml_str = r#"
            [github.repository]
            owner = "gatuno-manga"
            name = "web"

            [github.labels]
            migrations = { "legacy-bug" = "type:fix", "ui" = "area:web" }
        "#;
        let config: Config = toml::from_str(toml_str).expect("should parse");
        let labels = config
            .spec_for("web")
            .unwrap()
            .labels
            .expect("labels config exists");
        assert_eq!(
            labels.migrations.get("legacy-bug"),
            Some(&"type:fix".to_string())
        );
        assert_eq!(labels.migrations.len(), 2);
        assert_eq!(labels.migrations.get("ui"), Some(&"area:web".to_string()));
        assert!(!labels.migrations.contains_key("bug"));
    }

    #[test]
    fn parses_milestone_format_config() {
        let parse = |format: &str| {
            load_from_str(&format!("[github.repository]\nowner = 'o'\nname = 'n'\n[github.issues.rules]\nmilestone_format = '{format}'"))
                .map(|config| config.rules().milestone_format.clone())
        };
        assert_eq!(parse("semver").unwrap(), MilestoneFormat::Semver);
        assert_eq!(parse("any").unwrap(), MilestoneFormat::Any);
        assert_eq!(
            parse("^Fase \\d+").unwrap(),
            MilestoneFormat::Pattern(TitlePattern::new("^Fase \\d+").unwrap())
        );
        assert!(parse("(unclosed").is_err());
        assert_eq!(
            RulesConfig::default().milestone_format,
            MilestoneFormat::Semver
        );
    }

    #[test]
    fn rules_reject_the_removed_references_and_text_keys() {
        for key in ["references", "text"] {
            let toml = format!(
                "[github.repository]\nowner = 'o'\nname = 'n'\n[github.issues.rules]\n{key} = false\n"
            );
            assert!(load_from_str(&toml).is_err(), "{key} should be rejected");
        }
    }

    #[test]
    fn rules_keep_the_require_parent_alias() {
        let config = load_from_str(
            "[github.repository]\nowner = 'o'\nname = 'n'\n[github.issues.rules]\nrequire_parent = false\n",
        )
        .expect("the alias loads");
        assert!(!config.rules().parent);
    }

    #[test]
    fn parses_custom_area_prefixes_config() {
        let toml_str = r#"
            [github.repository]
            owner = "gatuno-manga"
            name = "web"

            [github.issues.rules]
            area_prefixes = ["area:", "área:"]
        "#;
        let config: Config = toml::from_str(toml_str).expect("should parse");
        assert_eq!(config.rules().area_prefixes, vec!["area:", "área:"]);
    }

    #[test]
    fn spec_for_applies_repository_overlay() {
        let toml_str = r##"
            [github.repository]
            owner = "gatuno-manga"
            name = "web"

            [github]
            repositories = ["web", "api"]

            [github.labels]
            unmanaged = "prune"
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]

            [github.overrides.web.labels]
            unmanaged = "error"
            items = [{ name = "area:ssr", color = "2ea44f", description = "SSR" }]

            [github.overrides.infra.milestones]
            items = [{ title = "v1", description = "Infra" }]
        "##;
        let config: Config = toml::from_str(toml_str).expect("should parse");
        let web = config.spec_for("web").unwrap();
        let web_labels = web.labels.expect("labels");
        assert_eq!(web_labels.items.len(), 2);
        assert_eq!(web_labels.unmanaged, Unmanaged::Error);

        let api = config.spec_for("api").unwrap();
        let api_labels = api.labels.expect("labels");
        assert_eq!(api_labels.items.len(), 1);
        assert_eq!(api_labels.unmanaged, Unmanaged::Prune);
        assert!(api.milestones.is_none());

        let infra = config.spec_for("infra").unwrap();
        assert_eq!(infra.milestones.expect("milestones").items.len(), 1);
    }

    #[test]
    fn overlay_rejects_unknown_keys() {
        let toml_str = r#"
            [github.repository]
            owner = "o"
            name = "web"

            [github.overrides.web.labels]
            prune_unmanged = true
        "#;
        assert!(toml::from_str::<Config>(toml_str).is_err());
    }

    #[test]
    fn removed_prune_unmanaged_fails_naming_the_replacement() {
        for table in [
            "github.labels",
            "github.milestones",
            "github.overrides.web.labels",
            "github.overrides.web.milestones",
        ] {
            let toml = format!(
                "[github.repository]\nowner = \"o\"\nname = \"r\"\n\n[{table}]\nprune_unmanaged = true\n"
            );
            let message = load_from_str(&toml).unwrap_err().to_string();
            assert!(message.contains("prune_unmanaged"), "{message}");
            assert!(message.contains(&format!("[{table}]")), "{message}");
            assert!(message.contains("unmanaged = \"prune\""), "{message}");
        }
    }

    #[test]
    fn unmanaged_defaults_differ_per_kind() {
        let config = load_from_str(
            "[github.repository]\nowner = \"o\"\nname = \"r\"\n\n[github.labels]\nitems = []\n\n[github.milestones]\nitems = []\n",
        )
        .unwrap();
        assert_eq!(
            config.spec_for("r").unwrap().labels.unwrap().unmanaged,
            Unmanaged::Warn
        );
        assert_eq!(
            config.spec_for("r").unwrap().milestones.unwrap().unmanaged,
            Unmanaged::Error
        );
    }
}
