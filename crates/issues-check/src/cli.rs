//! The command line of `issues-check`.

use clap::{Arg, ArgAction, Command};
use std::path::PathBuf;

use tools_core::adapters::github::Target;
use tools_core::config::{ConfigError, RulesConfig, load_or_find};

const PROGRAM: &str = "issues-check";
const CONFIG: &str = "config";
const OWNER: &str = "owner";
const REPO: &str = "repo";
const PROJECT: &str = "project";
const NO_PARENT: &str = "no-parent";
const NO_MEMBERSHIP: &str = "no-membership";

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

/// The parsed arguments for `issues-check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repo: Option<String>,
    project: Option<u32>,
    no_parent: bool,
    no_membership: bool,
}

/// Backward compatibility alias.
pub type AuditArguments = Arguments;

impl Arguments {
    /// Reads the whole command line, program name first.
    ///
    /// # Errors
    ///
    /// Returns [`UsageError`] for an argument the tool does not know, and for
    /// `--help`, which asks for the usage and is not a failure.
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, UsageError> {
        let matches = command().try_get_matches_from(arguments)?;
        Ok(Self {
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            owner: matches.get_one::<String>(OWNER).cloned(),
            repo: matches.get_one::<String>(REPO).cloned(),
            project: matches
                .get_one::<String>(PROJECT)
                .and_then(|p| p.parse().ok()),
            no_parent: matches.get_flag(NO_PARENT),
            no_membership: matches.get_flag(NO_MEMBERSHIP),
        })
    }

    /// Resolves the target and rules from configuration and CLI overrides.
    ///
    /// # Errors
    ///
    /// Fails when configuration cannot be read or target lacks owner/repo.
    pub fn resolve(&self) -> Result<(Target, RulesConfig), String> {
        let loaded = load_or_find(self.config_path.as_deref());
        let (cfg_owner, cfg_repo, cfg_proj, mut rules) = match loaded {
            Ok(cfg) => (
                Some(cfg.github.repository.owner),
                Some(cfg.github.repository.name),
                cfg.github.project.and_then(|p| p.number),
                cfg.github.issues.rules,
            ),
            Err(ConfigError::NotFound) => (None, None, None, RulesConfig::default()),
            Err(err) => return Err(err.to_string()),
        };

        let owner = self.owner.clone().or(cfg_owner).ok_or_else(|| {
            "owner is required (specify via --owner or in tools.toml)".to_string()
        })?;
        let repo = self.repo.clone().or(cfg_repo).ok_or_else(|| {
            "repository is required (specify via --repo or in tools.toml)".to_string()
        })?;
        let project = self.project.or(cfg_proj);

        if self.no_parent {
            rules.parent = false;
        }
        if self.no_membership || project.is_none() {
            rules.membership = false;
        }

        let target = Target::new(owner, repo, project);
        Ok((target, rules))
    }
}

fn command() -> Command {
    Command::new(PROGRAM)
        .about("Audits the structure and text of repository issues and writes nothing")
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
        .arg(
            Arg::new(NO_PARENT)
                .long(NO_PARENT)
                .action(ArgAction::SetTrue)
                .help("Disable parent requirement for open issues"),
        )
        .arg(
            Arg::new(NO_MEMBERSHIP)
                .long(NO_MEMBERSHIP)
                .action(ArgAction::SetTrue)
                .help("Disable Project membership audit"),
        )
}
