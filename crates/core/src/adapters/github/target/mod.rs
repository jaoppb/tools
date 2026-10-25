//! Which Project of which repository the sync works on.

use crate::adapters::github::GhError;

/// The owner, repository and Project number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    owner: String,
    repository: String,
    project: Option<u32>,
}

impl Target {
    /// A target with these coordinates.
    #[must_use]
    pub fn new(
        owner: impl Into<String>,
        repository: impl Into<String>,
        project: Option<u32>,
    ) -> Self {
        Self {
            owner: owner.into(),
            repository: repository.into(),
            project,
        }
    }

    /// The Cosmo Project: project 4 of `jaoppb`, linked to `jaoppb/cosmo`.
    #[must_use]
    pub fn cosmo() -> Self {
        Self::new("jaoppb", "cosmo", Some(4))
    }

    /// The login of the Project's owner.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The repository name, without its owner.
    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// The Project number, if configured.
    #[must_use]
    pub const fn project(&self) -> Option<u32> {
        self.project
    }

    /// Returns the project number or fails with [`GhError::MissingProject`].
    ///
    /// # Errors
    ///
    /// Fails when `project` is `None`.
    pub fn required_project(&self) -> Result<u32, GhError> {
        self.project.ok_or(GhError::MissingProject)
    }

    /// The repository as `owner/name`.
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repository)
    }
}
