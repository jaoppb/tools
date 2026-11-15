//! Reading the releases and tags of an action's repository through `gh`.

use serde_json::Value;

use crate::adapters::github::error::GhError;
use crate::adapters::github::runner::GhRunner;
use crate::application::{ActionsError, ActionsPort};
use crate::domain::actions::{RepoId, Versions};

/// Reads action versions with `gh api`.
#[derive(Debug, Clone, Default)]
pub struct GhActions {
    runner: GhRunner,
}

impl GhActions {
    /// An adapter with a specific runner (useful for tests).
    #[must_use]
    pub const fn with_runner(runner: GhRunner) -> Self {
        Self { runner }
    }

    fn latest_release(&self, repo: &RepoId) -> Result<Option<String>, GhError> {
        match self
            .runner
            .rest_get(&format!("repos/{repo}/releases/latest"))
        {
            Ok(text) => Ok(serde_json::from_str::<Value>(&text)
                .map_err(bad_reply)?
                .get("tag_name")
                .and_then(Value::as_str)
                .map(str::to_owned)),
            // A repository that publishes only tags has no latest release.
            Err(GhError::Failed { reason }) if reason.contains("HTTP 404") => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn tags(&self, repo: &RepoId) -> Result<Vec<(String, String)>, GhError> {
        let text = self
            .runner
            .rest_get_pages(&format!("repos/{repo}/tags?per_page=100"))?;
        let pages: Vec<Vec<Value>> = serde_json::from_str(&text).map_err(bad_reply)?;
        Ok(pages
            .iter()
            .flatten()
            .filter_map(|tag| {
                let name = tag.get("name")?.as_str()?;
                let sha = tag.get("commit")?.get("sha")?.as_str()?;
                Some((name.to_owned(), sha.to_owned()))
            })
            .collect())
    }
}

fn bad_reply(error: serde_json::Error) -> GhError {
    GhError::BadReply {
        reason: error.to_string(),
    }
}

impl ActionsPort for GhActions {
    fn versions(&self, repo: &RepoId) -> Result<Versions, ActionsError> {
        let read = |error: GhError| ActionsError::Read {
            repo: repo.to_string(),
            reason: error.to_string(),
        };
        Ok(Versions {
            latest_release: self.latest_release(repo).map_err(read)?,
            tags: self.tags(repo).map_err(read)?,
        })
    }
}
