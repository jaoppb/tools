//! The wire shapes of one page of Project items, as GraphQL prints them.
//!
//! Everything here is visible to the parent module only, which turns the
//! shapes into domain values.

use serde::Deserialize;

use crate::adapters::github::parse::{PageCursor, RepositoryRef};
use crate::adapters::github::target::Target;
use crate::domain::Repo;

#[derive(Debug, Deserialize)]
pub(super) struct Reply {
    pub(super) data: Answer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Answer {
    #[serde(alias = "user", alias = "organization")]
    pub(super) repository_owner: Option<Owner>,
}

impl Answer {
    pub(super) fn project_v2(self) -> Option<Project> {
        self.repository_owner.and_then(|owner| owner.project_v2)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Owner {
    pub(super) project_v2: Option<Project>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Project {
    pub(super) items: Items,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Items {
    pub(super) page_info: PageCursor,
    pub(super) nodes: Vec<Item>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Item {
    pub(super) field_values: FieldValues,
    pub(super) content: Option<Content>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FieldValues {
    pub(super) nodes: Vec<FieldValue>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FieldValue {
    pub(super) name: Option<String>,
    pub(super) number: Option<serde_json::Number>,
    pub(super) field: Option<FieldName>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FieldName {
    pub(super) name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Content {
    #[serde(flatten)]
    pub(super) head: Head,
    #[serde(flatten)]
    pub(super) detail: Detail,
}

#[derive(Debug, Deserialize)]
pub(super) struct Head {
    pub(super) number: Option<u64>,
    pub(super) state: Option<String>,
    pub(super) repository: Option<RepositoryRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Detail {
    pub(super) labels: Option<Names>,
    pub(super) milestone: Option<Milestoned>,
    pub(super) parent: Option<Parented>,
    pub(super) blocked_by: Option<BlockedBy>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Names {
    pub(super) nodes: Vec<Named>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Named {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Milestoned {
    pub(super) title: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Parented {
    pub(super) number: u64,
    pub(super) labels: Names,
}

#[derive(Debug, Deserialize)]
pub(super) struct BlockedBy {
    pub(super) nodes: Vec<Blocking>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Blocking {
    #[serde(flatten)]
    pub(super) head: Head,
}

impl Head {
    pub(super) fn is_in(&self, target: &Target) -> bool {
        self.repository
            .as_ref()
            .is_some_and(|repository| target.contains_repo(&repository.name_with_owner))
    }

    pub(super) fn repo(&self) -> Option<Repo> {
        self.repository.as_ref().and_then(RepositoryRef::to_repo)
    }
}

impl FieldValue {
    pub(super) fn is_field(&self, name: &str) -> bool {
        self.field.as_ref().and_then(|field| field.name.as_deref()) == Some(name)
    }
}

impl FieldValues {
    pub(super) fn select(&self, name: &str) -> Option<&str> {
        self.nodes
            .iter()
            .filter(|value| value.is_field(name))
            .find_map(|value| value.name.as_deref())
    }

    pub(super) fn whole_number(&self, name: &str) -> Option<u8> {
        self.nodes
            .iter()
            .filter(|value| value.is_field(name))
            .find_map(|value| whole(value.number.as_ref()?))
    }
}

/// GraphQL numbers are floats, so a whole Effort arrives as `5.0`. A number
/// that is not a whole value from 0 to 255 is read as absent.
fn whole(number: &serde_json::Number) -> Option<u8> {
    let text = number.to_string();
    text.strip_suffix(".0").unwrap_or(&text).parse().ok()
}
