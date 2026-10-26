//! The command line for `repo-sync`, parsed with clap.

use clap::{Arg, ArgAction, Command};
use std::path::PathBuf;

use crate::adapters::github::repo::RepoEntities;
use crate::adapters::github::target::Target;
use crate::application::repo::RepoSyncMode;
use crate::config::{ConfigError, LabelsConfig, MilestonesConfig, load_or_find};
use crate::domain::repo::RepoSettings;

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
    /// Desired repository settings, if configured.
    pub settings: Option<RepoSettings>,
    /// Desired labels configuration, if configured.
    pub labels: Option<LabelsConfig>,
    /// Desired milestones configuration, if configured.
    pub milestones: Option<MilestonesConfig>,
    /// Active entity selection.
    pub entities: RepoEntities,
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
    /// Fails when configuration cannot be found or required owner/repo coordinates are missing.
    pub fn resolve_all(&self) -> Result<ResolvedRepo, String> {
        let loaded = load_or_find(self.config_path.as_deref());
        let (cfg_owner, cfg_repo, cfg_settings, cfg_labels, cfg_milestones) = match loaded {
            Ok(cfg) => {
                let s = cfg.repo_settings().map(RepoSettings::from);
                let l = cfg.labels().cloned();
                let m = cfg.milestones().cloned();
                (
                    Some(cfg.github.repository.owner),
                    Some(cfg.github.repository.name),
                    s,
                    l,
                    m,
                )
            }
            Err(ConfigError::NotFound) => (None, None, None, None, None),
            Err(err) => return Err(err.to_string()),
        };

        let owner = self.owner.clone().or(cfg_owner).ok_or_else(|| {
            "owner is required (specify via --owner or in tools.toml)".to_string()
        })?;
        let repo = self.repo.clone().or(cfg_repo).ok_or_else(|| {
            "repository is required (specify via --repo or in tools.toml)".to_string()
        })?;

        let target = Target::new(owner, repo, None);
        Ok(ResolvedRepo {
            target,
            settings: cfg_settings,
            labels: cfg_labels,
            milestones: cfg_milestones,
            entities: self.entities,
        })
    }

    /// Resolves the target and desired settings for backwards compatibility.
    ///
    /// # Errors
    ///
    /// Fails when settings are missing.
    pub fn resolve(&self) -> Result<(Target, RepoSettings), String> {
        let resolved = self.resolve_all()?;
        let s = resolved.settings.ok_or_else(|| {
            "repository settings not configured in tools.toml under [github.repository.settings]"
                .to_string()
        })?;
        Ok((resolved.target, s))
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
}
