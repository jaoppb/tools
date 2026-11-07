//! The `tools` command line: `tools <noun> <verb>` over the issue, repository and project
//! workflows.
//!
//! Each noun is a cargo feature (`issues`, `repo`, `project`, all on by default), so a build can
//! carry only the groups it needs. Read-only verbs are always `check`; verbs that write are
//! `sync` and `migrate`.

pub mod cli;
mod exit;
#[cfg(feature = "issues")]
pub mod issues;
#[cfg(feature = "project")]
pub mod project;
#[cfg(feature = "repo")]
pub mod repo;

pub use exit::Exit;

use cli::Invocation;

/// Runs a parsed command line.
///
/// # Errors
///
/// Returns the [`Exit`] (text and code) of a run that stops early; a finished run is an
/// [`Exit`] too, with the code its verb defines.
pub fn run(invocation: Invocation) -> Result<Exit, Exit> {
    match invocation {
        #[cfg(feature = "issues")]
        Invocation::Issues(arguments) => issues::run(&arguments),
        #[cfg(feature = "repo")]
        Invocation::Repo(arguments) => repo::run(&arguments),
        #[cfg(feature = "project")]
        Invocation::Project(arguments) => project::run(&arguments),
    }
}
