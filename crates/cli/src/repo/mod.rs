//! `tools repo check|sync`: repository settings, labels and milestones against `tools.toml`.
//!
//! `check` exits with 0 when everything is in sync, with 1 on drift and with 2 on configuration
//! or runtime errors; `sync` reconciles GitHub to `tools.toml`.

pub mod cli;
pub mod consistency;

use tools_core::adapters::github::{GhRepo, Target};
use tools_core::application::{RepoSync, RepoSyncError};

use crate::Exit;
use cli::{Arguments, ResolvedRepo};

/// Runs `tools repo check|sync`.
///
/// # Errors
///
/// Returns an [`Exit`] with code 1 on drift or file problems, and 2 when the run cannot complete.
pub fn run(arguments: &Arguments) -> Result<Exit, Exit> {
    let resolved = arguments.resolve().map_err(Exit::failed)?;
    if arguments.checks_files(&resolved) {
        let problems = file_problems(arguments, &resolved);
        if !problems.is_empty() {
            return Err(Exit::new(problems.join("\n"), 1));
        }
    }
    let is_multi = resolved.repos.len() > 1;
    let mut outputs = Vec::new();
    let mut drift_outputs = Vec::new();

    for (repo, spec) in &resolved.repos {
        let single_target = Target::single(repo.owner(), repo.name(), resolved.target.project());
        let repo_sync = RepoSync::new(GhRepo::new(single_target), spec.clone());
        match repo_sync.run(arguments.mode()) {
            Ok(outcome) => {
                let text = outcome.to_string();
                if !text.is_empty() {
                    if is_multi {
                        outputs.push(format!("[{}]\n{text}", repo.name()));
                    } else {
                        outputs.push(text);
                    }
                }
            }
            Err(RepoSyncError::Drift(outcome)) => {
                if is_multi {
                    drift_outputs.push(format!("[{}]\n{outcome}", repo.name()));
                } else {
                    drift_outputs.push(outcome.to_string());
                }
            }
            Err(RepoSyncError::Read(reason) | RepoSyncError::Write(reason)) => {
                let text = if is_multi {
                    format!("[{}] {reason}", repo.name())
                } else {
                    reason
                };
                return Err(Exit::new(text, 2));
            }
        }
    }

    if !drift_outputs.is_empty() {
        return Err(Exit::new(drift_outputs.join("\n"), 1));
    }

    Ok(Exit::new(outputs.join("\n"), 0))
}

/// Problems between `tools.toml` and the local templates and docs, checked from its directory.
/// They are reported alone, before any GitHub drift, so the two kinds never share a run.
fn file_problems(arguments: &Arguments, resolved: &ResolvedRepo) -> Vec<String> {
    let names: Vec<String> = resolved
        .repos
        .first()
        .and_then(|(_, spec)| spec.labels.as_ref())
        .map(|labels| {
            labels
                .items
                .iter()
                .map(|label| label.name.clone())
                .collect()
        })
        .unwrap_or_default();
    consistency::check(&arguments.root(), &names, resolved.issue_templates.as_ref())
}
