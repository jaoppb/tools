//! Status mapping between GitHub Project columns and canonical [`Status`].

use std::collections::BTreeMap;

use crate::domain::issue::Status;

/// An error creating a [`StatusMapping`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "unknown canonical status '{0}'; valid statuses are Backlog, Ready, In progress, In review, Done"
)]
pub struct StatusMappingError(pub String);

/// Configurable mapping between external Project column names and canonical [`Status`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusMapping {
    /// Normalized (lowercase, trimmed) alias -> canonical Status.
    aliases: BTreeMap<String, Status>,
}

impl StatusMapping {
    /// An empty mapping with only standard case-insensitive status matching.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            aliases: BTreeMap::new(),
        }
    }

    /// Creates a mapping from an iterator of (column_name, canonical_status_name) pairs.
    ///
    /// # Errors
    ///
    /// Returns [`StatusMappingError`] if a target status name is not a recognized canonical status.
    pub fn from_pairs<K, V, I>(pairs: I) -> Result<Self, StatusMappingError>
    where
        K: AsRef<str>,
        V: AsRef<str>,
        I: IntoIterator<Item = (K, V)>,
    {
        let mut aliases = BTreeMap::new();

        for (col, target) in pairs {
            let col_trimmed = col.as_ref().trim();
            if col_trimmed.is_empty() {
                continue;
            }
            let target_trimmed = target.as_ref().trim();
            let status = Status::from_name_case_insensitive(target_trimmed)
                .ok_or_else(|| StatusMappingError(target_trimmed.to_owned()))?;

            aliases.insert(col_trimmed.to_ascii_lowercase(), status);
        }

        Ok(Self { aliases })
    }

    /// Resolves an external column name to a canonical [`Status`].
    ///
    /// Checks configured aliases first, then falls back to case-insensitive standard status names.
    #[must_use]
    pub fn status_for(&self, name: &str) -> Option<Status> {
        let trimmed = name.trim();
        let lower = trimmed.to_ascii_lowercase();

        if let Some(&status) = self.aliases.get(&lower) {
            return Some(status);
        }

        Status::from_name_case_insensitive(trimmed)
    }
}
