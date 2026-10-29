//! Command-line entry point: `repo-sync [--check | --dry-run] [--settings] [--labels] [--milestones]`.
//!
//! Exits with 0 when repository configuration is in sync or applied, with 1 when `--check`
//! finds configuration drift, and with 2 on configuration or runtime errors.
//! The report goes to standard error because printing macros are forbidden.

use std::io::Write;
use std::process::ExitCode;

use repo_sync::cli::RepoArguments;
use tools_core::adapters::github::GhRepo;
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
    let arguments = RepoArguments::parse(command_line).map_err(|usage| Exit {
        text: usage.to_string(),
        code: usage.exit_code(),
    })?;
    let resolved = arguments.resolve().map_err(|err| Exit {
        text: err.to_string(),
        code: 2,
    })?;
    let repo_sync = RepoSync::new(GhRepo::new(resolved.target), resolved.spec);
    let outcome = repo_sync
        .run(arguments.mode())
        .map_err(|error| match error {
            RepoSyncError::Drift(outcome) => Exit {
                text: outcome.to_string(),
                code: 1,
            },
            RepoSyncError::Read(reason) | RepoSyncError::Write(reason) => Exit {
                text: reason,
                code: 2,
            },
        })?;
    Ok(outcome.to_string())
}

fn emit(text: &str, code: u8) -> ExitCode {
    let line = format!("{}\n", text.trim_end());
    let written = std::io::stderr().lock().write_all(line.as_bytes());
    written.map_or_else(|_| ExitCode::from(2), |()| ExitCode::from(code))
}
