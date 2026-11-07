//! Which Project and repository (or repositories) the tools work on.

use crate::adapters::github::GhError;
use crate::domain::Repo;

/// The target of GitHub operations: owner, repositories, and an optional project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    owner: String,
    repositories: Vec<Repo>,
    project: Option<u32>,
}

impl Target {
    /// Creates a single-repository target.
    #[must_use]
    pub fn single(
        owner: impl Into<String>,
        repository: impl Into<String>,
        project: Option<u32>,
    ) -> Self {
        let owner_str = owner.into();
        let repo_str = repository.into();
        let repo = Repo::new(&owner_str, &repo_str);
        Self {
            owner: owner_str,
            repositories: vec![repo],
            project,
        }
    }

    /// Creates a multi-repository target.
    #[must_use]
    pub fn multi(
        owner: impl Into<String>,
        repositories: Vec<String>,
        project: Option<u32>,
    ) -> Self {
        let owner_str = owner.into();
        let repos = repositories
            .into_iter()
            .map(|name| {
                if let Some(repo) = Repo::parse(&name) {
                    repo
                } else {
                    Repo::new(&owner_str, name)
                }
            })
            .collect();
        Self {
            owner: owner_str,
            repositories: repos,
            project,
        }
    }

    /// A target with these coordinates (for single repository).
    #[must_use]
    pub fn new(
        owner: impl Into<String>,
        repository: impl Into<String>,
        project: Option<u32>,
    ) -> Self {
        Self::single(owner, repository, project)
    }

    /// The login of the Project's owner.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
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
        self.project().ok_or(GhError::MissingProject)
    }

    /// Returns the target repositories as a borrowed slice.
    #[must_use]
    pub fn repos(&self) -> &[Repo] {
        &self.repositories
    }

    /// Returns the domain repository coordinates in this target.
    #[must_use]
    pub fn repo_coords(&self) -> Vec<Repo> {
        self.repositories.clone()
    }

    /// Whether this target includes the given repository name or full name (`owner/repo`).
    #[must_use]
    pub fn contains_repo(&self, repo: &str) -> bool {
        let repo = repo.trim();
        if let Some(parsed) = Repo::parse(repo) {
            self.repositories.contains(&parsed)
        } else {
            self.repositories.iter().any(|r| r.name() == repo)
        }
    }

    /// The primary repository name if single, or the first configured repository if multi.
    #[must_use]
    pub fn primary_repo(&self) -> &str {
        self.repositories.first().map_or("", Repo::name)
    }

    /// The repository coordinate for the primary repository.
    #[must_use]
    pub fn primary_repo_coord(&self) -> Option<&Repo> {
        self.repositories.first()
    }

    /// The repository name for a single target, or primary repo name.
    #[must_use]
    pub fn repository(&self) -> &str {
        self.primary_repo()
    }

    /// The full name `owner/repo` for single target (or primary repository).
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner(), self.repository())
    }

    /// Sets or overrides the owner.
    #[must_use]
    pub fn with_owner(mut self, owner: impl Into<String>) -> Self {
        self.owner = owner.into();
        self
    }

    /// Sets or overrides the project.
    #[must_use]
    pub fn with_project(mut self, project: Option<u32>) -> Self {
        self.project = project;
        self
    }
}
