//! Auditing the `uses:` references of workflows against the rules of `tools.toml`.

use std::collections::BTreeMap;

use crate::domain::actions::{Finding, Reference, RepoId, Rules, Versions, judge};

/// Port for reading what GitHub knows about the versions of an action's repository.
pub trait ActionsPort {
    /// The versions of `repo`.
    ///
    /// # Errors
    ///
    /// Returns [`ActionsError`] when GitHub cannot be read.
    fn versions(&self, repo: &RepoId) -> Result<Versions, ActionsError>;
}

/// Why the audit could not finish.
#[derive(Debug, thiserror::Error)]
pub enum ActionsError {
    /// GitHub could not be read for a repository.
    #[error("cannot read the versions of {repo}: {reason}")]
    Read {
        /// The repository.
        repo: String,
        /// What went wrong, in the words of the adapter.
        reason: String,
    },
}

/// Judges `references` under `rules`, asking the port once per repository.
///
/// # Errors
///
/// Returns [`ActionsError`] when a repository's versions cannot be read.
pub fn audit(
    port: &impl ActionsPort,
    rules: &Rules,
    references: &[Reference],
) -> Result<Vec<Finding>, ActionsError> {
    let mut known: BTreeMap<RepoId, Versions> = BTreeMap::new();
    let mut findings = Vec::new();
    for reference in references {
        if rules.needs_versions()
            && !rules.is_ignored(&reference.repo)
            && !known.contains_key(&reference.repo)
        {
            known.insert(reference.repo.clone(), port.versions(&reference.repo)?);
        }
        findings.extend(judge(rules, reference, known.get(&reference.repo)));
    }
    Ok(findings)
}
