//! The root command line: the flags every verb shares and the dispatch to the enabled groups.

use clap::{Arg, ArgAction, Command};

/// `--config`: the path to `tools.toml`.
pub const CONFIG: &str = "config";
/// `--owner` (alias `--org`): the GitHub owner or organization.
pub const OWNER: &str = "owner";
/// `--repo`: a repository name, repeatable.
pub const REPO: &str = "repo";

/// `--project`: the GitHub Project number, on the verbs that read a Project.
pub const PROJECT: &str = "project";

/// The `--project <NUMBER>` flag shared by the issue and project verbs.
#[must_use]
pub fn project_flag() -> Arg {
    Arg::new(PROJECT)
        .long(PROJECT)
        .value_name("NUMBER")
        .help("GitHub Project number override")
}

/// The repositories named by the global `--repo` flag.
#[must_use]
pub fn repos_of(matches: &clap::ArgMatches) -> Vec<String> {
    matches
        .get_many::<String>(REPO)
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

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

/// A parsed command line: the group and verb it names, with their arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// `tools issues check|migrate`.
    #[cfg(feature = "issues")]
    Issues(crate::issues::cli::Arguments),
    /// `tools repo check|sync`.
    #[cfg(feature = "repo")]
    Repo(crate::repo::cli::Arguments),
    /// `tools project check|sync`.
    #[cfg(feature = "project")]
    Project(crate::project::cli::Arguments),
}

/// The whole command tree: the shared flags and one subcommand per enabled group.
#[must_use]
pub fn command() -> Command {
    let root = Command::new("tools")
        .about("Issue workflow, repository and project automation")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new(CONFIG)
                .long(CONFIG)
                .global(true)
                .value_name("FILE")
                .help("Path to tools.toml configuration file"),
        )
        .arg(
            Arg::new(OWNER)
                .long(OWNER)
                .visible_alias("org")
                .global(true)
                .value_name("OWNER")
                .help("GitHub repository owner or organization override"),
        )
        .arg(
            Arg::new(REPO)
                .long(REPO)
                .global(true)
                .value_name("REPO")
                .action(ArgAction::Append)
                .help("GitHub repository name override"),
        );
    #[cfg(feature = "issues")]
    let root = root.subcommand(crate::issues::cli::command());
    #[cfg(feature = "repo")]
    let root = root.subcommand(crate::repo::cli::command());
    #[cfg(feature = "project")]
    let root = root.subcommand(crate::project::cli::command());
    root
}

/// Reads the whole command line, program name first.
///
/// # Errors
///
/// Returns [`UsageError`] for an argument the tool does not know, and for `--help`, which asks
/// for the usage and is not a failure.
pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Invocation, UsageError> {
    let matches = command().try_get_matches_from(arguments)?;
    match matches.subcommand() {
        #[cfg(feature = "issues")]
        Some(("issues", group)) => group.subcommand().map_or_else(
            || Err(missing_verb()),
            |(verb, verb_matches)| {
                Ok(Invocation::Issues(
                    crate::issues::cli::Arguments::from_matches(verb, verb_matches),
                ))
            },
        ),
        #[cfg(feature = "repo")]
        Some(("repo", group)) => group.subcommand().map_or_else(
            || Err(missing_verb()),
            |(verb, verb_matches)| {
                Ok(Invocation::Repo(crate::repo::cli::Arguments::from_matches(
                    verb,
                    verb_matches,
                )))
            },
        ),
        #[cfg(feature = "project")]
        Some(("project", group)) => group.subcommand().map_or_else(
            || Err(missing_verb()),
            |(verb, verb_matches)| {
                Ok(Invocation::Project(
                    crate::project::cli::Arguments::from_matches(verb, verb_matches),
                ))
            },
        ),
        _ => Err(missing_verb()),
    }
}

fn missing_verb() -> UsageError {
    UsageError(command().error(
        clap::error::ErrorKind::MissingSubcommand,
        "a group and a verb are required",
    ))
}
