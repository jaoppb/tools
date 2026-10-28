//! Reading repository state via GraphQL and REST API, and updating via REST API.

use crate::adapters::github::cursor_fields;
use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{
    parse_repo_labels, parse_repo_milestones, parse_repo_settings,
};
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::application::repo::{RepoPort, RepoSyncError};
use crate::domain::repo::{
    LabelDiff, MilestoneDiff, RepoLabel, RepoMilestone, RepoMilestoneSpec, RepoSettings, RepoSpec,
    RepoState, SettingDiff,
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

    fn label_endpoint(&self, name: &str) -> String {
        let encoded_name = percent_encode_path_segment(name);
        self.repo_endpoint(&format!("labels/{encoded_name}"))
    }

    fn milestone_endpoint(&self, number: u64) -> String {
        self.repo_endpoint(&format!("milestones/{number}"))
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
            Ok(page.into_parts())
        })
    }

    fn get_milestones(&self) -> Result<Vec<RepoMilestone>, GhError> {
        let request = query::repo_milestones(&self.target);
        paginate(&self.runner, &request, |reply| {
            let page = parse_repo_milestones(reply)?;
            Ok(page.into_parts())
        })
    }
}

impl RepoPort for GhRepo {
    fn get_state(&self, spec: &RepoSpec) -> Result<RepoState, RepoSyncError> {
        let settings = spec
            .settings
            .as_ref()
            .map(|_| self.get_settings())
            .transpose()
            .map_err(|err| RepoSyncError::Read(err.to_string()))?;

        let labels = spec
            .labels
            .as_ref()
            .map(|_| self.get_labels())
            .transpose()
            .map_err(|err| RepoSyncError::Read(err.to_string()))?;

        let milestones = spec
            .milestones
            .as_ref()
            .map(|_| self.get_milestones())
            .transpose()
            .map_err(|err| RepoSyncError::Read(err.to_string()))?;

        Ok(RepoState {
            settings,
            labels,
            milestones,
        })
    }

    fn update_settings(&self, settings: &[SettingDiff]) -> Result<(), RepoSyncError> {
        let endpoint = self.repo_endpoint("");
        let payload = settings_patch_payload(settings);
        self.runner.rest_patch(&endpoint, &payload).map_err(|err| {
            RepoSyncError::Write(format!("failed to update repository settings: {err}"))
        })?;
        Ok(())
    }

    fn apply_label(&self, diff: &LabelDiff) -> Result<(), RepoSyncError> {
        match diff {
            LabelDiff::Create(label) => self.create_label(label),
            LabelDiff::Update { current, desired } => self.update_label(&current.name, desired),
            LabelDiff::Delete { name, .. } => self.delete_label(name),
            LabelDiff::Unmanaged { .. } => Ok(()),
        }
    }

    fn apply_milestone(&self, diff: &MilestoneDiff) -> Result<(), RepoSyncError> {
        match diff {
            MilestoneDiff::Create(milestone) => self.create_milestone(milestone),
            MilestoneDiff::Update { current, desired } => {
                self.update_milestone(current.number, desired)
            }
            MilestoneDiff::Delete { number, .. } => self.delete_milestone(*number),
            MilestoneDiff::Unmanaged { .. } => Ok(()),
        }
    }
}

impl GhRepo {
    fn create_label(&self, label: &RepoLabel) -> Result<(), RepoSyncError> {
        let endpoint = self.repo_endpoint("labels");
        let payload = serde_json::to_string(label).map_err(|error| {
            RepoSyncError::Write(format!(
                "failed to serialize label '{}': {error}",
                label.name
            ))
        })?;
        self.runner.rest_post(&endpoint, &payload).map_err(|err| {
            RepoSyncError::Write(format!("failed to create label '{}': {err}", label.name))
        })?;
        Ok(())
    }

    fn update_label(&self, current_name: &str, label: &RepoLabel) -> Result<(), RepoSyncError> {
        let endpoint = self.label_endpoint(current_name);
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
        let endpoint = self.label_endpoint(name);
        self.runner.rest_delete(&endpoint).map_err(|err| {
            RepoSyncError::Write(format!("failed to delete label '{name}': {err}"))
        })?;
        Ok(())
    }

    fn create_milestone(&self, milestone: &RepoMilestoneSpec) -> Result<(), RepoSyncError> {
        let endpoint = self.repo_endpoint("milestones");
        let payload = serde_json::to_string(milestone).map_err(|error| {
            RepoSyncError::Write(format!(
                "failed to serialize milestone '{}': {error}",
                milestone.title
            ))
        })?;
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
        let endpoint = self.milestone_endpoint(number);
        let payload = serde_json::to_string(milestone).map_err(|error| {
            RepoSyncError::Write(format!(
                "failed to serialize milestone '{}': {error}",
                milestone.title
            ))
        })?;
        self.runner.rest_patch(&endpoint, &payload).map_err(|err| {
            RepoSyncError::Write(format!(
                "failed to update milestone '{}': {err}",
                milestone.title
            ))
        })?;
        Ok(())
    }

    fn delete_milestone(&self, number: u64) -> Result<(), RepoSyncError> {
        let endpoint = self.milestone_endpoint(number);
        self.runner.rest_delete(&endpoint).map_err(|err| {
            RepoSyncError::Write(format!("failed to delete milestone {number}: {err}"))
        })?;
        Ok(())
    }
}

fn settings_patch_payload(diffs: &[SettingDiff]) -> String {
    let mut map = serde_json::Map::new();
    for diff in diffs {
        match diff {
            SettingDiff::AllowMergeCommit { desired, .. } => {
                map.insert("allow_merge_commit".into(), (*desired).into());
            }
            SettingDiff::AllowSquashMerge { desired, .. } => {
                map.insert("allow_squash_merge".into(), (*desired).into());
            }
            SettingDiff::AllowRebaseMerge { desired, .. } => {
                map.insert("allow_rebase_merge".into(), (*desired).into());
            }
            SettingDiff::DeleteBranchOnMerge { desired, .. } => {
                map.insert("delete_branch_on_merge".into(), (*desired).into());
            }
            SettingDiff::MergeCommitTitle { desired, .. } => {
                map.insert("merge_commit_title".into(), desired.to_string().into());
            }
            SettingDiff::MergeCommitMessage { desired, .. } => {
                map.insert("merge_commit_message".into(), desired.to_string().into());
            }
        }
    }
    serde_json::Value::Object(map).to_string()
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
    use crate::domain::repo::{MergeCommitMessage, MergeCommitTitle};

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

    #[test]
    fn settings_patch_payload_serializes_only_differing_fields() {
        let diffs = vec![
            SettingDiff::AllowSquashMerge {
                current: true,
                desired: false,
            },
            SettingDiff::DeleteBranchOnMerge {
                current: false,
                desired: true,
            },
            SettingDiff::MergeCommitTitle {
                current: MergeCommitTitle::MergeMessage,
                desired: MergeCommitTitle::PrTitle,
            },
            SettingDiff::MergeCommitMessage {
                current: MergeCommitMessage::PrTitle,
                desired: MergeCommitMessage::PrBody,
            },
        ];

        let payload_str = settings_patch_payload(&diffs);
        let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap();
        assert_eq!(payload["allow_squash_merge"], false);
        assert_eq!(payload["delete_branch_on_merge"], true);
        assert_eq!(payload["merge_commit_title"], "PR_TITLE");
        assert_eq!(payload["merge_commit_message"], "PR_BODY");
        assert!(payload.get("allow_merge_commit").is_none());
    }
}
