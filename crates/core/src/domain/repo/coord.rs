//! Repository coordinate: owner and name.

use std::fmt;
use std::sync::Arc;

/// A GitHub repository identified by owner and repository name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Repo {
    owner: Arc<str>,
    name: Arc<str>,
}

impl Repo {
    /// Creates a repository coordinate with owner and name.
    #[must_use]
    pub fn new(owner: impl AsRef<str>, name: impl AsRef<str>) -> Self {
        Self {
            owner: Arc::from(owner.as_ref()),
            name: Arc::from(name.as_ref()),
        }
    }

    /// The login of the repository owner.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The name of the repository without owner.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The repository as `owner/name`.
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// Parses a repository coordinate from an `owner/name` string.
    #[must_use]
    pub fn parse(full_name: &str) -> Option<Self> {
        let (owner, name) = full_name.split_once('/')?;
        Some(Self::new(owner, name))
    }
}

impl std::str::FromStr for Repo {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn repo_accessors_and_display() {
        let repo = Repo::new("jaoppb", "tools");
        assert_eq!(repo.owner(), "jaoppb");
        assert_eq!(repo.name(), "tools");
        assert_eq!(repo.full_name(), "jaoppb/tools");
        assert_eq!(repo.to_string(), "tools");
    }

    #[test]
    fn repo_parse() {
        let repo = Repo::parse("jaoppb/tools").expect("valid repo string");
        assert_eq!(repo.owner(), "jaoppb");
        assert_eq!(repo.name(), "tools");
        assert_eq!(Repo::parse("invalid"), None);
    }

    #[test]
    fn repo_equality_and_hashing() {
        let repo1 = Repo::new("jaoppb", "tools");
        let repo2 = Repo::new("jaoppb", "tools");
        let repo3 = Repo::new("jaoppb", "cosmo");
        let repo4 = Repo::new("other", "tools");

        assert_eq!(repo1, repo2);
        assert_ne!(repo1, repo3);
        assert_ne!(repo1, repo4);

        let mut set = HashSet::new();
        set.insert(repo1.clone());
        set.insert(repo2);
        set.insert(repo3);
        set.insert(repo4);

        assert_eq!(set.len(), 3);
        assert!(set.contains(&repo1));
    }

    #[test]
    fn repo_ordering() {
        let r1 = Repo::new("a", "repo");
        let r2 = Repo::new("b", "repo");
        let r3 = Repo::new("b", "repo2");

        assert!(r1 < r2);
        assert!(r2 < r3);
    }
}
