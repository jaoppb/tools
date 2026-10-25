//! The two lookups a write needs: the item of an issue, and the Status field.

use serde::Deserialize;

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::bad_reply;

/// The ids needed to set a Status: the Project, the field, and its options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusField {
    project: String,
    field: String,
    options: Vec<StatusOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StatusOption {
    #[serde(rename = "id")]
    identifier: String,
    name: String,
}

impl StatusField {
    /// The Project id.
    #[must_use]
    pub fn project(&self) -> &str {
        &self.project
    }

    /// The Status field id.
    #[must_use]
    pub fn field(&self) -> &str {
        &self.field
    }

    /// The id of the option with this name, if the field has it.
    #[must_use]
    pub fn option(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .find(|option| option.name == name)
            .map(|option| option.identifier.as_str())
    }
}

#[derive(Debug, Deserialize)]
struct ItemReply {
    data: ItemAnswer,
}

#[derive(Debug, Deserialize)]
struct ItemAnswer {
    repository: ItemRepository,
}

#[derive(Debug, Deserialize)]
struct ItemRepository {
    issue: Option<ItemIssue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemIssue {
    project_items: ProjectItems,
}

#[derive(Debug, Deserialize)]
struct ProjectItems {
    nodes: Vec<ProjectItem>,
}

#[derive(Debug, Deserialize)]
struct ProjectItem {
    #[serde(rename = "id")]
    identifier: String,
    project: ProjectNumber,
}

#[derive(Debug, Deserialize)]
struct ProjectNumber {
    number: u32,
}

/// Reads the id of the item that puts an issue in this Project.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON.
pub fn parse_item_id(json: &str, project: u32) -> Result<Option<String>, GhError> {
    let reply: ItemReply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    Ok(reply
        .data
        .repository
        .issue
        .into_iter()
        .flat_map(|issue| issue.project_items.nodes)
        .find(|item| item.project.number == project)
        .map(|item| item.identifier))
}

#[derive(Debug, Deserialize)]
struct FieldReply {
    data: FieldAnswer,
}

#[derive(Debug, Deserialize)]
struct FieldAnswer {
    user: FieldUser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FieldUser {
    project_v2: FieldProject,
}

#[derive(Debug, Deserialize)]
struct FieldProject {
    #[serde(rename = "id")]
    identifier: String,
    field: Option<FieldNode>,
}

#[derive(Debug, Deserialize)]
struct FieldNode {
    #[serde(rename = "id")]
    identifier: String,
    options: Vec<StatusOption>,
}

/// Reads the Project id and the Status field with its options.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the answer is not the expected JSON or
/// the Project has no Status field.
pub fn parse_status_field(json: &str) -> Result<StatusField, GhError> {
    let reply: FieldReply = serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let project = reply.data.user.project_v2;
    let field = project.field.ok_or_else(|| GhError::BadReply {
        reason: "the Project has no Status field".to_owned(),
    })?;
    Ok(StatusField {
        project: project.identifier,
        field: field.identifier,
        options: field.options,
    })
}
