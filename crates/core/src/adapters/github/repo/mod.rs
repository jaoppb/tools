//! Reading repository state via GraphQL and REST API, and updating via REST API.

use serde::Deserialize;

use crate::adapters::github::error::GhError;
use crate::adapters::github::project::cursor_fields;
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::config::MilestoneState;
use crate::domain::repo::{RepoLabel, RepoMilestone, RepoSettings};

/// Selection of entity types to query or reconcile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoEntities {
    /// Whether repository settings are selected.
    pub settings: bool,
    /// Whether repository labels are selected.
    pub labels: bool,
    /// Whether repository milestones are selected.
    pub milestones: bool,
}

impl Default for RepoEntities {
    fn default() -> Self {
        Self::ALL
    }
}

impl RepoEntities {
    /// Select all entities (settings, labels, milestones).
    pub const ALL: Self = Self {
        settings: true,
        labels: true,
        milestones: true,
    };

    /// Whether any entity is selected.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.settings || self.labels || self.milestones
    }
}

/// Consolidated repository state read from GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoState {
    /// Current repository settings, if queried.
    pub settings: Option<RepoSettings>,
    /// Current repository labels, if queried.
    pub labels: Vec<RepoLabel>,
    /// Current repository milestones, if queried.
    pub milestones: Vec<RepoMilestone>,
}

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
        let mut labels = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let request = query::repo_labels(&self.target);
            let reply = self
                .runner
                .graphql(&request, &cursor_fields(cursor.as_deref()))?;
            let parsed: LabelsResponse = serde_json::from_str(&reply).map_err(|error| {
                GhError::BadReply {
                    reason: format!("failed to parse repository labels: {error}"),
                }
            })?;

            let conn = parsed
                .data
                .and_then(|d| d.repository)
                .and_then(|r| r.labels)
                .ok_or_else(|| GhError::BadReply {
                    reason: "missing labels connection in GraphQL reply".into(),
                })?;

            for node in conn.nodes {
                labels.push(RepoLabel::new(
                    node.name,
                    &node.color,
                    node.description.unwrap_or_default(),
                ));
            }

            if conn.page_info.has_next_page {
                cursor = conn.page_info.end_cursor;
                if cursor.is_none() {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(labels)
    }

    /// Fetches all repository milestones via GraphQL with cursor pagination.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails or the response cannot be parsed.
    pub fn get_milestones(&self) -> Result<Vec<RepoMilestone>, GhError> {
        let mut milestones = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let request = query::repo_milestones(&self.target);
            let reply = self
                .runner
                .graphql(&request, &cursor_fields(cursor.as_deref()))?;
            let parsed: MilestonesResponse = serde_json::from_str(&reply).map_err(|error| {
                GhError::BadReply {
                    reason: format!("failed to parse repository milestones: {error}"),
                }
            })?;

            let conn = parsed
                .data
                .and_then(|d| d.repository)
                .and_then(|r| r.milestones)
                .ok_or_else(|| GhError::BadReply {
                    reason: "missing milestones connection in GraphQL reply".into(),
                })?;

            for node in conn.nodes {
                milestones.push(RepoMilestone {
                    number: Some(node.number),
                    title: node.title,
                    description: node.description,
                    state: node.state,
                });
            }

            if conn.page_info.has_next_page {
                cursor = conn.page_info.end_cursor;
                if cursor.is_none() {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(milestones)
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
    pub fn update_label(&self, label: &RepoLabel) -> Result<(), GhError> {
        let endpoint = format!("repos/{}/labels/{}", self.target.full_name(), label.name);
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
        let endpoint = format!("repos/{}/labels/{name}", self.target.full_name());
        self.runner.rest_delete(&endpoint)?;
        Ok(())
    }

    /// Creates a repository milestone via REST POST.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn create_milestone(&self, milestone: &RepoMilestone) -> Result<(), GhError> {
        let endpoint = format!("repos/{}/milestones", self.target.full_name());
        let mut payload = serde_json::Map::new();
        payload.insert("title".into(), serde_json::Value::String(milestone.title.clone()));
        if let Some(ref desc) = milestone.description {
            payload.insert("description".into(), serde_json::Value::String(desc.clone()));
        }
        payload.insert(
            "state".into(),
            serde_json::Value::String(milestone.state.to_string()),
        );

        self.runner.rest_post(&endpoint, &serde_json::Value::Object(payload).to_string())?;
        Ok(())
    }

    /// Updates a repository milestone via REST PATCH.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails.
    pub fn update_milestone(&self, number: u64, milestone: &RepoMilestone) -> Result<(), GhError> {
        let endpoint = format!("repos/{}/milestones/{number}", self.target.full_name());
        let mut payload = serde_json::Map::new();
        payload.insert("title".into(), serde_json::Value::String(milestone.title.clone()));
        if let Some(ref desc) = milestone.description {
            payload.insert("description".into(), serde_json::Value::String(desc.clone()));
        }
        payload.insert(
            "state".into(),
            serde_json::Value::String(milestone.state.to_string()),
        );

        self.runner.rest_patch(&endpoint, &serde_json::Value::Object(payload).to_string())?;
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

// ---------------------------------------------------------------------------
// Serde helper models for GraphQL deserialization
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct LabelsResponse {
    data: Option<LabelsData>,
}

#[derive(Debug, Deserialize)]
struct LabelsData {
    repository: Option<LabelsRepo>,
}

#[derive(Debug, Deserialize)]
struct LabelsRepo {
    labels: Option<LabelsConnection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabelsConnection {
    page_info: PageInfo,
    nodes: Vec<LabelNode>,
}

#[derive(Debug, Deserialize)]
struct LabelNode {
    name: String,
    color: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MilestonesResponse {
    data: Option<MilestonesData>,
}

#[derive(Debug, Deserialize)]
struct MilestonesData {
    repository: Option<MilestonesRepo>,
}

#[derive(Debug, Deserialize)]
struct MilestonesRepo {
    milestones: Option<MilestonesConnection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MilestonesConnection {
    page_info: PageInfo,
    nodes: Vec<MilestoneNode>,
}

#[derive(Debug, Deserialize)]
struct MilestoneNode {
    number: u64,
    title: String,
    description: Option<String>,
    state: MilestoneState,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_labels_graphql_response() {
        let json = r##"{
            "data": {
                "repository": {
                    "labels": {
                        "pageInfo": {
                            "hasNextPage": false,
                            "endCursor": null
                        },
                        "nodes": [
                            {
                                "name": "type:feat",
                                "color": "1d76db",
                                "description": "New feature"
                            },
                            {
                                "name": "area:core",
                                "color": "#2EA44F",
                                "description": null
                            }
                        ]
                    }
                }
            }
        }"##;

        let parsed: LabelsResponse = serde_json::from_str(json).unwrap();
        let conn = parsed.data.unwrap().repository.unwrap().labels.unwrap();
        assert!(!conn.page_info.has_next_page);
        assert_eq!(conn.nodes.len(), 2);
        assert_eq!(conn.nodes[0].name, "type:feat");
        assert_eq!(conn.nodes[1].color, "#2EA44F");
    }

    #[test]
    fn parses_milestones_graphql_response_with_case_insensitivity() {
        let json = r##"{
            "data": {
                "repository": {
                    "milestones": {
                        "pageInfo": {
                            "hasNextPage": true,
                            "endCursor": "cursor123"
                        },
                        "nodes": [
                            {
                                "number": 1,
                                "title": "v0",
                                "description": "Bootstrap",
                                "state": "OPEN"
                            },
                            {
                                "number": 2,
                                "title": "v1",
                                "description": null,
                                "state": "closed"
                            }
                        ]
                    }
                }
            }
        }"##;

        let parsed: MilestonesResponse = serde_json::from_str(json).unwrap();
        let conn = parsed.data.unwrap().repository.unwrap().milestones.unwrap();
        assert!(conn.page_info.has_next_page);
        assert_eq!(conn.page_info.end_cursor.as_deref(), Some("cursor123"));
        assert_eq!(conn.nodes.len(), 2);
        assert_eq!(conn.nodes[0].state, MilestoneState::Open);
        assert_eq!(conn.nodes[1].state, MilestoneState::Closed);
    }
}
