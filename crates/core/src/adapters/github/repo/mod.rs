//! Reading and writing repository settings via `gh api`.

use crate::adapters::github::error::GhError;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::domain::repo::RepoSettings;

/// Communicates with GitHub REST API for repository settings.
#[derive(Debug, Clone)]
pub struct GhRepo {
    target: Target,
    runner: GhRunner,
}

impl GhRepo {
    /// A new adapter for `target` using default runner.
    #[must_use]
    pub fn new(target: Target) -> Self {
        Self {
            target,
            runner: GhRunner::default(),
        }
    }

    /// A new adapter with a specific runner (useful for tests).
    #[must_use]
    pub const fn with_runner(target: Target, runner: GhRunner) -> Self {
        Self { target, runner }
    }

    /// The target of this adapter.
    #[must_use]
    pub const fn target(&self) -> &Target {
        &self.target
    }

    /// Fetches the current repository settings.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the GitHub CLI call fails or the response cannot be parsed.
    pub fn get_settings(&self) -> Result<RepoSettings, GhError> {
        let endpoint = format!("repos/{}", self.target.full_name());
        let output = self.runner.rest_get(&endpoint)?;
        serde_json::from_str(&output).map_err(|error| GhError::BadReply {
            reason: format!("failed to parse repository settings: {error}"),
        })
    }

    /// Updates repository settings to `settings`.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the GitHub CLI call fails.
    pub fn update_settings(&self, settings: &RepoSettings) -> Result<(), GhError> {
        let endpoint = format!("repos/{}", self.target.full_name());
        let payload = serde_json::to_string(settings).map_err(|error| GhError::BadReply {
            reason: format!("failed to serialize repository settings: {error}"),
        })?;
        self.runner.rest_patch(&endpoint, &payload)?;
        Ok(())
    }
}
