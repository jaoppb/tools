//! The command line of `tools changelog check`, parsed with clap.

use clap::{Arg, ArgAction, ArgMatches, Command};

/// The verb that checks a pull request for a changelog entry and writes nothing.
pub const CHECK: &str = "check";
const BASE: &str = "base";
const LABELS: &str = "labels";
const SKIP_LABEL: &str = "skip-label";

/// The parsed arguments of `tools changelog check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    base: String,
    labels: Vec<String>,
    skip_label: String,
}

impl Arguments {
    /// Reads the arguments of the `check` verb. `base` and `skip-label` are required flags, so
    /// clap has already rejected a command line without them.
    #[must_use]
    pub fn from_matches(_verb: &str, matches: &ArgMatches) -> Self {
        let text = |name: &str| matches.get_one::<String>(name).cloned().unwrap_or_default();
        Self {
            base: text(BASE),
            labels: matches
                .get_many::<String>(LABELS)
                .map(|values| {
                    values
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            skip_label: text(SKIP_LABEL),
        }
    }

    /// The git revision the pull request is compared with (`<base>...HEAD`).
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The labels on the pull request.
    #[must_use]
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// The label that waives the changelog entry.
    #[must_use]
    pub fn skip_label(&self) -> &str {
        &self.skip_label
    }
}

/// The `changelog` noun with its `check` verb.
pub fn command() -> Command {
    Command::new("changelog")
        .about("Checks that a pull request records its change in CHANGELOG.md")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(CHECK)
                .about(
                    "Exit 1 when the change touches crates/, action.yml, scripts/ or \
                     .github/workflows/ without touching CHANGELOG.md",
                )
                .arg(
                    Arg::new(BASE)
                        .long(BASE)
                        .value_name("REF")
                        .required(true)
                        .help("Git revision the change is compared with, as <REF>...HEAD"),
                )
                .arg(
                    Arg::new(LABELS)
                        .long(LABELS)
                        .value_name("CSV")
                        .action(ArgAction::Append)
                        .value_delimiter(',')
                        .help("Comma-separated labels on the pull request"),
                )
                .arg(
                    Arg::new(SKIP_LABEL)
                        .long(SKIP_LABEL)
                        .value_name("LABEL")
                        .required(true)
                        .help("Label that waives the changelog entry"),
                ),
        )
}
