//! Command-line entry point: `tools <issues|repo|project> <verb>`.
//!
//! Exits with 0 when the run succeeded or `--help` was asked, with 1 when a `check` finds
//! something to fix, and with 2 when the run could not complete. The report goes to standard
//! error because printing macros are forbidden.

use std::io::Write;
use std::process::ExitCode;

use tools::Exit;

fn main() -> ExitCode {
    let exit = match run() {
        Ok(exit) | Err(exit) => exit,
    };
    emit(&exit)
}

fn run() -> Result<Exit, Exit> {
    let command_line = std::env::args_os().map(|argument| argument.to_string_lossy().into_owned());
    let invocation = tools::cli::parse(command_line)
        .map_err(|usage| Exit::new(usage.to_string(), usage.exit_code()))?;
    tools::run(invocation)
}

fn emit(exit: &Exit) -> ExitCode {
    let line = format!("{}\n", exit.text.trim_end());
    let written = std::io::stderr().lock().write_all(line.as_bytes());
    written.map_or_else(|_| ExitCode::from(2), |()| ExitCode::from(exit.code))
}
