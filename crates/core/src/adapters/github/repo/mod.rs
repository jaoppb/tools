//! Reading repository state via GraphQL and REST API, and updating via REST API.

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{parse_repo_labels, parse_repo_milestones};
use crate::adapters::github::project::cursor_fields;
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::application::repo::{RepoPort, RepoSyncError};
pub use crate::domain::repo::{RepoEntities, RepoState};
use crate::domain::repo::{RepoLabel, RepoMilestone, RepoMilestoneSpec, RepoSettings};

/// Communicates with GitHub GraphQL and REST APIs for repository management.
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

    /// Fetches the current repository settings via REST.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails or the response cannot be parsed.
    pub fn get_settings(&self) -> Result<RepoSettings, GhError> {
        let endpoint = format!("repos/{}", self.target.full_name());
        let output = self.runner.rest_get(&endpoint)?;
        serde_json::from_str(&output).map_err(|error| GhError::BadReply {
            reason: format!("failed to parse repository settings: {error}"),
        })
    }

    /// Fetches all repository labels via GraphQL with cursor pagination.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails or the response cannot be parsed.
    pub fn get_labels(&self) -> Result<Vec<RepoLabel>, GhError> {
        let request = query::repo_labels(&self.target);
        paginate(&self.runner, &request, |reply| {
            let page = parse_repo_labels(reply)?;
            Ok((page.next().map(str::to_owned), page.into_labels()))
        })
    }

    /// Fetches all repository milestones via GraphQL with cursor pagination.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails or the response cannot be parsed.
    pub fn get_milestones(&self) -> Result<Vec<RepoMilestone>, GhError> {
        let request = query::repo_milestones(&self.target);
        paginate(&self.runner, &request, |reply| {
            let page = parse_repo_milestones(reply)?;
            Ok((page.next().map(str::to_owned), page.into_milestones()))
        })
    }

    /// Fetches repository state for selected entities.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if any query fails or returns an unreadable reply.
    pub fn get_state(&self, entities: RepoEntities) -> Result<RepoState, GhError> {
        let settings = if entities.settings {
            Some(self.get_settings()?)
        } else {
            None
        };

        let labels = if entities.labels {
            self.get_labels()?
        } else {
            Vec::new()
        };

        let milestones = if entities.milestones {
            self.get_milestones()?
        } else {
            Vec::new()
        };

        Ok(RepoState {
            settings,
            labels,
            milestones,
        })
    }

    /// Updates repository settings to `settings` via REST PATCH.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn update_settings(&self, settings: &RepoSettings) -> Result<(), GhError> {
        let endpoint = format!("repos/{}", self.target.full_name());
        let payload = serde_json::to_string(settings).map_err(|error| GhError::BadReply {
            reason: format!("failed to serialize repository settings: {error}"),
        })?;
        self.runner.rest_patch(&endpoint, &payload)?;
        Ok(())
    }

    /// Creates a repository label via REST POST.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn create_label(&self, label: &RepoLabel) -> Result<(), GhError> {
        let endpoint = format!("repos/{}/labels", self.target.full_name());
        let payload = serde_json::json!({
            "name": label.name,
            "color": label.color,
            "description": label.description,
        });
        self.runner.rest_post(&endpoint, &payload.to_string())?;
        Ok(())
    }

    /// Updates a repository label via REST PATCH.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn update_label(&self, current_name: &str, label: &RepoLabel) -> Result<(), GhError> {
        let encoded_name = percent_encode_path_segment(current_name);
        let endpoint = format!("repos/{}/labels/{encoded_name}", self.target.full_name());
        let payload = serde_json::json!({
            "new_name": label.name,
            "color": label.color,
            "description": label.description,
        });
        self.runner.rest_patch(&endpoint, &payload.to_string())?;
        Ok(())
    }

    /// Deletes a repository label via REST DELETE.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn delete_label(&self, name: &str) -> Result<(), GhError> {
        let encoded_name = percent_encode_path_segment(name);
        let endpoint = format!("repos/{}/labels/{encoded_name}", self.target.full_name());
        self.runner.rest_delete(&endpoint)?;
        Ok(())
    }

    /// Creates a repository milestone via REST POST.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn create_milestone(&self, milestone: &RepoMilestoneSpec) -> Result<(), GhError> {
        let endpoint = format!("repos/{}/milestones", self.target.full_name());
        let payload = milestone_payload(milestone);
        self.runner.rest_post(&endpoint, &payload)?;
        Ok(())
    }

    /// Updates a repository milestone via REST PATCH.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn update_milestone(
        &self,
        number: u64,
        milestone: &RepoMilestoneSpec,
    ) -> Result<(), GhError> {
        let endpoint = format!("repos/{}/milestones/{number}", self.target.full_name());
        let payload = milestone_payload(milestone);
        self.runner.rest_patch(&endpoint, &payload)?;
        Ok(())
    }

    /// Deletes a repository milestone via REST DELETE.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn delete_milestone(&self, number: u64) -> Result<(), GhError> {
        let endpoint = format!("repos/{}/milestones/{number}", self.target.full_name());
        self.runner.rest_delete(&endpoint)?;
        Ok(())
    }
}

fn milestone_payload(milestone: &RepoMilestoneSpec) -> String {
    serde_json::json!({
        "title": milestone.title,
        "description": milestone.description.as_deref().unwrap_or(""),
        "state": milestone.state.to_string(),
    })
    .to_string()
}

impl RepoPort for GhRepo {
    fn get_state(&self, entities: RepoEntities) -> Result<RepoState, RepoSyncError> {
        self.get_state(entities)
            .map_err(|err| RepoSyncError::Read(err.to_string()))
    }

    fn update_settings(&self, settings: &RepoSettings) -> Result<(), RepoSyncError> {
        self.update_settings(settings).map_err(|err| {
            RepoSyncError::Write(format!("failed to update repository settings: {err}"))
        })
    }

    fn create_label(&self, label: &RepoLabel) -> Result<(), RepoSyncError> {
        self.create_label(label).map_err(|err| {
            RepoSyncError::Write(format!("failed to create label '{}': {err}", label.name))
        })
    }

    fn update_label(&self, current_name: &str, label: &RepoLabel) -> Result<(), RepoSyncError> {
        self.update_label(current_name, label).map_err(|err| {
            RepoSyncError::Write(format!("failed to update label '{}': {err}", label.name))
        })
    }

    fn delete_label(&self, name: &str) -> Result<(), RepoSyncError> {
        self.delete_label(name).map_err(|err| {
            RepoSyncError::Write(format!("failed to delete label '{name}': {err}"))
        })
    }

    fn create_milestone(&self, milestone: &RepoMilestoneSpec) -> Result<(), RepoSyncError> {
        self.create_milestone(milestone).map_err(|err| {
            RepoSyncError::Write(format!(
                "failed to create milestone '{}': {err}",
                milestone.title
            ))
        })
    }

    fn update_milestone(
        &self,
        number: u64,
        milestone: &RepoMilestoneSpec,
    ) -> Result<(), RepoSyncError> {
        self.update_milestone(number, milestone).map_err(|err| {
            RepoSyncError::Write(format!(
                "failed to update milestone '{}': {err}",
                milestone.title
            ))
        })
    }

    fn delete_milestone(&self, number: u64) -> Result<(), RepoSyncError> {
        self.delete_milestone(number).map_err(|err| {
            RepoSyncError::Write(format!("failed to delete milestone {number}: {err}"))
        })
    }
}

fn percent_encode_path_segment(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn paginate<T>(
    runner: &GhRunner,
    query: &str,
    mut extract_page: impl FnMut(&str) -> Result<(Option<String>, Vec<T>), GhError>,
) -> Result<Vec<T>, GhError> {
    let mut items = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let reply = runner.graphql(query, &cursor_fields(cursor.as_deref()))?;
        let (next_cursor, page_items) = extract_page(&reply)?;
        items.extend(page_items);
        if let Some(next) = next_cursor {
            cursor = Some(next);
        } else {
            break;
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encodes_spaces_and_special_characters() {
        assert_eq!(
            percent_encode_path_segment("good first issue"),
            "good%20first%20issue"
        );
        assert_eq!(percent_encode_path_segment("type:feat"), "type%3Afeat");
        assert_eq!(
            percent_encode_path_segment("simple-name_1.0"),
            "simple-name_1.0"
        );
    }
}
