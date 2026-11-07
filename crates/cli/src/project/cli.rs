//! The command line of `tools project check|sync`, parsed with clap.

use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

use tools_core::adapters::github::Target;
use tools_core::application::Mode;
use tools_core::config::{ConfigError, load_or_find};
use tools_core::workflow::{LoadError, Workflow};

use crate::cli::{CONFIG, OWNER, PROJECT, project_flag, repos_of};

/// The verb that prints the planned moves and writes nothing.
pub const CHECK: &str = "check";
/// The verb that writes the planned moves.
pub const SYNC: &str = "sync";
/// `--explain <ISSUE>`: print the trace of one issue instead of the run report.
pub const EXPLAIN: &str = "explain";

/// An error resolving project synchronization configuration.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("invalid [workflow] block: {0}")]
    Workflow(#[from] LoadError),
    #[error(
        "no [workflow] block in tools.toml: add `[workflow]` with `extends = \"builtin:cosmo\"` or `\"builtin:tools\"`, or declare your own statuses and rules"
    )]
    MissingWorkflow,
    #[error("owner is required (specify via --owner or in tools.toml)")]
    MissingOwner,
    #[error("repository is required (specify via --repo or in tools.toml)")]
    MissingRepository,
    #[error("project number is required (specify via --project or in tools.toml)")]
    MissingProject,
}

/// The parsed command line of `tools project check|sync`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    mode: Mode,
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repos: Vec<String>,
    project: Option<u32>,
    explain: Option<String>,
}

impl Arguments {
    /// Reads the arguments of the `check` or `sync` verb; the global flags travel with `matches`.
    #[must_use]
    pub fn from_matches(verb: &str, matches: &ArgMatches) -> Self {
        let repos = repos_of(matches);
        Self {
            mode: if verb == CHECK {
                Mode::Preview
            } else {
                Mode::Apply
            },
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            owner: matches.get_one::<String>(OWNER).cloned(),
            repos,
            project: matches
                .get_one::<String>(PROJECT)
                .and_then(|p| p.parse().ok()),
            explain: matches
                .try_get_one::<String>(EXPLAIN)
                .ok()
                .flatten()
                .cloned(),
        }
    }

    /// Whether the run writes its moves.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// The issue `--explain` names, as typed: `57` or `repo#57`.
    #[must_use]
    pub fn explain(&self) -> Option<&str> {
        self.explain.as_deref()
    }

    /// Resolves target and workflow from tools.toml and CLI overrides.
    ///
    /// # Errors
    ///
    /// Fails when configuration cannot be found or read, when the target lacks
    /// owner/repo/project, or when there is no valid `[workflow]` block.
    pub fn resolve(&self) -> Result<(Target, Workflow), ResolveError> {
        let loaded = load_or_find(self.config_path.as_deref());
        let (cfg_target, workflow) = match loaded {
            Ok(cfg) => (Some(cfg.target()), cfg.workflow()?),
            Err(ConfigError::NotFound) => (None, None),
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
                .or_else(|| cfg_target.as_ref().and_then(Target::project))
                .ok_or(ResolveError::MissingProject)?;
            Target::multi(owner, self.repos.clone(), Some(project))
        } else if let Some(mut target) = cfg_target {
            if let Some(owner) = self.owner.clone() {
                target = target.with_owner(owner);
            }
            if let Some(proj) = self.project {
                target = target.with_project(Some(proj));
            }
            if target.project().is_none() {
                return Err(ResolveError::MissingProject);
            }
            target
        } else {
            self.owner.as_ref().ok_or(ResolveError::MissingOwner)?;
            self.project.ok_or(ResolveError::MissingProject)?;
            return Err(ResolveError::MissingRepository);
        };

        let workflow = workflow.ok_or(ResolveError::MissingWorkflow)?;
        Ok((target, workflow))
    }
}

/// The `project` noun with its `check` and `sync` verbs.
pub fn command() -> Command {
    Command::new("project")
        .about("Checks or moves the Status of GitHub Project cards according to workflow rules")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(CHECK)
                .about(
                    "Print the planned moves and frozen cards and write nothing; exits 1 when there are any",
                )
                .arg(project_flag())
                .arg(
                    Arg::new(EXPLAIN)
                        .long(EXPLAIN)
                        .value_name("ISSUE")
                        .help("Print the facts and rules of one issue (`57` or `repo#57`) and exit"),
                ),
        )
        .subcommand(
            Command::new(SYNC)
                .about("Write the planned moves to the Project")
                .arg(project_flag()),
        )
}
