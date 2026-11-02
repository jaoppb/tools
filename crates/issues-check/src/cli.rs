//! The command line of `issues-check`.

use clap::{Arg, ArgAction, Command};
use std::collections::BTreeMap;
use std::path::PathBuf;

use tools_core::adapters::github::Target;
use tools_core::application::IssueMigrationMode;
use tools_core::config::{ConfigError, RepoLabelsSpec, RulesConfig, load_or_find};
use tools_core::domain::{CANONICAL_TYPES, canonical_type_label};

const PROGRAM: &str = "issues-check";
const CONFIG: &str = "config";
const OWNER: &str = "owner";
const REPO: &str = "repo";
const PROJECT: &str = "project";
const NO_PARENT: &str = "no-parent";
const NO_MEMBERSHIP: &str = "no-membership";
const MIGRATE: &str = "migrate";
const APPLY: &str = "apply";
const DEFAULT_AREA: &str = "default-area";
const DEFAULT_TYPE: &str = "default-type";

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
    migrate: bool,
    apply: bool,
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
            migrate: matches.get_flag(MIGRATE),
            apply: matches.get_flag(APPLY),
            default_area: matches.get_one::<String>(DEFAULT_AREA).cloned(),
            default_type: matches.get_one::<String>(DEFAULT_TYPE).cloned(),
        })
    }

    /// Whether issue migration mode is enabled.
    #[must_use]
    pub const fn is_migrate(&self) -> bool {
        self.migrate
    }

    /// Execution mode for issue label migration.
    #[must_use]
    pub const fn migration_mode(&self) -> IssueMigrationMode {
        if self.apply {
            IssueMigrationMode::Apply
        } else {
            IssueMigrationMode::Preview
        }
    }

    /// Whether to apply issue migrations directly to GitHub.
    #[must_use]
    pub const fn is_apply(&self) -> bool {
        self.apply
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

fn command() -> Command {
    Command::new(PROGRAM)
        .about("Audits and migrates the structure and labels of repository issues")
        .arg(
            Arg::new(CONFIG)
                .long(CONFIG)
                .value_name("FILE")
                .help("Path to tools.toml configuration file"),
        )
        .arg(
            Arg::new(OWNER)
                .long(OWNER)
                .visible_alias("org")
                .value_name("OWNER")
                .help("GitHub repository owner or organization override"),
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
        .arg(
            Arg::new(MIGRATE)
                .long(MIGRATE)
                .action(ArgAction::SetTrue)
                .help("Plan or apply issue label migrations to match standard taxonomy"),
        )
        .arg(
            Arg::new(APPLY)
                .long(APPLY)
                .action(ArgAction::SetTrue)
                .requires(MIGRATE)
                .help(
                    "Apply issue label migrations directly to GitHub (defaults to dry-run preview)",
                ),
        )
        .arg(
            Arg::new(DEFAULT_AREA)
                .long(DEFAULT_AREA)
                .value_name("AREA")
                .requires(MIGRATE)
                .help("Fallback area label for issues missing an area: label"),
        )
        .arg(
            Arg::new(DEFAULT_TYPE)
                .long(DEFAULT_TYPE)
                .value_name("TYPE")
                .requires(MIGRATE)
                .value_parser(validate_default_type)
                .help("Fallback type label for issues missing a type: label"),
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
