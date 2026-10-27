//! The command line for `repo-sync`, parsed with clap.

use clap::{Arg, ArgAction, Command};
use std::path::PathBuf;

use crate::adapters::github::target::Target;
use crate::application::repo::RepoSyncMode;
use crate::config::{LabelsConfig, MilestonesConfig, load_or_find};
use crate::domain::repo::{RepoEntities, RepoSettings};

const PROGRAM: &str = "repo-sync";
const CHECK: &str = "check";
const DRY_RUN: &str = "dry-run";
const SETTINGS: &str = "settings";
const LABELS: &str = "labels";
const MILESTONES: &str = "milestones";
const CONFIG: &str = "config";
const OWNER: &str = "owner";
const REPO: &str = "repo";

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
    entities: RepoEntities,
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repo: Option<String>,
}

/// Resolved repository target and desired configurations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepo {
    /// Repository target.
    pub target: Target,
    /// Desired repository settings, if configured and selected.
    pub settings: Option<RepoSettings>,
    /// Desired labels configuration, if configured and selected.
    pub labels: Option<LabelsConfig>,
    /// Desired milestones configuration, if configured and selected.
    pub milestones: Option<MilestonesConfig>,
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

        let has_filter = matches.get_flag(SETTINGS)
            || matches.get_flag(LABELS)
            || matches.get_flag(MILESTONES);

        let entities = if has_filter {
            RepoEntities {
                settings: matches.get_flag(SETTINGS),
                labels: matches.get_flag(LABELS),
                milestones: matches.get_flag(MILESTONES),
            }
        } else {
            RepoEntities::ALL
        };

        Ok(Self {
            mode,
            entities,
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

    /// Selected entities.
    #[must_use]
    pub const fn entities(&self) -> RepoEntities {
        self.entities
    }

    /// Resolves target and all configured resources from `tools.toml` and CLI flags.
    ///
    /// # Errors
    ///
    /// Fails when configuration cannot be found or read, or when a requested entity is not configured.
    pub fn resolve(&self) -> Result<ResolvedRepo, String> {
        let cfg = load_or_find(self.config_path.as_deref()).map_err(|err| err.to_string())?;
        let all_settings = cfg.repo_settings().map(RepoSettings::from);
        let all_labels = cfg.labels().cloned();
        let all_milestones = cfg.milestones().cloned();

        let has_filter = self.entities != RepoEntities::ALL;

        if has_filter {
            if self.entities.settings && all_settings.is_none() {
                return Err("repository settings requested via --settings, but not configured in tools.toml under [github.repository.settings]".into());
            }
            if self.entities.labels && all_labels.is_none() {
                return Err("repository labels requested via --labels, but not configured in tools.toml under [github.labels]".into());
            }
            if self.entities.milestones && all_milestones.is_none() {
                return Err("repository milestones requested via --milestones, but not configured in tools.toml under [github.milestones]".into());
            }
        } else if all_settings.is_none() && all_labels.is_none() && all_milestones.is_none() {
            return Err("no repository settings, labels, or milestones configured in tools.toml".into());
        }

        let settings = if self.entities.settings { all_settings } else { None };
        let labels = if self.entities.labels { all_labels } else { None };
        let milestones = if self.entities.milestones { all_milestones } else { None };

        let owner = self
            .owner
            .clone()
            .unwrap_or(cfg.github.repository.owner);
        let repo = self
            .repo
            .clone()
            .unwrap_or(cfg.github.repository.name);

        let target = Target::new(owner, repo, None);
        Ok(ResolvedRepo {
            target,
            settings,
            labels,
            milestones,
        })
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
        assert_eq!(args.entities(), RepoEntities::ALL);

        let args = RepoArguments::parse(["repo-sync".into(), "--dry-run".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Preview);
        assert_eq!(args.entities(), RepoEntities::ALL);

        let args = RepoArguments::parse(["repo-sync".into()]).unwrap();
        assert_eq!(args.mode(), RepoSyncMode::Apply);
        assert_eq!(args.entities(), RepoEntities::ALL);

        let args = RepoArguments::parse(["repo-sync".into(), "--labels".into()]).unwrap();
        assert_eq!(
            args.entities(),
            RepoEntities {
                settings: false,
                labels: true,
                milestones: false,
            }
        );

        let args = RepoArguments::parse([
            "repo-sync".into(),
            "--settings".into(),
            "--milestones".into(),
        ])
        .unwrap();
        assert_eq!(
            args.entities(),
            RepoEntities {
                settings: true,
                labels: false,
                milestones: true,
            }
        );
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
        assert!(err.contains("failed to read config file") || err.contains("not found"));
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
        assert!(err.contains("repository labels requested via --labels, but not configured"));

        let args_all = RepoArguments::parse([
            "repo-sync".into(),
            "--config".into(),
            path.to_str().unwrap().into(),
        ])
        .unwrap();

        let err_all = args_all.resolve().unwrap_err();
        assert!(err_all.contains("no repository settings, labels, or milestones configured"));

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
        assert!(resolved.settings.is_none());
        assert!(resolved.labels.is_some());
        assert!(resolved.milestones.is_none());

        // When no filter is passed, configured entities should be Some
        let args_all = RepoArguments::parse([
            "repo-sync".into(),
            "--config".into(),
            path.to_str().unwrap().into(),
        ])
        .unwrap();
        let resolved_all = args_all.resolve().unwrap();
        assert!(resolved_all.settings.is_some());
        assert!(resolved_all.labels.is_some());
        assert!(resolved_all.milestones.is_none());

        let _ = std::fs::remove_file(path);
    }
}
