//! `tools project check|sync`: the Status of Project cards against the workflow rules.
//!
//! `check` prints the planned moves and exits with 1 when there are any; `sync` writes them.

pub mod cli;

use tools_core::adapters::github::GhProject;
use tools_core::application::{Mode, Sync};

use crate::Exit;
use cli::Arguments;

/// Runs `tools project check|sync`.
///
/// # Errors
///
/// Returns an [`Exit`] with code 2 when the run cannot complete. A `check` that plans moves is an
/// `Ok` [`Exit`] with code 1.
pub fn run(arguments: &Arguments) -> Result<Exit, Exit> {
    let (target, mapping, rules) = arguments.resolve().map_err(Exit::failed)?;
    let gh_project = GhProject::new(target).with_mapping(mapping);
    let mut sync = Sync::with_rules(gh_project, rules);
    let outcome = sync.run(arguments.mode()).map_err(Exit::failed)?;
    let pending = arguments.mode() == Mode::Preview && !outcome.plan().moves().is_empty();
    Ok(Exit::new(outcome.to_string(), u8::from(pending)))
}
