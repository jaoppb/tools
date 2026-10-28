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

    /// Runs a REST API GET request against GitHub.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Missing`] when the program is not installed, and
    /// [`GhError::Failed`] when it fails or exits with an error.
    pub fn rest_get(&self, endpoint: &str) -> Result<String, GhError> {
        let mut command = Command::new(&self.program);
        command.args(["api", endpoint]);
        run(&mut command)
    }

    /// Runs a REST API PATCH request against GitHub with a JSON payload provided on standard input.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Missing`] when the program is not installed, and
    /// [`GhError::Failed`] when it fails or exits with an error.
    pub fn rest_patch(&self, endpoint: &str, body: &str) -> Result<String, GhError> {
        self.rest_send("PATCH", endpoint, body)
    }

    /// Runs a REST API POST request against GitHub with a JSON payload provided on standard input.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Missing`] when the program is not installed, and
    /// [`GhError::Failed`] when it fails or exits with an error.
    pub fn rest_post(&self, endpoint: &str, body: &str) -> Result<String, GhError> {
        self.rest_send("POST", endpoint, body)
    }

    fn rest_send(&self, method: &str, endpoint: &str, body: &str) -> Result<String, GhError> {
        let mut command = Command::new(&self.program);
        command.args(["api", "-X", method, endpoint, "--input", "-"]);
        run_with_input(&mut command, body)
    }

    /// Runs a REST API DELETE request against GitHub.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Missing`] when the program is not installed, and
    /// [`GhError::Failed`] when it fails or exits with an error.
    pub fn rest_delete(&self, endpoint: &str) -> Result<String, GhError> {
        let mut command = Command::new(&self.program);
        command.args(["api", "-X", "DELETE", endpoint]);
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

fn run_with_input(command: &mut Command, input: &str) -> Result<String, GhError> {
    use std::io::Write;
    use std::process::Stdio;

    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|error| match error.kind() {
        ErrorKind::NotFound => GhError::Missing,
        _ => GhError::Failed {
            reason: error.to_string(),
        },
    })?;

    if let Some(mut stdin) = child.stdin.take()
        && let Err(write_err) = stdin.write_all(input.as_bytes())
    {
        drop(stdin);
        if let Ok(output) = child.wait_with_output() {
            let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if !said.is_empty() {
                return Err(GhError::Failed { reason: said });
            }
        }
        return Err(GhError::Failed {
            reason: format!("failed to write payload to standard input: {write_err}"),
        });
    }

    let output = child.wait_with_output().map_err(|error| GhError::Failed {
        reason: error.to_string(),
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
