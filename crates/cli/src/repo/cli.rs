//! The command line for `tools repo check|sync`, parsed with clap.

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::path::PathBuf;

use tools_core::adapters::github::Target;
use tools_core::application::RepoSyncMode;
use tools_core::config::{Config, ConfigError, IssueTemplatesConfig, find_config, load_or_find};
use tools_core::domain::repo::{Repo, RepoPlanError, RepoSpec};

use crate::cli::{CONFIG, OWNER, repos_of};

/// The verb that audits drift without writing.
pub const CHECK: &str = "check";
/// The verb that reconciles GitHub to `tools.toml`.
pub const SYNC: &str = "sync";
const SETTINGS: &str = "settings";
const LABELS: &str = "labels";
const MILESTONES: &str = "milestones";
const PROJECT: &str = "project";

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

/// The parsed command line of `tools repo check|sync`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    mode: RepoSyncMode,
    explicit_entities: Option<EntityFilter>,
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repos: Vec<String>,
}

/// Resolved repository target and the desired specification of each target repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepo {
    /// Repository target.
    pub target: Target,
    /// Each target repository with its desired specification, overlay applied.
    pub repos: Vec<(Repo, RepoSpec)>,
    /// What the offline step compares with the local files; `None` when this run skips the step,
    /// as `tools repo check --help` states.
    pub local_files: Option<LocalFiles>,
}

/// The declared issue templates and docs that the offline step of `repo check` compares with the
/// files next to `tools.toml`. Both are opt-in; with neither, no file is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFiles {
    /// The `[github.issue_templates]` table, if declared.
    pub templates: Option<IssueTemplatesConfig>,
    /// The docs of `[github.labels].docs` whose label block is checked.
    pub docs: Vec<String>,
}

/// When the offline step runs, as `tools repo check --help` states it.
const LOCAL_FILES_HELP: &str = "\
Before any GitHub call, `check` compares local files next to tools.toml with it: the issue
templates when [github.issue_templates] is declared, and the label block of each doc listed in
[github.labels].docs. With neither declared, no file is read. The step runs only when the check
targets the configured repository alone (no --repo, no --org/--owner, no [github].repositories
list) and covers labels (no scope flag, or --labels); otherwise it is skipped. A problem exits 1.";

impl Arguments {
    /// Reads the arguments of the `check` or `sync` verb; the global flags travel with `matches`.
    #[must_use]
    pub fn from_matches(verb: &str, matches: &ArgMatches) -> Self {
        let mode = if verb == CHECK {
            RepoSyncMode::Check
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

        let repos = repos_of(matches);

        Self {
            mode,
            explicit_entities,
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            owner: matches.get_one::<String>(OWNER).cloned(),
            repos,
        }
    }

    /// Execution mode.
    #[must_use]
    pub const fn mode(&self) -> RepoSyncMode {
        self.mode
    }

    /// Whether this run includes the offline step on the local files: a `check` of the configured
    /// repository alone (no `--repo`, no `--org`/`--owner`, no `[github].repositories` list) that
    /// covers labels (no scope flag, or `--labels`). The files belong to the checkout holding
    /// `tools.toml`, which is that repository and no other.
    fn checks_local_files(&self, cfg: &Config) -> bool {
        self.mode == RepoSyncMode::Check
            && self.owner.is_none()
            && self.repos.is_empty()
            && cfg.repositories().is_empty()
            && self.explicit_entities.is_none_or(|filter| filter.labels)
    }

    /// The directory holding `tools.toml` (found the way [`load_or_find`] finds it), where the templates and docs live.
    #[must_use]
    pub fn root(&self) -> PathBuf {
        let cwd = || PathBuf::from(".");
        let found = self.config_path.clone().or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|dir| find_config(&dir))
        });
        found
            .and_then(|path| path.parent().map(PathBuf::from))
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or_else(cwd)
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
        if self.owner.is_some() && self.repos.is_empty() && cfg.repositories().is_empty() {
            return Err(RepoResolveError::NoRepositoriesForOrg);
        }

        let owner = self.owner.clone().unwrap_or_else(|| cfg.owner().to_owned());
        let project_number = cfg.project_number();
        let target = if !self.repos.is_empty() {
            Target::multi(owner, self.repos.clone(), project_number)
        } else if !cfg.repositories().is_empty() {
            Target::multi(owner, cfg.repositories().to_vec(), project_number)
        } else {
            Target::single(owner, cfg.repository_name().to_owned(), project_number)
        };

        let repos = target
            .repo_coords()
            .into_iter()
            .map(|repo| {
                let spec = self.finalize(cfg.spec_for(repo.name())?)?;
                Ok((repo, spec))
            })
            .collect::<Result<Vec<_>, RepoResolveError>>()?;

        let local_files = self.checks_local_files(&cfg).then(|| LocalFiles {
            templates: cfg.issue_templates().cloned(),
            docs: cfg.label_docs().to_vec(),
        });
        Ok(ResolvedRepo {
            target,
            repos,
            local_files,
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

/// The `repo` noun with its `check` and `sync` verbs.
pub fn command() -> Command {
    Command::new("repo")
        .about("Audits or synchronizes repository settings, labels, and milestones with tools.toml")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(entity_flags(
            Command::new(CHECK)
                .about("Audit GitHub configuration drift without modifying state")
                .after_help(LOCAL_FILES_HELP),
        ))
        .subcommand(entity_flags(
            Command::new(SYNC).about("Reconcile GitHub repository configuration with tools.toml"),
        ))
}

fn entity_flags(command: Command) -> Command {
    command
        .arg(
            Arg::new(SETTINGS)
                .long(SETTINGS)
                .action(ArgAction::SetTrue)
                .help("Limit to repository settings"),
        )
        .arg(
            Arg::new(LABELS)
                .long(LABELS)
                .action(ArgAction::SetTrue)
                .help("Limit to repository labels"),
        )
        .arg(
            Arg::new(MILESTONES)
                .long(MILESTONES)
                .action(ArgAction::SetTrue)
                .help("Limit to repository milestones"),
        )
        .arg(
            Arg::new(PROJECT)
                .long(PROJECT)
                .action(ArgAction::SetTrue)
                .help("Limit to the repository project title"),
        )
}

#[cfg(test)]
mod tests {
    fn spec_of<'a>(resolved: &'a ResolvedRepo, name: &str) -> &'a RepoSpec {
        &resolved
            .repos
            .iter()
            .find(|(repo, _)| repo.name() == name)
            .expect("target repository")
            .1
    }

    use super::*;
    use crate::cli::{Invocation, UsageError};

    fn parse(words: impl IntoIterator<Item = String>) -> Result<Arguments, UsageError> {
        let command_line = ["tools".to_owned(), "repo".to_owned()]
            .into_iter()
            .chain(words);
        match crate::cli::parse(command_line)? {
            Invocation::Repo(arguments) => Ok(arguments),
            _ => unreachable!("a repo command line parses to the repo group"),
        }
    }
    use tools_core::domain::repo::RepoProjectSpec;

    #[test]
    fn parses_flags_correctly() {
        let args = parse(["check".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Check);

        let args = parse(["sync".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Apply);

        let args = parse(["sync".into(), "--org".into(), "gatuno-manga".into()]).unwrap();
        assert_eq!(args.owner(), Some("gatuno-manga"));

        let args_repos = parse([
            "sync".into(),
            "--repo".into(),
            "web".into(),
            "--repo".into(),
            "api".into(),
        ])
        .unwrap();
        assert_eq!(args_repos.repos(), &["web", "api"]);
    }

    const LOCAL_FILES_CONFIG: &str = r#"
        [github.repository]
        owner = "jaoppb"
        name = "tools"

        [github.repository.settings]
        allow_merge_commit = true

        [github.labels]
        docs = ["AGENTS.md"]
        items = [{ name = "type:feat", color = "1d76db" }]

        [github.milestones]
        items = [{ title = "v1" }]

        [github.issue_templates]
        items = [{ file = "feat.yml", label = "type:feat", title = "feat: " }]
    "#;

    fn local_files_of(words: &[&str], toml_str: &str) -> Option<LocalFiles> {
        let config = tools_core::config::load_from_str(toml_str).unwrap();
        parse(words.iter().map(|word| (*word).to_owned()))
            .unwrap()
            .resolve_with_config(config)
            .unwrap()
            .local_files
    }

    #[test]
    fn local_files_carry_the_declared_templates_and_docs() {
        let files = local_files_of(&["check"], LOCAL_FILES_CONFIG).expect("plain check runs it");
        assert_eq!(files.docs, ["AGENTS.md"]);
        assert_eq!(files.templates.expect("declared").items[0].file, "feat.yml");

        let minimal = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"

            [github.labels]
            items = [{ name = "type:feat", color = "1d76db" }]
        "#;
        let files = local_files_of(&["check"], minimal).expect("plain check runs it");
        assert!(files.templates.is_none());
        assert!(files.docs.is_empty());
    }

    #[test]
    fn local_files_are_checked_only_for_a_labels_check_of_the_configured_repository() {
        for (words, runs) in [
            (&["check"][..], true),
            (&["check", "--labels"][..], true),
            (&["check", "--labels", "--settings"][..], true),
            (&["check", "--settings"][..], false),
            (&["check", "--milestones"][..], false),
            (&["check", "--repo", "tools"][..], false),
            (&["check", "--repo", "web"][..], false),
            (&["check", "--org", "other", "--repo", "tools"][..], false),
            (&["sync"][..], false),
            (&["sync", "--labels"][..], false),
        ] {
            assert_eq!(
                local_files_of(words, LOCAL_FILES_CONFIG).is_some(),
                runs,
                "{words:?}"
            );
        }
        let listed = format!("{LOCAL_FILES_CONFIG}\n[github]\nrepositories = [\"tools\"]\n");
        assert!(local_files_of(&["check"], &listed).is_none());
    }

    #[test]
    fn resolve_fails_loud_when_config_is_missing() {
        let args = parse([
            "sync".into(),
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

        let args = parse(["sync".into(), "--labels".into()]).unwrap();
        let err = args.resolve_with_config(config.clone()).unwrap_err();
        assert!(matches!(
            err,
            RepoResolveError::MissingEntity {
                entity: "labels",
                section: "[github.labels]"
            }
        ));

        let args_all_flags = parse([
            "sync".into(),
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

        let args_all = parse(["sync".into()]).unwrap();
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
        let args = parse(["sync".into(), "--labels".into()]).unwrap();
        let resolved = args.resolve_with_config(config.clone()).unwrap();
        assert!(spec_of(&resolved, "tools").settings.is_none());
        assert!(spec_of(&resolved, "tools").labels.is_some());
        assert!(spec_of(&resolved, "tools").milestones.is_none());

        // When no filter is passed, configured entities should be Some
        let args_all = parse(["sync".into()]).unwrap();
        let resolved_all = args_all.resolve_with_config(config).unwrap();
        assert!(spec_of(&resolved_all, "tools").settings.is_some());
        assert!(spec_of(&resolved_all, "tools").labels.is_some());
        assert!(spec_of(&resolved_all, "tools").milestones.is_none());
        assert!(spec_of(&resolved_all, "tools").project.is_none());
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

        let args = parse(["sync".into(), "--project".into()]).unwrap();
        let resolved = args.resolve_with_config(config.clone()).unwrap();
        assert!(spec_of(&resolved, "tools").settings.is_none());
        assert!(spec_of(&resolved, "tools").labels.is_none());
        assert!(spec_of(&resolved, "tools").milestones.is_none());
        assert_eq!(
            spec_of(&resolved, "tools").project,
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

        let args = parse(["sync".into()]).unwrap();
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

        let args = parse([
            "sync".into(),
            "--repo".into(),
            "web".into(),
            "--repo".into(),
            "api".into(),
        ])
        .unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        assert_eq!(
            spec_of(&resolved, "web")
                .labels
                .as_ref()
                .unwrap()
                .items
                .len(),
            2
        );
        assert_eq!(
            spec_of(&resolved, "api")
                .labels
                .as_ref()
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn resolve_accepts_config_defined_only_by_overlays() {
        let toml_str = r#"
            [github.repository]
            owner = "gatuno-manga"
            name = "web"

            [github.overrides.web.labels]
            items = [{ name = "area:ssr", color = "2ea44f", description = "SSR" }]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let args = parse(["sync".into(), "--labels".into()]).unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        assert_eq!(
            spec_of(&resolved, "web")
                .labels
                .as_ref()
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn resolve_rejects_duplicate_items_introduced_by_overlay() {
        let toml_str = r#"
            [github.repository]
            owner = "gatuno-manga"
            name = "web"

            [github.overrides.web.labels]
            items = [
                { name = "a", color = "111111", description = "" },
                { name = "a", color = "222222", description = "" },
            ]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let args = parse(["sync".into()]).unwrap();
        let err = args.resolve_with_config(config).unwrap_err();
        assert!(err.to_string().contains("duplicate item"));
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
        let args = parse(["sync".into()]).unwrap();
        let resolved = args.resolve_with_config(config).unwrap();
        assert_eq!(
            spec_of(&resolved, "web")
                .labels
                .as_ref()
                .unwrap()
                .items
                .len(),
            1
        );
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

        let args = parse([
            "sync".into(),
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

        let args = parse(["sync".into(), "--repo".into(), "web".into()]).unwrap();
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

        let args = parse([
            "sync".into(),
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

        let args = parse(["sync".into(), "--org".into(), "gatuno-manga".into()]).unwrap();
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

        let args = parse(["sync".into(), "--org".into(), "gatuno-manga".into()]).unwrap();
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

        let args = parse(["sync".into()]).unwrap();
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
