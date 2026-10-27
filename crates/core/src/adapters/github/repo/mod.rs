//! Reading repository state via GraphQL and REST API, and updating via REST API.

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{
    parse_repo_labels, parse_repo_milestones, parse_repo_settings,
};
use crate::adapters::github::project::cursor_fields;
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::application::repo::{RepoPort, RepoSyncError};
use crate::domain::repo::{
    RepoEntities, RepoLabel, RepoMilestone, RepoMilestoneSpec, RepoSettings, RepoState,
};

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

    fn repo_endpoint(&self, subpath: &str) -> String {
        if subpath.is_empty() {
            format!("repos/{}", self.target.full_name())
        } else {
            format!("repos/{}/{subpath}", self.target.full_name())
        }
    }

    fn get_settings(&self) -> Result<RepoSettings, GhError> {
        let endpoint = self.repo_endpoint("");
        let output = self.runner.rest_get(&endpoint)?;
        parse_repo_settings(&output)
    }

    fn get_labels(&self) -> Result<Vec<RepoLabel>, GhError> {
        let request = query::repo_labels(&self.target);
        paginate(&self.runner, &request, |reply| {
            let page = parse_repo_labels(reply)?;
            Ok((page.next().map(str::to_owned), page.into_labels()))
        })
    }

    fn get_milestones(&self) -> Result<Vec<RepoMilestone>, GhError> {
        let request = query::repo_milestones(&self.target);
        paginate(&self.runner, &request, |reply| {
            let page = parse_repo_milestones(reply)?;
            Ok((page.next().map(str::to_owned), page.into_milestones()))
        })
    }
}

impl RepoPort for GhRepo {
    fn get_state(&self, entities: RepoEntities) -> Result<RepoState, RepoSyncError> {
        let settings = if entities.settings {
            Some(
                self.get_settings()
                    .map_err(|err| RepoSyncError::Read(err.to_string()))?,
            )
        } else {
            None
        };

        let labels = if entities.labels {
            Some(
                self.get_labels()
                    .map_err(|err| RepoSyncError::Read(err.to_string()))?,
            )
        } else {
            None
        };

        let milestones = if entities.milestones {
            Some(
                self.get_milestones()
                    .map_err(|err| RepoSyncError::Read(err.to_string()))?,
            )
        } else {
            None
        };

        Ok(RepoState {
            settings,
            labels,
            milestones,
        })
    }

    fn update_settings(&self, settings: &RepoSettings) -> Result<(), RepoSyncError> {
        let endpoint = self.repo_endpoint("");
        let payload = serde_json::to_string(settings).map_err(|error| {
            RepoSyncError::Write(format!("failed to serialize repository settings: {error}"))
        })?;
        self.runner.rest_patch(&endpoint, &payload).map_err(|err| {
            RepoSyncError::Write(format!("failed to update repository settings: {err}"))
        })?;
        Ok(())
    }

    fn create_label(&self, label: &RepoLabel) -> Result<(), RepoSyncError> {
        let endpoint = self.repo_endpoint("labels");
        let payload = serde_json::to_string(label).map_err(|error| {
            RepoSyncError::Write(format!("failed to serialize label '{}': {error}", label.name))
        })?;
        self.runner.rest_post(&endpoint, &payload).map_err(|err| {
            RepoSyncError::Write(format!("failed to create label '{}': {err}", label.name))
        })?;
        Ok(())
    }

    fn update_label(&self, current_name: &str, label: &RepoLabel) -> Result<(), RepoSyncError> {
        let encoded_name = percent_encode_path_segment(current_name);
        let endpoint = self.repo_endpoint(&format!("labels/{encoded_name}"));
        let payload = serde_json::json!({
            "new_name": label.name,
            "color": label.color,
            "description": label.description,
        });
        self.runner
            .rest_patch(&endpoint, &payload.to_string())
            .map_err(|err| {
                RepoSyncError::Write(format!("failed to update label '{}': {err}", label.name))
            })?;
        Ok(())
    }

    fn delete_label(&self, name: &str) -> Result<(), RepoSyncError> {
        let encoded_name = percent_encode_path_segment(name);
        let endpoint = self.repo_endpoint(&format!("labels/{encoded_name}"));
        self.runner.rest_delete(&endpoint).map_err(|err| {
            RepoSyncError::Write(format!("failed to delete label '{name}': {err}"))
        })?;
        Ok(())
    }

    fn create_milestone(&self, milestone: &RepoMilestoneSpec) -> Result<(), RepoSyncError> {
        let endpoint = self.repo_endpoint("milestones");
        let payload = milestone_payload(milestone);
        self.runner.rest_post(&endpoint, &payload).map_err(|err| {
            RepoSyncError::Write(format!(
                "failed to create milestone '{}': {err}",
                milestone.title
            ))
        })?;
        Ok(())
    }

    fn update_milestone(
        &self,
        number: u64,
        milestone: &RepoMilestoneSpec,
    ) -> Result<(), RepoSyncError> {
        let endpoint = self.repo_endpoint(&format!("milestones/{number}"));
        let payload = milestone_payload(milestone);
        self.runner.rest_patch(&endpoint, &payload).map_err(|err| {
            RepoSyncError::Write(format!(
                "failed to update milestone '{}': {err}",
                milestone.title
            ))
        })?;
        Ok(())
    }

    fn delete_milestone(&self, number: u64) -> Result<(), RepoSyncError> {
        let endpoint = self.repo_endpoint(&format!("milestones/{number}"));
        self.runner.rest_delete(&endpoint).map_err(|err| {
            RepoSyncError::Write(format!("failed to delete milestone {number}: {err}"))
        })?;
        Ok(())
    }
}

fn milestone_payload(milestone: &RepoMilestoneSpec) -> String {
    serde_json::json!({
        "title": milestone.title,
        "description": milestone.description,
        "state": milestone.state.to_string(),
    })
    .to_string()
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
