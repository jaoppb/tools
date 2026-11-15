//! `tools actions check`: the `uses:` references of the workflows against the rules of
//! `[actions]` in `tools.toml`.
//!
//! Exits with 0 when every enabled rule holds (or none is enabled), with 1 when a reference
//! breaks one, and with 2 when the configuration, a workflow file or GitHub cannot be read.

pub mod cli;

use std::path::Path;

use tools_core::adapters::github::GhActions;
use tools_core::application::audit_actions;
use tools_core::config::load_or_find;
use tools_core::domain::actions::{Reference, scan};

use crate::Exit;
use cli::Arguments;

/// Runs `tools actions check`.
///
/// # Errors
///
/// Returns an [`Exit`] with code 1 on findings, and 2 when the run cannot complete.
pub fn run(arguments: &Arguments) -> Result<Exit, Exit> {
    let config = load_or_find(arguments.config()).map_err(Exit::failed)?;
    let Some(rules) = config
        .actions()
        .filter(tools_core::domain::actions::Rules::any)
    else {
        return Ok(Exit::new(
            "no [actions] rule is enabled: nothing to check",
            0,
        ));
    };
    let references = read_references(arguments.workflows())?;
    let findings =
        audit_actions(&GhActions::default(), &rules, &references).map_err(Exit::failed)?;
    if findings.is_empty() {
        return Ok(Exit::new(
            format!(
                "{} action references checked: all rules hold",
                references.len()
            ),
            0,
        ));
    }
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    Err(Exit::new(lines.join("\n"), 1))
}

fn read_references(directory: &Path) -> Result<Vec<Reference>, Exit> {
    let unreadable = |path: &Path, error: std::io::Error| {
        Exit::failed(format!("cannot read {}: {error}", path.display()))
    };
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .map_err(|error| unreadable(directory, error))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("yml" | "yaml")
            )
        })
        .collect();
    files.sort();
    let mut references = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).map_err(|error| unreadable(&path, error))?;
        references.extend(scan(&path.to_string_lossy(), &text));
    }
    Ok(references)
}
