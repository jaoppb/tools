//! The command line of `tools actions check`, parsed with clap.

use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

use crate::cli::CONFIG;

/// The verb that audits the workflow references and writes nothing.
pub const CHECK: &str = "check";
const WORKFLOWS: &str = "workflows";
const DEFAULT_WORKFLOWS: &str = ".github/workflows";

/// The parsed arguments of `tools actions check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    config: Option<PathBuf>,
    workflows: PathBuf,
}

impl Arguments {
    /// Reads the arguments of the `check` verb.
    #[must_use]
    pub fn from_matches(_verb: &str, matches: &ArgMatches) -> Self {
        Self {
            config: matches.get_one::<String>(CONFIG).map(PathBuf::from),
            workflows: matches
                .get_one::<String>(WORKFLOWS)
                .map_or_else(|| PathBuf::from(DEFAULT_WORKFLOWS), PathBuf::from),
        }
    }

    /// The `--config` path, when given.
    #[must_use]
    pub fn config(&self) -> Option<&std::path::Path> {
        self.config.as_deref()
    }

    /// The directory of workflow files to scan.
    #[must_use]
    pub fn workflows(&self) -> &std::path::Path {
        &self.workflows
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
                        .help("Directory of workflow files (default: .github/workflows)"),
                ),
        )
}
