//! The command line for `repo-sync`, parsed with clap.

use clap::{Arg, ArgAction, Command};
use std::collections::BTreeMap;
use std::path::PathBuf;

use tools_core::adapters::github::Target;
use tools_core::application::RepoSyncMode;
use tools_core::config::{Config, ConfigError, load_or_find};
use tools_core::domain::repo::{RepoPlanError, RepoSpec};

const PROGRAM: &str = "repo-sync";
const CHECK: &str = "check";
const DRY_RUN: &str = "dry-run";
const SETTINGS: &str = "settings";
const LABELS: &str = "labels";
const MILESTONES: &str = "milestones";
const PROJECT: &str = "project";
const CONFIG: &str = "config";
const OWNER: &str = "owner";
const REPO: &str = "repo";

/// Internal entity filter from command-line flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EntityFilter {
    settings: bool,
    labels: bool,
    milestones: bool,
    project: bool,
}

impl EntityFilter {
    fn apply(self, spec: RepoSpec) -> Result<RepoSpec, RepoResolveError> {
        Ok(RepoSpec {
            settings: select_entity(
                self.settings,
                spec.settings,
                "settings",
                "[github.repository.settings]",
            )?,
            labels: select_entity(self.labels, spec.labels, "labels", "[github.labels]")?,
            milestones: select_entity(
                self.milestones,
                spec.milestones,
                "milestones",
                "[github.milestones]",
            )?,
            project: select_entity(self.project, spec.project, "project", "[github.project]")?,
        })
    }
}

/// An error resolving repository configuration from tools.toml and CLI arguments.
#[derive(Debug, thiserror::Error)]
pub enum RepoResolveError {
    /// Failed to find or read configuration.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// Configuration or reconciliation planning validation error.
    #[error(transparent)]
    Plan(#[from] RepoPlanError),
    /// No repository settings, labels, milestones, or project configured in tools.toml.
    #[error("no repository settings, labels, milestones, or project configured in tools.toml")]
    EmptyConfig,
    /// Requested entity not configured in tools.toml.
    #[error(
        "repository {entity} requested via --{entity}, but not configured in tools.toml under {section}"
    )]
    MissingEntity {
        /// The entity name (e.g. `settings`, `labels`, `milestones`, `project`).
        entity: &'static str,
        /// The tools.toml section (e.g. `[github.labels]`).
        section: &'static str,
    },
    /// Organization override provided without target repositories.
    #[error(
        "organization override provided via --org/--owner, but no target repositories specified (pass --repo or define repositories in tools.toml)"
    )]
    NoRepositoriesForOrg,
}

/// A command line that was not run: an argument clap rejected, or `--help`.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct UsageError(#[from] clap::Error);

impl UsageError {
    /// The exit code clap gives it: 0 for `--help` and 2 for a rejected argument.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        u8::try_from(self.0.exit_code()).unwrap_or(2)
    }
}

/// The parsed command line for `repo-sync`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    mode: RepoSyncMode,
    explicit_entities: Option<EntityFilter>,
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repos: Vec<String>,
}

/// Resolved repository target and desired configuration specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepo {
    /// Repository target.
    pub target: Target,
    /// Shared desired configuration specification.
    pub spec: RepoSpec,
    /// Specifications of target repositories that declare a `[github.overrides.<repo>]` overlay.
    pub overrides: BTreeMap<String, RepoSpec>,
}

impl ResolvedRepo {
    /// The desired specification for `repo_name`: its overlay when declared, else the shared one.
    #[must_use]
    pub fn spec_for(&self, repo_name: &str) -> &RepoSpec {
        self.overrides.get(repo_name).unwrap_or(&self.spec)
    }
}

impl Arguments {
    /// Parses the arguments.
    ///
    /// # Errors
    ///
    /// Returns [`UsageError`] on invalid syntax or `--help`.
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, UsageError> {
        let matches = command().try_get_matches_from(arguments)?;
        let mode = if matches.get_flag(CHECK) {
            RepoSyncMode::Check
        } else if matches.get_flag(DRY_RUN) {
            RepoSyncMode::Preview
        } else {
            RepoSyncMode::Apply
        };

        let has_filter = matches.get_flag(SETTINGS)
            || matches.get_flag(LABELS)
            || matches.get_flag(MILESTONES)
            || matches.get_flag(PROJECT);

        let explicit_entities = if has_filter {
            Some(EntityFilter {
                settings: matches.get_flag(SETTINGS),
                labels: matches.get_flag(LABELS),
                milestones: matches.get_flag(MILESTONES),
                project: matches.get_flag(PROJECT),
            })
        } else {
            None
        };

        let repos: Vec<String> = matches
            .get_many::<String>(REPO)
            .map(|values| values.cloned().collect())
            .unwrap_or_default();

        Ok(Self {
            mode,
            explicit_entities,
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            owner: matches.get_one::<String>(OWNER).cloned(),
            repos,
        })
    }

    /// Execution mode.
    #[must_use]
    pub const fn mode(&self) -> RepoSyncMode {
        self.mode
    }

    /// Repositories specified on the command line.
    #[must_use]
    pub fn repos(&self) -> &[String] {
        &self.repos
    }

    /// Organization or owner override specified on the command line.
    #[must_use]
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    /// Resolves target and all configured resources from `tools.toml` and CLI flags.
    ///
    /// # Errors
    ///
    /// Fails when configuration cannot be found or read, or when a requested entity is not configured.
    pub fn resolve(&self) -> Result<ResolvedRepo, RepoResolveError> {
        let cfg = load_or_find(self.config_path.as_deref())?;
        self.resolve_with_config(cfg)
    }

    /// Resolves target and configuration specification from an already loaded [`Config`] and CLI flags.
    ///
    /// # Errors
    ///
    /// Fails when a requested entity is not configured or when tools.toml has no repository configuration.
    pub fn resolve_with_config(&self, cfg: Config) -> Result<ResolvedRepo, RepoResolveError> {
        if self.owner.is_some() && self.repos.is_empty() && cfg.github.repositories.is_empty() {
            return Err(RepoResolveError::NoRepositoriesForOrg);
        }

        let owner = self
            .owner
            .clone()
            .unwrap_or_else(|| cfg.github.repository.owner.clone());
        let project_number = cfg.github.project.as_ref().and_then(|p| p.number);
        let target = if !self.repos.is_empty() {
            Target::multi(owner, self.repos.clone(), project_number)
        } else if !cfg.github.repositories.is_empty() {
            Target::multi(owner, cfg.github.repositories.clone(), project_number)
        } else {
            Target::single(owner, cfg.github.repository.name.clone(), project_number)
        };

        let spec = self.finalize(cfg.spec_for("")?)?;
        let mut overrides = BTreeMap::new();
        for repo in target.repo_coords() {
            let name = repo.name();
            if cfg.overridden_repos().any(|overridden| overridden == name) {
                overrides.insert(name.to_string(), self.finalize(cfg.spec_for(name)?)?);
            }
        }

        Ok(ResolvedRepo {
            target,
            spec,
            overrides,
        })
    }

    /// Applies the entity filter and validates a resolved specification.
    fn finalize(&self, spec: RepoSpec) -> Result<RepoSpec, RepoResolveError> {
        let spec = match self.explicit_entities {
            Some(filter) => filter.apply(spec)?,
            None if spec.is_empty() => return Err(RepoResolveError::EmptyConfig),
            None => spec,
        };
        spec.validate()?;
        Ok(spec)
    }
}

fn select_entity<T>(
    flag: bool,
    opt: Option<T>,
    entity: &'static str,
    section: &'static str,
) -> Result<Option<T>, RepoResolveError> {
    match (flag, opt) {
        (true, Some(val)) => Ok(Some(val)),
        (true, None) => Err(RepoResolveError::MissingEntity { entity, section }),
        (false, _) => Ok(None),
    }
}

fn command() -> Command {
    Command::new(PROGRAM)
        .about("Synchronizes repository settings, labels, and milestones with tools.toml")
        .arg(
            Arg::new(CHECK)
                .long(CHECK)
                .action(ArgAction::SetTrue)
                .conflicts_with(DRY_RUN)
                .help("Audit GitHub configuration drift without modifying state"),
        )
        .arg(
            Arg::new(DRY_RUN)
                .long(DRY_RUN)
                .action(ArgAction::SetTrue)
                .conflicts_with(CHECK)
                .help("Preview reconciliation actions without modifying GitHub state"),
        )
        .arg(
            Arg::new(SETTINGS)
                .long(SETTINGS)
                .action(ArgAction::SetTrue)
                .help("Synchronize repository settings only"),
        )
        .arg(
            Arg::new(LABELS)
                .long(LABELS)
                .action(ArgAction::SetTrue)
                .help("Synchronize repository labels only"),
        )
        .arg(
            Arg::new(MILESTONES)
                .long(MILESTONES)
                .action(ArgAction::SetTrue)
                .help("Synchronize repository milestones only"),
        )
        .arg(
            Arg::new(PROJECT)
                .long(PROJECT)
                .action(ArgAction::SetTrue)
                .help("Synchronize repository project title only"),
        )
        .arg(
            Arg::new(CONFIG)
                .long(CONFIG)
                .value_name("FILE")
                .help("Path to tools.toml configuration file"),
        )
        .arg(
            Arg::new(OWNER)
                .long(OWNER)
                .visible_alias("org")
                .value_name("OWNER")
                .help("GitHub repository owner or organization override"),
        )
        .arg(
            Arg::new(REPO)
                .long(REPO)
                .value_name("REPO")
                .action(ArgAction::Append)
                .help("GitHub repository name override"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools_core::domain::repo::RepoProjectSpec;

    #[test]
    fn parses_flags_correctly() {
        let args = Arguments::parse(["repo-sync".into(), "--check".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Check);

        let args = Arguments::parse(["repo-sync".into(), "--dry-run".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Preview);

        let args = Arguments::parse(["repo-sync".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Apply);

        let args =
            Arguments::parse(["repo-sync".into(), "--org".into(), "gatuno-manga".into()]).unwrap();
        assert_eq!(args.owner(), Some("gatuno-manga"));

        let args_repos = Arguments::parse([
            "repo-sync".into(),
            "--repo".into(),
            "web".into(),
            "--repo".into(),
            "api".into(),
        ])
        .unwrap();
        assert_eq!(args_repos.repos(), &["web", "api"]);
    }

    #[test]
    fn resolve_fails_loud_when_config_is_missing() {
        let args = Arguments::parse([
            "repo-sync".into(),
            "--config".into(),
            "non-existent-tools.toml".into(),
        ])
        .unwrap();
        let err = args.resolve().unwrap_err();
        assert!(matches!(
            err,
            RepoResolveError::Config(ConfigError::Io { .. })
        ));
    }

    #[test]
    fn resolve_fails_loud_when_requested_entity_is_missing() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args = Arguments::parse(["repo-sync".into(), "--labels".into()]).unwrap();
        let err = args.resolve_with_config(config.clone()).unwrap_err();
        assert!(matches!(
            err,
            RepoResolveError::MissingEntity {
                entity: "labels",
                section: "[github.labels]"
            }
        ));

        let args_all_flags = Arguments::parse([
            "repo-sync".into(),
            "--settings".into(),
            "--labels".into(),
            "--milestones".into(),
        ])
        .unwrap();
        let err_all_flags = args_all_flags
            .resolve_with_config(config.clone())
            .unwrap_err();
        assert!(matches!(
            err_all_flags,
            RepoResolveError::MissingEntity {
                entity: "settings",
                section: "[github.repository.settings]"
            }
        ));

        let args_all = Arguments::parse(["repo-sync".into()]).unwrap();
        let err_all = args_all.resolve_with_config(config).unwrap_err();
        assert!(matches!(err_all, RepoResolveError::EmptyConfig));
    }

    #[test]
    fn resolve_filters_selected_entities() {
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

            [github.labels]
            items = [
                { name = "type:feat", color = "1d76db", description = "Feature" }
            ]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        // When --labels is passed, settings should be None
        let args = Arguments::parse(["repo-sync".into(), "--labels".into()]).unwrap();
        let resolved = args.resolve_with_config(config.clone()).unwrap();
        assert!(resolved.spec.settings.is_none());
        assert!(resolved.spec.labels.is_some());
        assert!(resolved.spec.milestones.is_none());

        // When no filter is passed, configured entities should be Some
        let args_all = Arguments::parse(["repo-sync".into()]).unwrap();
        let resolved_all = args_all.resolve_with_config(config).unwrap();
        assert!(resolved_all.spec.settings.is_some());
        assert!(resolved_all.spec.labels.is_some());
        assert!(resolved_all.spec.milestones.is_none());
        assert!(resolved_all.spec.project.is_none());
    }

    #[test]
    fn resolve_filters_project_entity() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.project]
            number = 4
            title = "Tools Roadmap"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args = Arguments::parse(["repo-sync".into(), "--project".into()]).unwrap();
        let resolved = args.resolve_with_config(config.clone()).unwrap();
        assert!(resolved.spec.settings.is_none());
        assert!(resolved.spec.labels.is_none());
        assert!(resolved.spec.milestones.is_none());
        assert_eq!(
            resolved.spec.project,
            Some(RepoProjectSpec::new(4, "Tools Roadmap"))
        );
    }

    #[test]
    fn resolve_fails_when_project_title_missing_number() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.project]
            title = "Tools Roadmap"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args = Arguments::parse(["repo-sync".into()]).unwrap();
        let err = args.resolve_with_config(config).unwrap_err();
        assert!(matches!(
            err,
            RepoResolveError::Plan(RepoPlanError::Validation(_))
        ));
        assert!(err.to_string().contains("project number is required"));
    }

    #[test]
    fn resolve_applies_repository_overlay_per_target() {
        let toml_str = r#"
            [github.repository]
            owner = "gatuno-manga"
            name = "web"

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]

            [github.overrides.web.labels]
            items = [{ name = "area:ssr", color = "2ea44f", description = "SSR" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args = Arguments::parse([
            "repo-sync".into(),
            "--repo".into(),
            "web".into(),
            "--repo".into(),
            "api".into(),
        ])
        .unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        assert_eq!(
            resolved
                .spec_for("web")
                .labels
                .as_ref()
                .unwrap()
                .items
                .len(),
            2
        );
        assert_eq!(
            resolved
                .spec_for("api")
                .labels
                .as_ref()
                .unwrap()
                .items
                .len(),
            1
        );
        assert_eq!(resolved.overrides.len(), 1);
    }

    #[test]
    fn resolve_ignores_overlay_of_untargeted_repo() {
        let toml_str = r#"
            [github.repository]
            owner = "gatuno-manga"
            name = "web"

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]

            [github.overrides.infra.labels]
            items = [{ name = "area:ci", color = "2ea44f", description = "CI" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let args = Arguments::parse(["repo-sync".into()]).unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        assert!(resolved.overrides.is_empty());
    }

    #[test]
    fn resolve_targets_multiple_repos_from_cli() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args = Arguments::parse([
            "repo-sync".into(),
            "--repo".into(),
            "web".into(),
            "--repo".into(),
            "api".into(),
        ])
        .unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        let coords: Vec<_> = resolved
            .target
            .repo_coords()
            .iter()
            .map(|r| (r.owner().to_string(), r.name().to_string()))
            .collect();
        assert_eq!(
            coords,
            vec![
                ("jaoppb".to_string(), "web".to_string()),
                ("jaoppb".to_string(), "api".to_string())
            ]
        );
    }

    #[test]
    fn resolve_targets_single_repo_from_cli() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args = Arguments::parse(["repo-sync".into(), "--repo".into(), "web".into()]).unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        let coords: Vec<_> = resolved
            .target
            .repo_coords()
            .iter()
            .map(|r| (r.owner().to_string(), r.name().to_string()))
            .collect();
        assert_eq!(coords, vec![("jaoppb".to_string(), "web".to_string())]);
    }

    #[test]
    fn resolve_targets_org_and_multiple_repos_from_cli() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args = Arguments::parse([
            "repo-sync".into(),
            "--org".into(),
            "gatuno-manga".into(),
            "--repo".into(),
            "web".into(),
            "--repo".into(),
            "api".into(),
        ])
        .unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        let coords: Vec<_> = resolved
            .target
            .repo_coords()
            .iter()
            .map(|r| (r.owner().to_string(), r.name().to_string()))
            .collect();
        assert_eq!(
            coords,
            vec![
                ("gatuno-manga".to_string(), "web".to_string()),
                ("gatuno-manga".to_string(), "api".to_string())
            ]
        );
    }

    #[test]
    fn resolve_targets_org_with_config_repositories() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github]
            repositories = ["web", "api"]

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args =
            Arguments::parse(["repo-sync".into(), "--org".into(), "gatuno-manga".into()]).unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        let coords: Vec<_> = resolved
            .target
            .repo_coords()
            .iter()
            .map(|r| (r.owner().to_string(), r.name().to_string()))
            .collect();
        assert_eq!(
            coords,
            vec![
                ("gatuno-manga".to_string(), "web".to_string()),
                ("gatuno-manga".to_string(), "api".to_string())
            ]
        );
    }

    #[test]
    fn resolve_fails_loud_when_org_provided_without_target_repositories() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args =
            Arguments::parse(["repo-sync".into(), "--org".into(), "gatuno-manga".into()]).unwrap();
        let err = args.resolve_with_config(config).unwrap_err();
        assert!(matches!(err, RepoResolveError::NoRepositoriesForOrg));
        assert_eq!(
            err.to_string(),
            "organization override provided via --org/--owner, but no target repositories specified (pass --repo or define repositories in tools.toml)"
        );

        // Also test with explicit repositories = []
        let toml_empty_repos = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github]
            repositories = []

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]
        "#;
        let config_empty: Config = toml::from_str(toml_empty_repos).unwrap();
        let err_empty = args.resolve_with_config(config_empty).unwrap_err();
        assert!(matches!(err_empty, RepoResolveError::NoRepositoriesForOrg));
    }

    #[test]
    fn resolve_falls_back_to_config_repository_when_no_overrides() {
        let toml_str = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db", description = "Feature" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        let args = Arguments::parse(["repo-sync".into()]).unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        let coords: Vec<_> = resolved
            .target
            .repo_coords()
            .iter()
            .map(|r| (r.owner().to_string(), r.name().to_string()))
            .collect();
        assert_eq!(coords, vec![("jaoppb".to_string(), "tools".to_string())]);
    }
}
