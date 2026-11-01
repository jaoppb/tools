//! The wire shapes of one page of the repository's issues, as GraphQL prints them.
//!
//! Everything here is visible to the parent module only, which turns the
//! shapes into domain values.

use serde::Deserialize;

use crate::adapters::github::parse::PageCursor;
use crate::adapters::github::target::Target;
use crate::domain::Repo;

#[derive(Debug, Deserialize)]
pub(super) struct Reply {
    pub(super) data: Answer,
}

#[derive(Debug, Deserialize)]
pub(super) struct Answer {
    pub(super) repository: Repository,
}

#[derive(Debug, Deserialize)]
pub(super) struct Repository {
    pub(super) issues: Issues,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Issues {
    pub(super) total_count: u64,
    pub(super) page_info: PageCursor,
    pub(super) nodes: Vec<Node>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Node {
    #[serde(flatten)]
    pub(super) head: Head,
    #[serde(flatten)]
    pub(super) placed: Placed,
    pub(super) blocked_by: BlockedBy,
}

#[derive(Debug, Deserialize)]
pub(super) struct Head {
    pub(super) number: u64,
    pub(super) state: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Placed {
    pub(super) labels: Names,
    pub(super) milestone: Option<Milestoned>,
    pub(super) parent: Option<Parented>,
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BlockedBy {
    pub(super) total_count: u64,
    pub(super) nodes: Vec<Blocking>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Blocking {
    pub(super) number: u64,
    pub(super) repository: Option<Home>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Home {
    pub(super) name_with_owner: String,
}

impl Home {
    pub(super) fn to_repo(&self) -> Option<Repo> {
        Repo::parse(&self.name_with_owner)
    }
}

impl Blocking {
    pub(super) fn is_in(&self, target: &Target) -> bool {
        self.repository
            .as_ref()
            .is_some_and(|home| target.contains_repo(&home.name_with_owner))
    }

    pub(super) fn repo(&self, current_repo: &Repo) -> Repo {
        self.repository
            .as_ref()
            .and_then(Home::to_repo)
            .unwrap_or_else(|| current_repo.clone())
    }
}
