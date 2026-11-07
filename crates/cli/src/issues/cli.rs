//! The command line of `tools issues check|migrate`.

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::collections::BTreeMap;
use std::path::PathBuf;

use tools_core::adapters::github::Target;
use tools_core::config::{ConfigError, RepoLabelsSpec, RulesConfig, load_or_find};
use tools_core::domain::{CANONICAL_TYPES, canonical_type_label};

use crate::cli::{CONFIG, OWNER, PROJECT, project_flag, repos_of};

/// The verb that audits issues and previews the label migrations without writing.
pub const CHECK: &str = "check";
/// The verb that rewrites issue labels to the configured taxonomy.
pub const MIGRATE: &str = "migrate";
const NO_PARENT: &str = "no-parent";
const NO_MEMBERSHIP: &str = "no-membership";
const DEFAULT_AREA: &str = "default-area";
const DEFAULT_TYPE: &str = "default-type";

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
    default_area: Option<String>,
    default_type: Option<String>,
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
            default_area: matches
                .try_get_one::<String>(DEFAULT_AREA)
                .ok()
                .flatten()
                .cloned(),
            default_type: matches
                .try_get_one::<String>(DEFAULT_TYPE)
                .ok()
                .flatten()
                .cloned(),
        }
    }

    /// Whether the verb is `migrate`, which writes, rather than `check`.
    #[must_use]
    pub const fn is_migrate(&self) -> bool {
        self.migrate
    }

    /// Default area label to assign if an issue has none.
    #[must_use]
    pub fn default_area(&self) -> Option<&str> {
        self.default_area.as_deref()
    }

    /// Default type label to assign if an issue has none.
    #[must_use]
    pub fn default_type(&self) -> Option<&str> {
        self.default_type.as_deref()
    }

    /// Resolves the target, rules, and migrations from configuration and CLI overrides.
    ///
    /// # Errors
    ///
    /// Fails when configuration cannot be read or target lacks owner/repo.
    pub fn resolve(&self) -> Result<ResolvedCheck, ResolveError> {
        let loaded = load_or_find(self.config_path.as_deref());
        let (cfg_target, mut rules, migrations) = match loaded {
            Ok(cfg) => {
                let target = cfg.target();
                let migrations = cfg.github.labels.as_ref().map_or_else(
                    RepoLabelsSpec::default_migrations,
                    RepoLabelsSpec::resolved_migrations,
                );
                (Some(target), cfg.github.issues.rules, migrations)
            }
            Err(ConfigError::NotFound) => (
                None,
                RulesConfig::default(),
                RepoLabelsSpec::default_migrations(),
            ),
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
                .arg(project_flag())
                .arg(
                    Arg::new(DEFAULT_AREA)
                        .long(DEFAULT_AREA)
                        .value_name("AREA")
                        .help("Fallback area label for issues missing an area: label"),
                )
                .arg(
                    Arg::new(DEFAULT_TYPE)
                        .long(DEFAULT_TYPE)
                        .value_name("TYPE")
                        .value_parser(validate_default_type)
                        .help("Fallback type label for issues missing a type: label"),
                ),
        )
}

fn validate_default_type(value: &str) -> Result<String, String> {
    canonical_type_label(value).ok_or_else(|| {
        format!(
            "invalid default type '{value}'; must be one of: {}",
            CANONICAL_TYPES.join(", ")
        )
    })
}
