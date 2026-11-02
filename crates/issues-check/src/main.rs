//! Command-line entry point: `issues-check`.
//!
//! Exits with 0 when the issues break no rule or `--help` was asked, with 1 when
//! they do, and with 2 when the check could not run. The report goes to standard
//! error because printing macros are forbidden.

use std::io::Write;
use std::process::ExitCode;

use issues_check::cli::Arguments;
use tools_core::adapters::github::{GhIssues, GhProject, GhText};
use tools_core::application::{Audit, IssueMigrator, RepositoryIssues};

/// What to print and how to exit.
struct Exit {
    text: String,
    code: u8,
}

fn main() -> ExitCode {
    match run() {
        Ok(exit) | Err(exit) => emit(&exit.text, exit.code),
    }
}

fn run() -> Result<Exit, Exit> {
    let command_line = std::env::args_os().map(|argument| argument.to_string_lossy().into_owned());
    let arguments = Arguments::parse(command_line).map_err(|usage| Exit {
        text: usage.to_string(),
        code: usage.exit_code(),
    })?;
    let resolved = arguments.resolve().map_err(|err| Exit {
        text: err.to_string(),
        code: 2,
    })?;

    if arguments.is_migrate() {
        let gh_issues = GhIssues::new(resolved.target);
        let tree = gh_issues.issues().map_err(|error| Exit {
            text: error.to_string(),
            code: 2,
        })?;
        let migrator = IssueMigrator::new(
            &gh_issues,
            resolved.migrations,
            arguments.default_area().map(str::to_owned),
        );
        let outcome = migrator
            .run(&tree, arguments.migration_mode())
            .map_err(|error| Exit {
                text: error.to_string(),
                code: 2,
            })?;
        return Ok(Exit {
            text: outcome.to_string(),
            code: 0,
        });
    }

    let project = resolved
        .target
        .project()
        .map(|_| GhProject::new(resolved.target.clone()));
    let report = Audit::new(
        GhIssues::new(resolved.target.clone()),
        project,
        GhText::new(resolved.target),
        resolved.rules,
    )
    .run()
    .map_err(|error| Exit {
        text: error.to_string(),
        code: 2,
    })?;
    Ok(Exit {
        text: report.to_string(),
        code: report.exit_code(),
    })
}

fn emit(text: &str, code: u8) -> ExitCode {
    let line = format!("{}\n", text.trim_end());
    let written = std::io::stderr().lock().write_all(line.as_bytes());
    written.map_or_else(|_| ExitCode::from(2), |()| ExitCode::from(code))
}
