//! The command line of `tools changelog check`, parsed with clap.

use clap::{Arg, ArgAction, ArgMatches, Command};

/// The verb that checks a pull request for a changelog entry and writes nothing.
pub const CHECK: &str = "check";
const BASE: &str = "base";
const LABELS: &str = "labels";
const SKIP_LABEL: &str = "skip-label";
const WATCH: &str = "watch";
const CHANGELOG: &str = "changelog";

/// The parsed arguments of `tools changelog check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arguments {
    base: String,
    labels: Vec<String>,
    skip_label: String,
    watch: Vec<String>,
    changelog: String,
}

impl Arguments {
    /// Reads the arguments of the `check` verb; clap has already rejected a command line without
    /// the required flags.
    #[must_use]
    pub fn from_matches(_verb: &str, matches: &ArgMatches) -> Self {
        // arch-lint forbids `expect`; the flags are `required`, so the default never shows.
        let text = |name: &str| matches.get_one::<String>(name).cloned().unwrap_or_default();
        Self {
            base: text(BASE),
            watch: matches
                .get_many::<String>(WATCH)
                .map(|values| values.cloned().collect())
                .unwrap_or_default(),
            changelog: text(CHANGELOG),
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

    /// The paths whose change needs an entry: a directory ends in `/`, a file does not.
    #[must_use]
    pub fn watch(&self) -> &[String] {
        &self.watch
    }

    /// The changelog file a change must touch.
    #[must_use]
    pub fn changelog(&self) -> &str {
        &self.changelog
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
                    "Exit 1 when the change touches a watched path without touching the changelog",
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
                    Arg::new(WATCH)
                        .long(WATCH)
                        .value_name("PATH")
                        .required(true)
                        .action(ArgAction::Append)
                        .help(
                            "Path whose change needs an entry, repeatable; a directory ends in /",
                        ),
                )
                .arg(
                    Arg::new(CHANGELOG)
                        .long(CHANGELOG)
                        .value_name("FILE")
                        .required(true)
                        .help("Changelog file the change must touch"),
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
