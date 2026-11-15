//! The command line of `tools actions check`, parsed with clap.

use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

use crate::cli::CONFIG;

/// The verb that audits the workflow references and writes nothing.
pub const CHECK: &str = "check";
const WORKFLOWS: &str = "workflows";
/// The directories scanned when `--workflows` is not given, relative to the repository root.
pub const DEFAULT_DIRECTORIES: [&str; 2] = [".github/workflows", ".github/actions"];

/// The parsed arguments of `tools actions check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    config: Option<PathBuf>,
    directories: Option<Vec<PathBuf>>,
}

impl Arguments {
    /// Reads the arguments of the `check` verb.
    #[must_use]
    pub fn from_matches(_verb: &str, matches: &ArgMatches) -> Self {
        Self {
            config: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            directories: matches
                .get_many::<String>(WORKFLOWS)
                .map(|values| values.map(PathBuf::from).collect()),
        }
    }

    /// The `--config` path, when given.
    #[must_use]
    pub fn config(&self) -> Option<&std::path::Path> {
        self.config.as_deref()
    }

    /// The directories named with `--workflows`, scanned recursively for `.yml` and `.yaml`
    /// files; `None` means the defaults, resolved against the directory of `tools.toml`.
    #[must_use]
    pub fn directories(&self) -> Option<&[PathBuf]> {
        self.directories.as_deref()
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
                            "Directory scanned recursively for workflow and composite action files, repeatable (default: .github/workflows and .github/actions next to tools.toml)",
                        ),
                ),
        )
}
