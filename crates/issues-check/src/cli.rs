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

/// An error resolving issue audit configuration.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("owner is required (specify via --owner or in tools.toml)")]
    MissingOwner,
    #[error("repository is required (specify via --repo or in tools.toml)")]
    MissingRepository,
}

/// The parsed arguments for `issues-check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repos: Vec<String>,
    project: Option<u32>,
    no_parent: bool,
    no_membership: bool,
}

impl Arguments {
    /// Reads the whole command line, program name first.
    ///
    /// # Errors
    ///
    /// Returns [`UsageError`] for an argument the tool does not know, and for
    /// `--help`, which asks for the usage and is not a failure.
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, UsageError> {
        let matches = command().try_get_matches_from(arguments)?;
        let repos: Vec<String> = matches
            .get_many::<String>(REPO)
            .map(|values| values.cloned().collect())
            .unwrap_or_default();
        Ok(Self {
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            owner: matches.get_one::<String>(OWNER).cloned(),
            repos,
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
    pub fn resolve(&self) -> Result<(Target, RulesConfig), ResolveError> {
        let loaded = load_or_find(self.config_path.as_deref());
        let (cfg_target, mut rules) = match loaded {
            Ok(cfg) => {
                let target = cfg.target();
                (Some(target), cfg.github.issues.rules)
            }
            Err(ConfigError::NotFound) => (None, RulesConfig::default()),
            Err(err) => return Err(ResolveError::Config(err)),
        };

        let target = if !self.repos.is_empty() {
            let owner = self
                .owner
                .clone()
                .or_else(|| cfg_target.as_ref().map(|t| t.owner().to_owned()))
                .ok_or(ResolveError::MissingOwner)?;
            let project = self
                .project
                .or_else(|| cfg_target.as_ref().and_then(Target::project));
            Target::multi(owner, self.repos.clone(), project)
        } else if let Some(mut target) = cfg_target {
            if let Some(owner) = self.owner.clone() {
                target = target.with_owner(owner);
            }
            if let Some(proj) = self.project {
                target = target.with_project(Some(proj));
            }
            target
        } else {
            self.owner.as_ref().ok_or(ResolveError::MissingOwner)?;
            return Err(ResolveError::MissingRepository);
        };

        if self.no_parent {
            rules.parent = false;
        }
        if self.no_membership || target.project().is_none() {
            rules.membership = false;
        }

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
                .action(ArgAction::Append)
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
