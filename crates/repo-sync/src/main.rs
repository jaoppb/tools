//! Command-line entry point: `repo-sync [--check | --dry-run]`.
//!
//! Exits with 0 when settings are in sync or applied, with 1 when `--check`
//! finds settings drift, and with 2 on configuration or runtime errors.
//! The report goes to standard error because printing macros are forbidden.

use std::io::Write;
use std::process::ExitCode;

use tools_core::adapters::github::GhRepo;
use tools_core::application::{RepoSync, RepoSyncError};
use tools_core::cli::{RepoArguments, RepoUsageError};

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
    let arguments = RepoArguments::parse(command_line).map_err(|usage: RepoUsageError| Exit {
        text: usage.to_string(),
        code: usage.exit_code(),
    })?;
    let (target, desired) = arguments
        .resolve()
        .map_err(|err| Exit { text: err, code: 2 })?;
    let repo_sync = RepoSync::new(GhRepo::new(target), desired);
    let outcome = repo_sync.run(arguments.mode()).map_err(|error| match error {
        RepoSyncError::Drift(report) => Exit {
            text: report,
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
