//! Auditing the `uses:` references of workflows against the rules of `tools.toml`.

use std::collections::BTreeMap;

use crate::domain::actions::{
    Finding, Reference, RepoId, Rules, Versions, judge_static, judge_versions, unreadable_versions,
};

/// Port for reading what GitHub knows about the versions of an action's repository.
pub trait ActionsPort {
    /// The versions of `repo`.
    ///
    /// # Errors
    ///
    /// Returns [`ActionsError`] when GitHub cannot be read.
    fn versions(&self, repo: &RepoId) -> Result<Versions, ActionsError>;
}

/// Why the versions of a repository could not be read.
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

/// Judges `references` under `rules`, asking the port at most once per repository.
///
/// A repository that `allow` or `deny` forbids is not looked up. A repository whose versions
/// cannot be read (deleted, private, or GitHub unreachable) yields a `versions` finding for
/// each of its references, and the audit goes on with the others.
pub fn audit(port: &impl ActionsPort, rules: &Rules, references: &[Reference]) -> Vec<Finding> {
    let mut known: BTreeMap<RepoId, Result<Versions, ActionsError>> = BTreeMap::new();
    let mut findings = Vec::new();
    for reference in references
        .iter()
        .filter(|reference| !rules.is_ignored(&reference.repo))
    {
        findings.extend(judge_static(rules, reference));
        if !rules.needs_versions() || rules.forbids(&reference.repo) {
            continue;
        }
        let versions = known
            .entry(reference.repo.clone())
            .or_insert_with(|| port.versions(&reference.repo));
        match versions {
            Ok(versions) => findings.extend(judge_versions(rules, reference, versions)),
            Err(error) => findings.push(unreadable_versions(reference, &error.to_string())),
        }
    }
    findings
}
