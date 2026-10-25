//! Running `gh` and returning what it printed.

use std::io::ErrorKind;
use std::process::Command;

use crate::adapters::github::error::GhError;

/// The `gh` program, or another program with the same interface for tests.
#[derive(Debug, Clone)]
pub struct GhRunner {
    program: String,
}

impl Default for GhRunner {
    fn default() -> Self {
        Self::new("gh")
    }
}

impl GhRunner {
    /// A runner for this program.
    #[must_use]
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_owned(),
        }
    }

    /// Runs one GraphQL request. Each entry of `fields` is `name=value` and is
    /// passed as a string variable.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Missing`] when the program is not installed, and
    /// [`GhError::Failed`] when it fails or exits with an error.
    pub fn graphql(&self, query: &str, fields: &[String]) -> Result<String, GhError> {
        let mut command = Command::new(&self.program);
        command
            .args(["api", "graphql", "-f"])
            .arg(format!("query={query}"))
            .args(fields.iter().flat_map(|field| ["-f", field.as_str()]));
        run(&mut command)
    }
}

fn run(command: &mut Command) -> Result<String, GhError> {
    let output = command.output().map_err(|error| match error.kind() {
        ErrorKind::NotFound => GhError::Missing,
        _ => GhError::Failed {
            reason: error.to_string(),
        },
    })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(failure(&output.stderr, &output.status.to_string()))
}

fn failure(stderr: &[u8], status: &str) -> GhError {
    let said = String::from_utf8_lossy(stderr).trim().to_owned();
    if said.is_empty() {
        return GhError::Failed {
            reason: status.to_owned(),
        };
    }
    GhError::Failed { reason: said }
}
