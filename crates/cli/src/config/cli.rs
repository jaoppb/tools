//! The command line of `tools config check|migrate`, parsed with clap.

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::path::PathBuf;

use crate::cli::CONFIG;

/// The verb that lists pending migrations and writes nothing.
pub const CHECK: &str = "check";
/// The verb that rewrites `tools.toml`.
pub const MIGRATE: &str = "migrate";
const DRY_RUN: &str = "dry-run";

/// The parsed arguments of `tools config check|migrate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    migrate: bool,
    dry_run: bool,
    config_path: Option<PathBuf>,
}

impl Arguments {
    /// Reads the arguments of the `check` or `migrate` verb; the global flags travel with `matches`.
    #[must_use]
    pub fn from_matches(verb: &str, matches: &ArgMatches) -> Self {
        Self {
            migrate: verb == MIGRATE,
            dry_run: matches.try_get_one::<bool>(DRY_RUN).ok().flatten().copied() == Some(true),
            config_path: matches.get_one::<String>(CONFIG).map(PathBuf::from),
        }
    }

    /// Whether the verb is `migrate`.
    #[must_use]
    pub const fn is_migrate(&self) -> bool {
        self.migrate
    }

    /// Whether `migrate` prints the new file instead of writing it.
    #[must_use]
    pub const fn dry_run(&self) -> bool {
        self.dry_run
    }

    /// The `--config` path, if given; otherwise `tools.toml` is searched upwards.
    #[must_use]
    pub fn config_path(&self) -> Option<&std::path::Path> {
        self.config_path.as_deref()
    }
}

/// The `config` noun with its `check` and `migrate` verbs.
pub fn command() -> Command {
    Command::new("config")
        .about("Checks or migrates tools.toml to the current configuration schema")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(CHECK)
                .about("List the removed keys tools.toml still carries; exits 1 when any is pending"),
        )
        .subcommand(
            Command::new(MIGRATE)
                .about("Rewrite tools.toml: status_mapping and [github.issues.rules] become [workflow]")
                .arg(
                    Arg::new(DRY_RUN)
                        .long(DRY_RUN)
                        .action(ArgAction::SetTrue)
                        .help("Print the migrated file instead of writing it"),
                ),
        )
}
