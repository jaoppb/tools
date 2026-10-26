//! The command line: `project-sync [--dry-run]`, parsed with clap.

pub mod audit;
pub mod repo;

pub use crate::cli::repo::{RepoArguments, RepoUsageError};

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::path::PathBuf;

use crate::adapters::github::target::Target;
use crate::application::Mode;
use crate::config::{ConfigError, load_or_find};

const PROGRAM: &str = "project-sync";
const DRY_RUN: &str = "dry-run";
const CONFIG: &str = "config";
const OWNER: &str = "owner";
const REPO: &str = "repo";
const PROJECT: &str = "project";

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

/// The parsed command line for `project-sync`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    mode: Mode,
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repo: Option<String>,
    project: Option<u32>,
}

impl Arguments {
    /// Parses the arguments.
    ///
    /// # Errors
    ///
    /// Returns [`UsageError`] on invalid syntax or `--help`.
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, UsageError> {
        let matches = command().try_get_matches_from(arguments)?;
        Ok(Self {
            mode: mode_of(&matches),
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            owner: matches.get_one::<String>(OWNER).cloned(),
            repo: matches.get_one::<String>(REPO).cloned(),
            project: matches
                .get_one::<String>(PROJECT)
                .and_then(|p| p.parse().ok()),
        })
    }

    /// Whether the run writes its moves.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// Resolves the target from configuration and CLI overrides.
    ///
    /// # Errors
    ///
    /// Fails when configuration cannot be found or read, or target lacks owner/repo/project.
    pub fn resolve_target(&self) -> Result<Target, String> {
        let loaded = load_or_find(self.config_path.as_deref());
        let (cfg_owner, cfg_repo, cfg_proj) = match loaded {
            Ok(cfg) => (
                Some(cfg.github.repository.owner),
                Some(cfg.github.repository.name),
                cfg.github.project.and_then(|p| p.number),
            ),
            Err(ConfigError::NotFound) => (None, None, None),
            Err(err) => return Err(err.to_string()),
        };

        let owner = self.owner.clone().or(cfg_owner).ok_or_else(|| {
            "owner is required (specify via --owner or in tools.toml)".to_string()
        })?;
        let repo = self.repo.clone().or(cfg_repo).ok_or_else(|| {
            "repository is required (specify via --repo or in tools.toml)".to_string()
        })?;
        let project = self.project.or(cfg_proj).ok_or_else(|| {
            "project number is required (specify via --project or in tools.toml)".to_string()
        })?;

        Ok(Target::new(owner, repo, Some(project)))
    }
}

fn command() -> Command {
    Command::new(PROGRAM)
        .about("Moves the Status of GitHub Project cards according to workflow rules")
        .arg(
            Arg::new(DRY_RUN)
                .long(DRY_RUN)
                .action(ArgAction::SetTrue)
                .help("Print the moves and write nothing"),
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
        .arg(
            Arg::new(PROJECT)
                .long(PROJECT)
                .value_name("NUMBER")
                .help("GitHub Project number override"),
        )
}

fn mode_of(matches: &ArgMatches) -> Mode {
    if matches.get_flag(DRY_RUN) {
        return Mode::Preview;
    }
    Mode::Apply
}
