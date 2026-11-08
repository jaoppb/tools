//! The command line of `tools issues check|migrate`.

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::collections::BTreeMap;
use std::path::PathBuf;

use tools_core::adapters::github::Target;
use tools_core::config::{ConfigError, RulesConfig, load_or_find};

use crate::cli::{CONFIG, OWNER, PROJECT, project_flag, repos_of};

/// The verb that audits issues and previews the label migrations without writing.
pub const CHECK: &str = "check";
/// The verb that rewrites issue labels to the configured taxonomy.
pub const MIGRATE: &str = "migrate";
const NO_PARENT: &str = "no-parent";
const NO_MEMBERSHIP: &str = "no-membership";

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

/// The parsed arguments of `tools issues check|migrate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    config_path: Option<PathBuf>,
    owner: Option<String>,
    repos: Vec<String>,
    project: Option<u32>,
    no_parent: bool,
    no_membership: bool,
    migrate: bool,
}

/// Resolved target, audit rules, and label migrations from configuration and CLI overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCheck {
    /// Target repository coordinates.
    pub target: Target,
    /// Issue audit rules.
    pub rules: RulesConfig,
    /// Resolved legacy-to-canonical label migrations.
    pub migrations: BTreeMap<String, String>,
    /// Deprecations of the configuration file, to print after the report.
    pub warnings: Vec<String>,
}

impl Arguments {
    /// Reads the arguments of the `check` or `migrate` verb; the global flags travel with `matches`.
    #[must_use]
    pub fn from_matches(verb: &str, matches: &ArgMatches) -> Self {
        let repos = repos_of(matches);
        let flag =
            |name: &str| matches.try_get_one::<bool>(name).ok().flatten().copied() == Some(true);
        Self {
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            owner: matches.get_one::<String>(OWNER).cloned(),
            repos,
            project: matches
                .get_one::<String>(PROJECT)
                .and_then(|p| p.parse().ok()),
            no_parent: flag(NO_PARENT),
            no_membership: flag(NO_MEMBERSHIP),
            migrate: verb == MIGRATE,
        }
    }

    /// Whether the verb is `migrate`, which writes, rather than `check`.
    #[must_use]
    pub const fn is_migrate(&self) -> bool {
        self.migrate
    }

    /// Resolves the target, rules, and migrations from configuration and CLI overrides.
    ///
    /// # Errors
    ///
    /// Fails when configuration cannot be read or target lacks owner/repo.
    pub fn resolve(&self) -> Result<ResolvedCheck, ResolveError> {
        let loaded = load_or_find(self.config_path.as_deref());
        let (cfg_target, mut rules, migrations, warnings) = match loaded {
            Ok(cfg) => {
                let target = cfg.target();
                let migrations = cfg
                    .github
                    .labels
                    .as_ref()
                    .map(|labels| labels.migrations.clone())
                    .unwrap_or_default();
                (
                    Some(target),
                    cfg.github.issues.rules,
                    migrations,
                    cfg.warnings,
                )
            }
            Err(ConfigError::NotFound) => {
                (None, RulesConfig::default(), BTreeMap::new(), Vec::new())
            }
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

        Ok(ResolvedCheck {
            target,
            rules,
            migrations,
            warnings,
        })
    }
}

/// The `issues` noun with its `check` and `migrate` verbs.
pub fn command() -> Command {
    Command::new("issues")
        .about("Audits and migrates the structure and labels of repository issues")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(CHECK)
                .about("Audit issues against the workflow rules and preview label migrations")
                .arg(project_flag())
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
                ),
        )
        .subcommand(
            Command::new(MIGRATE)
                .about("Rewrite issue labels to match the configured taxonomy")
                .arg(project_flag()),
        )
}
