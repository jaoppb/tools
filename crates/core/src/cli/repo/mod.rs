//! The command line for `repo-sync`, parsed with clap.

use clap::{Arg, ArgAction, Command};
use std::path::PathBuf;

use crate::adapters::github::target::Target;
use crate::application::repo::RepoSyncMode;
use crate::config::{ConfigError, load_or_find};
use crate::domain::repo::RepoSpec;

const PROGRAM: &str = "repo-sync";
const CHECK: &str = "check";
const DRY_RUN: &str = "dry-run";
const SETTINGS: &str = "settings";
const LABELS: &str = "labels";
const MILESTONES: &str = "milestones";
const CONFIG: &str = "config";
const OWNER: &str = "owner";
const REPO: &str = "repo";

/// Internal entity filter from command-line flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EntityFilter {
    settings: bool,
    labels: bool,
    milestones: bool,
}

/// An error resolving repository configuration from tools.toml and CLI arguments.
#[derive(Debug, thiserror::Error)]
pub enum RepoResolveError {
    /// Failed to find or read configuration.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// No repository settings, labels, or milestones configured in tools.toml.
    #[error("no repository settings, labels, or milestones configured in tools.toml")]
    EmptyConfig,
    /// Requested entity not configured in tools.toml.
    #[error(
        "repository {entity} requested via --{entity}, but not configured in tools.toml under {section}"
    )]
    MissingEntity {
        /// The entity name (e.g. `settings`, `labels`, `milestones`).
        entity: &'static str,
        /// The tools.toml section (e.g. `[github.labels]`).
        section: &'static str,
    },
}

/// A command line that was not run: an argument clap rejected, or `--help`.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RepoUsageError(#[from] clap::Error);

impl RepoUsageError {
    /// The exit code clap gives it: 0 for `--help` and 2 for a rejected argument.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        u8::try_from(self.0.exit_code()).unwrap_or(2)
    }
}

/// The parsed command line for `repo-sync`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoArguments {
    mode: RepoSyncMode,
    explicit_entities: Option<EntityFilter>,
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repo: Option<String>,
}

/// Resolved repository target and desired configuration specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepo {
    /// Repository target.
    pub target: Target,
    /// Desired configuration specification.
    pub spec: RepoSpec,
}

impl RepoArguments {
    /// Parses the arguments.
    ///
    /// # Errors
    ///
    /// Returns [`RepoUsageError`] on invalid syntax or `--help`.
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, RepoUsageError> {
        let matches = command().try_get_matches_from(arguments)?;
        let mode = if matches.get_flag(CHECK) {
            RepoSyncMode::Check
        } else if matches.get_flag(DRY_RUN) {
            RepoSyncMode::Preview
        } else {
            RepoSyncMode::Apply
        };

        let has_filter =
            matches.get_flag(SETTINGS) || matches.get_flag(LABELS) || matches.get_flag(MILESTONES);

        let explicit_entities = if has_filter {
            Some(EntityFilter {
                settings: matches.get_flag(SETTINGS),
                labels: matches.get_flag(LABELS),
                milestones: matches.get_flag(MILESTONES),
            })
        } else {
            None
        };

        Ok(Self {
            mode,
            explicit_entities,
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            owner: matches.get_one::<String>(OWNER).cloned(),
            repo: matches.get_one::<String>(REPO).cloned(),
        })
    }

    /// Execution mode.
    #[must_use]
    pub const fn mode(&self) -> RepoSyncMode {
        self.mode
    }

    /// Resolves target and all configured resources from `tools.toml` and CLI flags.
    ///
    /// # Errors
    ///
    /// Fails when configuration cannot be found or read, or when a requested entity is not configured.
    pub fn resolve(&self) -> Result<ResolvedRepo, RepoResolveError> {
        let cfg = load_or_find(self.config_path.as_deref())?;
        let all_settings = cfg.repo_settings().cloned();
        let all_labels = cfg.labels().cloned();
        let all_milestones = cfg.milestones().cloned();

        let (settings, labels, milestones) = if let Some(entities) = self.explicit_entities {
            (
                select_entity(
                    entities.settings,
                    all_settings,
                    "settings",
                    "[github.repository.settings]",
                )?,
                select_entity(entities.labels, all_labels, "labels", "[github.labels]")?,
                select_entity(
                    entities.milestones,
                    all_milestones,
                    "milestones",
                    "[github.milestones]",
                )?,
            )
        } else {
            if all_settings.is_none() && all_labels.is_none() && all_milestones.is_none() {
                return Err(RepoResolveError::EmptyConfig);
            }
            (all_settings, all_labels, all_milestones)
        };

        let owner = self.owner.clone().unwrap_or(cfg.github.repository.owner);
        let repo = self.repo.clone().unwrap_or(cfg.github.repository.name);

        let target = Target::new(owner, repo, None);
        let spec = RepoSpec::new(settings, labels, milestones);
        Ok(ResolvedRepo { target, spec })
    }
}

fn select_entity<T>(
    flag: bool,
    opt: Option<T>,
    entity: &'static str,
    section: &'static str,
) -> Result<Option<T>, RepoResolveError> {
    match (flag, opt) {
        (true, None) => Err(RepoResolveError::MissingEntity { entity, section }),
        (true, Some(val)) => Ok(Some(val)),
        (false, _) => Ok(None),
    }
}

fn command() -> Command {
    Command::new(PROGRAM)
        .about("Synchronizes and audits GitHub repository settings, labels, and milestones against tools.toml")
        .arg(
            Arg::new(CHECK)
                .long(CHECK)
                .action(ArgAction::SetTrue)
                .help("Audit repository state and exit with non-zero code if drift is detected"),
        )
        .arg(
            Arg::new(DRY_RUN)
                .long(DRY_RUN)
                .action(ArgAction::SetTrue)
                .conflicts_with(CHECK)
                .help("Print differences without applying changes to GitHub"),
        )
        .arg(
            Arg::new(SETTINGS)
                .long(SETTINGS)
                .action(ArgAction::SetTrue)
                .help("Reconcile repository merge and branch settings"),
        )
        .arg(
            Arg::new(LABELS)
                .long(LABELS)
                .action(ArgAction::SetTrue)
                .help("Reconcile repository labels"),
        )
        .arg(
            Arg::new(MILESTONES)
                .long(MILESTONES)
                .action(ArgAction::SetTrue)
                .help("Reconcile repository milestones"),
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
                .value_name("OWNER")
                .help("GitHub repository owner override"),
        )
        .arg(
            Arg::new(REPO)
                .long(REPO)
                .value_name("REPO")
                .help("GitHub repository name override"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags_correctly() {
        let args = RepoArguments::parse(["repo-sync".into(), "--check".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Check);

        let args = RepoArguments::parse(["repo-sync".into(), "--dry-run".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Preview);

        let args = RepoArguments::parse(["repo-sync".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Apply);
    }

    #[test]
    fn resolve_fails_loud_when_config_is_missing() {
        let args = RepoArguments::parse([
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
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("test-tools-no-labels.toml");
        let content = r#"
            [github.repository]
            owner = "jaoppb"
            name = "tools"
        "#;
        std::fs::File::create(&path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let args = RepoArguments::parse([
            "repo-sync".into(),
            "--labels".into(),
            "--config".into(),
            path.to_str().unwrap().into(),
        ])
        .unwrap();

        let err = args.resolve().unwrap_err();
        assert!(matches!(
            err,
            RepoResolveError::MissingEntity {
                entity: "labels",
                section: "[github.labels]"
            }
        ));

        let args_all_flags = RepoArguments::parse([
            "repo-sync".into(),
            "--settings".into(),
            "--labels".into(),
            "--milestones".into(),
            "--config".into(),
            path.to_str().unwrap().into(),
        ])
        .unwrap();

        let err_all_flags = args_all_flags.resolve().unwrap_err();
        assert!(matches!(
            err_all_flags,
            RepoResolveError::MissingEntity {
                entity: "settings",
                section: "[github.repository.settings]"
            }
        ));

        let args_all = RepoArguments::parse([
            "repo-sync".into(),
            "--config".into(),
            path.to_str().unwrap().into(),
        ])
        .unwrap();

        let err_all = args_all.resolve().unwrap_err();
        assert!(matches!(err_all, RepoResolveError::EmptyConfig));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolve_filters_selected_entities() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("test-tools-filter.toml");
        let content = r#"
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
        std::fs::File::create(&path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        // When --labels is passed, settings should be None
        let args = RepoArguments::parse([
            "repo-sync".into(),
            "--labels".into(),
            "--config".into(),
            path.to_str().unwrap().into(),
        ])
        .unwrap();
        let resolved = args.resolve().unwrap();
        assert!(resolved.spec.settings.is_none());
        assert!(resolved.spec.labels.is_some());
        assert!(resolved.spec.milestones.is_none());

        // When no filter is passed, configured entities should be Some
        let args_all = RepoArguments::parse([
            "repo-sync".into(),
            "--config".into(),
            path.to_str().unwrap().into(),
        ])
        .unwrap();
        let resolved_all = args_all.resolve().unwrap();
        assert!(resolved_all.spec.settings.is_some());
        assert!(resolved_all.spec.labels.is_some());
        assert!(resolved_all.spec.milestones.is_none());

        let _ = std::fs::remove_file(path);
    }
}
