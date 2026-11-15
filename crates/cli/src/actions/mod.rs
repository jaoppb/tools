//! `tools actions check`: the `uses:` references of the workflows against the rules of
//! `[actions]` in `tools.toml`.
//!
//! Exits with 0 when every enabled rule holds (or none is enabled), with 1 when a reference
//! breaks one or the versions of its repository cannot be read, and with 2 when the
//! configuration or a workflow file cannot be read.

pub mod cli;

use std::path::{Path, PathBuf};

use tools_core::adapters::github::GhActions;
use tools_core::application::audit_actions;
use tools_core::config::{find_config, load_or_find};
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
    let references = read_references(arguments)?;
    if references.is_empty() {
        return Err(Exit::failed(
            "rules are enabled but no workflow file was found; run from the repository or pass --workflows",
        ));
    }
    let findings = audit_actions(&GhActions::default(), &rules, &references);
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

fn read_references(arguments: &Arguments) -> Result<Vec<Reference>, Exit> {
    let directories = arguments
        .directories()
        .map_or_else(default_directories, <[PathBuf]>::to_vec);
    let mut files = Vec::new();
    for directory in &directories {
        collect_files(directory, &mut files)?;
    }
    files.sort();
    let mut references = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).map_err(|error| unreadable(&path, &error))?;
        references.extend(scan(&path.to_string_lossy(), &text));
    }
    Ok(references)
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), Exit> {
    let entries = std::fs::read_dir(directory).map_err(|error| unreadable(directory, &error))?;
    for entry in entries {
        let entry = entry.map_err(|error| unreadable(directory, &error))?;
        let path = entry.path();
        // `file_type` does not follow symlinks, so a link cycle cannot recurse forever; a
        // symlinked directory is skipped and a symlinked file is read.
        let kind = entry
            .file_type()
            .map_err(|error| unreadable(&path, &error))?;
        if kind.is_symlink() && path.is_dir() {
            continue;
        }
        if kind.is_dir() {
            collect_files(&path, files)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("yml" | "yaml")
        ) {
            files.push(path);
        }
    }
    Ok(())
}

fn unreadable(path: &Path, error: &std::io::Error) -> Exit {
    Exit::failed(format!("cannot read {}: {error}", path.display()))
}

/// The default directories under the repository root, which is where `tools.toml` lives: the
/// working directory may be a subdirectory. Only the ones that exist are scanned.
fn default_directories() -> Vec<PathBuf> {
    let root = std::env::current_dir()
        .ok()
        .and_then(|directory| find_config(&directory))
        .and_then(|config| config.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    cli::DEFAULT_DIRECTORIES
        .iter()
        .map(|directory| root.join(directory))
        .filter(|directory| directory.is_dir())
        .collect()
}
