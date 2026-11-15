//! The command line of `tools actions check`, parsed with clap.

use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

use crate::cli::CONFIG;

/// The verb that audits the workflow references and writes nothing.
pub const CHECK: &str = "check";
const WORKFLOWS: &str = "workflows";
const DEFAULT_DIRECTORIES: [&str; 2] = [".github/workflows", ".github/actions"];

/// The parsed arguments of `tools actions check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    config: Option<PathBuf>,
    directories: Vec<PathBuf>,
    explicit: bool,
}

impl Arguments {
    /// Reads the arguments of the `check` verb.
    #[must_use]
    pub fn from_matches(_verb: &str, matches: &ArgMatches) -> Self {
        Self {
            config: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            directories: matches.get_many::<String>(WORKFLOWS).map_or_else(
                || DEFAULT_DIRECTORIES.iter().map(PathBuf::from).collect(),
                |values| values.map(PathBuf::from).collect(),
            ),
            explicit: matches.contains_id(WORKFLOWS),
        }
    }

    /// The `--config` path, when given.
    #[must_use]
    pub fn config(&self) -> Option<&std::path::Path> {
        self.config.as_deref()
    }

    /// The directories scanned recursively for `.yml` and `.yaml` files.
    #[must_use]
    pub fn directories(&self) -> &[PathBuf] {
        &self.directories
    }

    /// Whether the directories were named with `--workflows`: a missing one is then an error,
    /// while a missing default is skipped.
    #[must_use]
    pub const fn explicit(&self) -> bool {
        self.explicit
    }
}

/// The `actions` noun with its `check` verb.
pub fn command() -> Command {
    Command::new("actions")
        .about("Audits the `uses:` references of GitHub workflows")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(CHECK)
                .about("Exit 1 when a reference breaks a rule enabled under [actions]")
                .arg(
                    Arg::new(WORKFLOWS)
                        .long(WORKFLOWS)
                        .value_name("DIR")
                        .action(clap::ArgAction::Append)
                        .help(
                            "Directory scanned recursively for workflow and composite action files, repeatable (default: .github/workflows and .github/actions)",
                        ),
                ),
        )
}
