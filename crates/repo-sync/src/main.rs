//! Command-line entry point: `repo-sync [--check | --dry-run] [--settings] [--labels] [--milestones]`.
//!
//! Exits with 0 when repository configuration is in sync or applied, with 1 when `--check`
//! finds configuration drift, and with 2 on configuration or runtime errors.
//! The report goes to standard error because printing macros are forbidden.

use std::io::Write;
use std::process::ExitCode;

use repo_sync::cli::Arguments;
use tools_core::adapters::github::{GhRepo, Target};
use tools_core::application::{RepoSync, RepoSyncError};

/// What to print and how to exit.
struct Exit {
    text: String,
    code: u8,
}

fn main() -> ExitCode {
    match run() {
        Ok(outcome) => emit(&outcome, 0),
        Err(exit) => emit(&exit.text, exit.code),
    }
}

fn run() -> Result<String, Exit> {
    let command_line = std::env::args_os().map(|argument| argument.to_string_lossy().into_owned());
    let arguments = Arguments::parse(command_line).map_err(|usage| Exit {
        text: usage.to_string(),
        code: usage.exit_code(),
    })?;
    let resolved = arguments.resolve().map_err(|err| Exit {
        text: err.to_string(),
        code: 2,
    })?;
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
                return Err(Exit { text, code: 2 });
            }
        }
    }

    if !drift_outputs.is_empty() {
        return Err(Exit {
            text: drift_outputs.join("\n"),
            code: 1,
        });
    }

    Ok(outputs.join("\n"))
}

fn emit(text: &str, code: u8) -> ExitCode {
    let line = format!("{}\n", text.trim_end());
    let written = std::io::stderr().lock().write_all(line.as_bytes());
    written.map_or_else(|_| ExitCode::from(2), |()| ExitCode::from(code))
}
