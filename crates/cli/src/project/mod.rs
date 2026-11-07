//! `tools project check|sync`: the Status of Project cards against the workflow rules.
//!
//! `check` prints the planned moves and the frozen cards and exits with 1 when there are any;
//! `sync` writes the moves.

pub mod cli;

use tools_core::adapters::github::GhProject;
use tools_core::application::{Mode, Outcome, Sync};
use tools_core::domain::IssueNumber;

use crate::Exit;
use cli::Arguments;

/// Runs `tools project check|sync`.
///
/// # Errors
///
/// Returns an [`Exit`] with code 2 when the run cannot complete. A `check` that plans moves or
/// freezes a card is an `Ok` [`Exit`] with code 1.
pub fn run(arguments: &Arguments) -> Result<Exit, Exit> {
    let (target, workflow) = arguments.resolve().map_err(Exit::failed)?;
    let project = GhProject::with_statuses(target, workflow.statuses().clone());
    let mut sync = Sync::new(project, workflow);
    let outcome = sync.run(arguments.mode()).map_err(Exit::failed)?;
    if let Some(selector) = arguments.explain() {
        return explain(&outcome, selector);
    }
    let pending = arguments.mode() == Mode::Preview && outcome.pending();
    Ok(Exit::new(outcome.to_string(), u8::from(pending)))
}

/// The trace of the card `selector` names. Exit 0: it is a view, not a verdict.
fn explain(outcome: &Outcome, selector: &str) -> Result<Exit, Exit> {
    let matching: Vec<&IssueNumber> = outcome
        .evaluation()
        .iter()
        .map(|decision| decision.number())
        .filter(|number| selects(selector, number))
        .collect();
    match matching.as_slice() {
        [number] => {
            let decision = outcome.evaluation().decision(number);
            Ok(Exit::new(
                decision
                    .map(|found| found.explain().to_string())
                    .unwrap_or_default(),
                0,
            ))
        }
        [] => Err(Exit::failed(format!(
            "{selector} is not a card of the Project"
        ))),
        _ => Err(Exit::failed(format!(
            "{selector} is ambiguous: name it as repo#number"
        ))),
    }
}

/// `57` matches that number in every repository, `repo#57` only in `repo`.
fn selects(selector: &str, number: &IssueNumber) -> bool {
    let wanted = selector.trim();
    match wanted.rsplit_once('#') {
        Some((repo, raw)) => {
            raw.parse() == Ok(number.value()) && (repo.is_empty() || repo == number.repo().name())
        }
        None => wanted.parse() == Ok(number.value()),
    }
}
