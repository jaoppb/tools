//! Command-line entry point: `project-sync [--dry-run]`.
//!
//! Exits with 0 when the run succeeded or `--help` was asked, and with 2 when it
//! could not run. The report goes to standard error because printing macros are
//! forbidden.

use std::io::Write;
use std::process::ExitCode;

use project_sync::cli::Arguments;
use tools_core::adapters::github::GhProject;
use tools_core::application::Sync;

/// What to print and how to exit when the run stops early.
struct Exit {
    text: String,
    code: u8,
}

fn main() -> ExitCode {
    match run() {
        Ok(report) => emit(&report, 0),
        Err(exit) => emit(&exit.text, exit.code),
    }
}

fn run() -> Result<String, Exit> {
    let command_line = std::env::args_os().map(|argument| argument.to_string_lossy().into_owned());
    let arguments = Arguments::parse(command_line).map_err(|usage| Exit {
        text: usage.to_string(),
        code: usage.exit_code(),
    })?;
    let target = arguments
        .resolve_target()
        .map_err(|err| Exit { text: err, code: 2 })?;
    let mut sync = Sync::new(GhProject::new(target));
    let outcome = sync.run(arguments.mode()).map_err(|error| Exit {
        text: error.to_string(),
        code: 2,
    })?;
    Ok(outcome.to_string())
}

fn emit(text: &str, code: u8) -> ExitCode {
    let line = format!("{}\n", text.trim_end());
    let written = std::io::stderr().lock().write_all(line.as_bytes());
    written.map_or_else(|_| ExitCode::from(2), |()| ExitCode::from(code))
}
