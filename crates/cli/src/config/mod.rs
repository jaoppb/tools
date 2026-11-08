//! `tools config check|migrate`: the removed `tools.toml` keys and their rewrite into `[workflow]`.
//!
//! `check` exits with 1 when a migration is pending, 0 when the file is current, and 2 when it
//! cannot be read or does not load. `migrate` writes the file (or prints it with `--dry-run`),
//! exits 0 when done or when there was nothing to do, 1 when it refuses to guess, and 2 when it
//! cannot run.

pub mod cli;

use std::path::PathBuf;

use tools_core::config::migrate::{self, MigrateError};
use tools_core::config::{find_config, load_from_str};

use crate::Exit;
use cli::Arguments;

/// Runs `tools config check|migrate`.
///
/// # Errors
///
/// Returns an [`Exit`] with code 2 when the run cannot complete.
pub fn run(arguments: &Arguments) -> Result<Exit, Exit> {
    let path = locate(arguments)?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| Exit::failed(format!("failed to read {}: {error}", path.display())))?;
    if arguments.is_migrate() {
        migrate(arguments, &path, &contents)
    } else {
        check(&path, &contents)
    }
}

fn locate(arguments: &Arguments) -> Result<PathBuf, Exit> {
    if let Some(path) = arguments.config_path() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .ok()
        .and_then(|cwd| find_config(&cwd))
        .ok_or_else(|| Exit::failed("tools.toml not found in current directory or any parent"))
}

fn check(path: &std::path::Path, contents: &str) -> Result<Exit, Exit> {
    let report = migrate::inspect(contents).map_err(Exit::failed)?;
    if report.pending.is_empty() {
        load_from_str(contents)
            .map_err(|error| Exit::failed(format!("{}: {error}", path.display())))?;
        let mut lines = vec![format!("{}: nothing to migrate", path.display())];
        lines.extend(report.deprecated);
        return Ok(Exit::new(lines.join("\n"), 0));
    }
    let mut lines: Vec<String> = report
        .pending
        .iter()
        .map(|item| format!("{}: {item}", path.display()))
        .collect();
    lines.extend(report.deprecated);
    lines.push("run `tools config migrate` to rewrite the file".to_owned());
    Ok(Exit::new(lines.join("\n"), 1))
}

fn migrate(arguments: &Arguments, path: &std::path::Path, contents: &str) -> Result<Exit, Exit> {
    let migration = match migrate::migrate(contents) {
        Ok(migration) => migration,
        Err(refusal @ MigrateError::Rename(_)) => return Ok(Exit::new(refusal.to_string(), 1)),
        Err(error) => return Err(Exit::failed(error)),
    };
    if !migration.changed() {
        return Ok(Exit::new(
            format!("{}: nothing to migrate", path.display()),
            0,
        ));
    }
    load_from_str(&migration.text).map_err(|error| {
        Exit::failed(format!(
            "the migrated {} would not load, nothing was written: {error}",
            path.display()
        ))
    })?;
    let notes: Vec<String> = migration
        .notes
        .iter()
        .map(|note| format!("- {note}"))
        .collect();
    if arguments.dry_run() {
        return Ok(Exit::new(
            format!("{}\n\n{}", notes.join("\n"), migration.text),
            0,
        ));
    }
    std::fs::write(path, &migration.text)
        .map_err(|error| Exit::failed(format!("failed to write {}: {error}", path.display())))?;
    Ok(Exit::new(
        format!("migrated {}\n{}", path.display(), notes.join("\n")),
        0,
    ))
}
