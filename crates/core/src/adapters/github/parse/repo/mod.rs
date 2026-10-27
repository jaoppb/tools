//! One page of the repository's labels or milestones.

use serde::Deserialize;

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{PageCursor, bad_reply};
use crate::domain::repo::{MilestoneState, RepoLabel, RepoMilestone};

/// One page of repository labels, with the cursor of the next page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoLabelsPage {
    labels: Vec<RepoLabel>,
    next: Option<String>,
}

impl RepoLabelsPage {
    /// Takes the labels of this page.
    #[must_use]
    pub fn into_labels(self) -> Vec<RepoLabel> {
        self.labels
    }

    /// The cursor to pass for the next page, if there is one.
    #[must_use]
    pub fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }
}

/// One page of repository milestones, with the cursor of the next page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoMilestonesPage {
    milestones: Vec<RepoMilestone>,
    next: Option<String>,
}

impl RepoMilestonesPage {
    /// Takes the milestones of this page.
    #[must_use]
    pub fn into_milestones(self) -> Vec<RepoMilestone> {
        self.milestones
    }

    /// The cursor to pass for the next page, if there is one.
    #[must_use]
    pub fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }
}

#[derive(Debug, Deserialize)]
struct GraphqlReply<T> {
    data: RepositoryReply<T>,
}

#[derive(Debug, Deserialize)]
struct RepositoryReply<T> {
    repository: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Connection<T> {
    page_info: PageCursor,
    nodes: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct LabelsField {
    labels: Connection<LabelNode>,
}

#[derive(Debug, Deserialize)]
struct LabelNode {
    name: String,
    color: String,
    description: Option<String>,
}

/// Reads one page of repository labels.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the reply cannot be parsed.
pub fn parse_repo_labels(json: &str) -> Result<RepoLabelsPage, GhError> {
    let reply: GraphqlReply<LabelsField> =
        serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let conn = reply.data.repository.labels;
    let next = conn.page_info.next();
    let labels = conn
        .nodes
        .into_iter()
        .map(|node| RepoLabel::new(node.name, &node.color, node.description.unwrap_or_default()))
        .collect();
    Ok(RepoLabelsPage { labels, next })
}

#[derive(Debug, Deserialize)]
struct MilestonesField {
    milestones: Connection<MilestoneNode>,
}

#[derive(Debug, Deserialize)]
struct MilestoneNode {
    number: u64,
    title: String,
    description: Option<String>,
    state: String,
}

/// Reads one page of repository milestones.
///
/// # Errors
///
/// Returns [`GhError::BadReply`] when the reply cannot be parsed or contains an unknown state.
pub fn parse_repo_milestones(json: &str) -> Result<RepoMilestonesPage, GhError> {
    let reply: GraphqlReply<MilestonesField> =
        serde_json::from_str(json).map_err(|error| bad_reply(&error))?;
    let conn = reply.data.repository.milestones;
    let next = conn.page_info.next();
    let mut milestones = Vec::with_capacity(conn.nodes.len());
    for node in conn.nodes {
        let state = match node.state.to_ascii_lowercase().as_str() {
            "open" => MilestoneState::Open,
            "closed" => MilestoneState::Closed,
            other => {
                return Err(GhError::BadReply {
                    reason: format!("unknown milestone state: {other}"),
                });
            }
        };
        milestones.push(RepoMilestone {
            number: node.number,
            title: node.title,
            description: node.description.unwrap_or_default(),
            state,
        });
    }
    Ok(RepoMilestonesPage { milestones, next })
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

        let page = parse_repo_labels(json).unwrap();
        assert_eq!(page.next(), None);
        assert_eq!(page.labels.len(), 2);
        assert_eq!(page.labels[0].name, "type:feat");
        assert_eq!(page.labels[0].color, "1d76db");
        assert_eq!(page.labels[0].description, "New feature");
        assert_eq!(page.labels[1].name, "area:core");
        assert_eq!(page.labels[1].color, "2ea44f");
        assert_eq!(page.labels[1].description, "");
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

        let page = parse_repo_milestones(json).unwrap();
        assert_eq!(page.next(), Some("cursor123"));
        assert_eq!(page.milestones.len(), 2);
        assert_eq!(page.milestones[0].number, 1);
        assert_eq!(page.milestones[0].title, "v0");
        assert_eq!(page.milestones[0].description, "Bootstrap");
        assert_eq!(page.milestones[0].state, MilestoneState::Open);
        assert_eq!(page.milestones[1].number, 2);
        assert_eq!(page.milestones[1].title, "v1");
        assert_eq!(page.milestones[1].description, "");
        assert_eq!(page.milestones[1].state, MilestoneState::Closed);
    }

    #[test]
    fn errors_on_invalid_milestone_state() {
        let json = r##"{
            "data": {
                "repository": {
                    "milestones": {
                        "pageInfo": {
                            "hasNextPage": false,
                            "endCursor": null
                        },
                        "nodes": [
                            {
                                "number": 1,
                                "title": "v0",
                                "description": null,
                                "state": "UNKNOWN_STATE"
                            }
                        ]
                    }
                }
            }
        }"##;

        let err = parse_repo_milestones(json).unwrap_err();
        assert!(matches!(err, GhError::BadReply { .. }));
    }
}
