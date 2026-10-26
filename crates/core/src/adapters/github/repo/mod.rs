//! Reading repository state via GraphQL and updating via REST API.

use serde::Deserialize;

use crate::adapters::github::error::GhError;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::domain::repo::{RepoLabel, RepoMilestone, RepoSettings};
use crate::graphql::{
    Document, Field, FragmentDefinition, OperationDefinition, OperationType, Selection,
    SelectionSet, Value, VariableDefinition,
};

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

    /// Fetches the current repository settings (via REST or GraphQL).
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the call fails or the response cannot be parsed.
    pub fn get_settings(&self) -> Result<RepoSettings, GhError> {
        let state = self.get_state(RepoEntities {
            settings: true,
            labels: false,
            milestones: false,
        })?;
        state.settings.ok_or_else(|| GhError::BadReply {
            reason: "repository settings missing from GitHub response".into(),
        })
    }

    /// Fetches repository state for selected entities via consolidated GraphQL query.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the GitHub CLI call fails or the response cannot be parsed.
    pub fn get_state(&self, entities: RepoEntities) -> Result<RepoState, GhError> {
        let mut state = RepoState::default();

        let doc = build_repo_query(entities);
        let query_str = doc.to_string();

        let fields = vec![
            format!("owner={}", self.target.owner()),
            format!("name={}", self.target.repository()),
        ];

        let reply = self.runner.graphql(&query_str, &fields)?;
        let parsed: GraphQLResponse = serde_json::from_str(&reply).map_err(|error| {
            GhError::BadReply {
                reason: format!("failed to parse repository GraphQL response: {error}"),
            }
        })?;

        let repo = parsed
            .data
            .and_then(|d| d.repository)
            .ok_or_else(|| GhError::BadReply {
                reason: "repository not found or missing from GraphQL reply".into(),
            })?;

        // 1. Extract settings
        if entities.settings
            && let (
                Some(allow_merge_commit),
                Some(allow_squash_merge),
                Some(allow_rebase_merge),
                Some(delete_branch_on_merge),
                Some(merge_commit_title),
                Some(merge_commit_message),
            ) = (
                repo.merge_commit_allowed,
                repo.squash_merge_allowed,
                repo.rebase_merge_allowed,
                repo.delete_branch_on_merge,
                repo.merge_commit_title,
                repo.merge_commit_message,
            )
        {
            state.settings = Some(RepoSettings {
                allow_merge_commit,
                allow_squash_merge,
                allow_rebase_merge,
                delete_branch_on_merge,
                merge_commit_title,
                merge_commit_message,
            });
        }

        // 2. Extract labels and paginate if needed
        if entities.labels
            && let Some(conn) = repo.labels
        {
            for node in conn.nodes {
                state.labels.push(RepoLabel {
                    id: node.id,
                    name: node.name,
                    color: node.color.trim_start_matches('#').to_ascii_lowercase(),
                    description: node.description.unwrap_or_default(),
                });
            }

            let mut page_info = conn.page_info;
            while page_info.has_next_page {
                let cursor = match page_info.end_cursor {
                    Some(ref c) => c.clone(),
                    None => break,
                };
                let (more_labels, next_info) = self.get_more_labels(&cursor)?;
                state.labels.extend(more_labels);
                page_info = next_info;
            }
        }

        // 3. Extract milestones and paginate if needed
        if entities.milestones
            && let Some(conn) = repo.milestones
        {
            for node in conn.nodes {
                state.milestones.push(RepoMilestone {
                    number: Some(node.number),
                    title: node.title,
                    description: node.description,
                    state: node.state.to_ascii_lowercase(),
                });
            }

            let mut page_info = conn.page_info;
            while page_info.has_next_page {
                let cursor = match page_info.end_cursor {
                    Some(ref c) => c.clone(),
                    None => break,
                };
                let (more_milestones, next_info) = self.get_more_milestones(&cursor)?;
                state.milestones.extend(more_milestones);
                page_info = next_info;
            }
        }

        Ok(state)
    }

    fn get_more_labels(&self, cursor: &str) -> Result<(Vec<RepoLabel>, PageInfo), GhError> {
        let label_frag = FragmentDefinition::new(
            "LabelFields",
            "Label",
            SelectionSet::new()
                .with_field("id")
                .with_field("name")
                .with_field("color")
                .with_field("description"),
        );

        let labels_field = Field::new("labels")
            .with_arg("first", 100)
            .with_arg("after", Value::Variable("cursor".into()))
            .with_selection_set(
                SelectionSet::new()
                    .with(
                        Field::new("pageInfo")
                            .with_selection_set(
                                SelectionSet::new()
                                    .with_field("hasNextPage")
                                    .with_field("endCursor"),
                            ),
                    )
                    .with(
                        Field::new("nodes").with_selection_set(
                            SelectionSet::new()
                                .with(Selection::FragmentSpread("LabelFields".into())),
                        ),
                    ),
            );

        let repo_field = Field::new("repository")
            .with_arg("owner", Value::Variable("owner".into()))
            .with_arg("name", Value::Variable("name".into()))
            .with_selection_set(SelectionSet::new().with(labels_field));

        let mut op = OperationDefinition::new(
            OperationType::Query,
            Some("MoreLabels"),
            SelectionSet::new().with(repo_field),
        );
        op.add_variable(VariableDefinition::new("owner", "String!"));
        op.add_variable(VariableDefinition::new("name", "String!"));
        op.add_variable(VariableDefinition::new("cursor", "String!"));

        let doc = Document::new().with_definition(op).with_definition(label_frag);
        let query_str = doc.to_string();

        let fields = vec![
            format!("owner={}", self.target.owner()),
            format!("name={}", self.target.repository()),
            format!("cursor={cursor}"),
        ];

        let reply = self.runner.graphql(&query_str, &fields)?;
        let parsed: GraphQLResponse = serde_json::from_str(&reply).map_err(|error| {
            GhError::BadReply {
                reason: format!("failed to parse subsequent labels page: {error}"),
            }
        })?;

        let conn = parsed
            .data
            .and_then(|d| d.repository)
            .and_then(|r| r.labels)
            .ok_or_else(|| GhError::BadReply {
                reason: "missing labels connection in reply".into(),
            })?;

        let labels = conn
            .nodes
            .into_iter()
            .map(|node| RepoLabel {
                id: node.id,
                name: node.name,
                color: node.color.trim_start_matches('#').to_ascii_lowercase(),
                description: node.description.unwrap_or_default(),
            })
            .collect();

        Ok((labels, conn.page_info))
    }

    fn get_more_milestones(&self, cursor: &str) -> Result<(Vec<RepoMilestone>, PageInfo), GhError> {
        let milestone_frag = FragmentDefinition::new(
            "MilestoneFields",
            "Milestone",
            SelectionSet::new()
                .with_field("number")
                .with_field("title")
                .with_field("description")
                .with_field("state"),
        );

        let milestones_field = Field::new("milestones")
            .with_arg("first", 100)
            .with_arg(
                "states",
                Value::List(vec![
                    Value::Enum("OPEN".into()),
                    Value::Enum("CLOSED".into()),
                ]),
            )
            .with_arg("after", Value::Variable("cursor".into()))
            .with_selection_set(
                SelectionSet::new()
                    .with(
                        Field::new("pageInfo")
                            .with_selection_set(
                                SelectionSet::new()
                                    .with_field("hasNextPage")
                                    .with_field("endCursor"),
                            ),
                    )
                    .with(
                        Field::new("nodes").with_selection_set(
                            SelectionSet::new()
                                .with(Selection::FragmentSpread("MilestoneFields".into())),
                        ),
                    ),
            );

        let repo_field = Field::new("repository")
            .with_arg("owner", Value::Variable("owner".into()))
            .with_arg("name", Value::Variable("name".into()))
            .with_selection_set(SelectionSet::new().with(milestones_field));

        let mut op = OperationDefinition::new(
            OperationType::Query,
            Some("MoreMilestones"),
            SelectionSet::new().with(repo_field),
        );
        op.add_variable(VariableDefinition::new("owner", "String!"));
        op.add_variable(VariableDefinition::new("name", "String!"));
        op.add_variable(VariableDefinition::new("cursor", "String!"));

        let doc = Document::new().with_definition(op).with_definition(milestone_frag);
        let query_str = doc.to_string();

        let fields = vec![
            format!("owner={}", self.target.owner()),
            format!("name={}", self.target.repository()),
            format!("cursor={cursor}"),
        ];

        let reply = self.runner.graphql(&query_str, &fields)?;
        let parsed: GraphQLResponse = serde_json::from_str(&reply).map_err(|error| {
            GhError::BadReply {
                reason: format!("failed to parse subsequent milestones page: {error}"),
            }
        })?;

        let conn = parsed
            .data
            .and_then(|d| d.repository)
            .and_then(|r| r.milestones)
            .ok_or_else(|| GhError::BadReply {
                reason: "missing milestones connection in reply".into(),
            })?;

        let milestones = conn
            .nodes
            .into_iter()
            .map(|node| RepoMilestone {
                number: Some(node.number),
                title: node.title,
                description: node.description,
                state: node.state.to_ascii_lowercase(),
            })
            .collect();

        Ok((milestones, conn.page_info))
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
            "color": label.color.trim_start_matches('#'),
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
            "color": label.color.trim_start_matches('#'),
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
        payload.insert("state".into(), serde_json::Value::String(milestone.state.clone()));

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
        payload.insert("state".into(), serde_json::Value::String(milestone.state.clone()));

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

/// Builds the consolidated GraphQL query using the AST builder.
fn build_repo_query(entities: RepoEntities) -> Document {
    let mut repo_fields = SelectionSet::new().with_field("id");
    let mut fragments = Vec::new();

    if entities.settings {
        repo_fields = repo_fields
            .with_field("mergeCommitAllowed")
            .with_field("squashMergeAllowed")
            .with_field("rebaseMergeAllowed")
            .with_field("deleteBranchOnMerge")
            .with_field("mergeCommitTitle")
            .with_field("mergeCommitMessage");
    }

    if entities.labels {
        let label_frag = FragmentDefinition::new(
            "LabelFields",
            "Label",
            SelectionSet::new()
                .with_field("id")
                .with_field("name")
                .with_field("color")
                .with_field("description"),
        );
        fragments.push(label_frag);

        let labels_field = Field::new("labels")
            .with_arg("first", 100)
            .with_selection_set(
                SelectionSet::new()
                    .with(
                        Field::new("pageInfo")
                            .with_selection_set(
                                SelectionSet::new()
                                    .with_field("hasNextPage")
                                    .with_field("endCursor"),
                            ),
                    )
                    .with(
                        Field::new("nodes").with_selection_set(
                            SelectionSet::new()
                                .with(Selection::FragmentSpread("LabelFields".into())),
                        ),
                    ),
            );

        repo_fields = repo_fields.with(labels_field);
    }

    if entities.milestones {
        let milestone_frag = FragmentDefinition::new(
            "MilestoneFields",
            "Milestone",
            SelectionSet::new()
                .with_field("number")
                .with_field("title")
                .with_field("description")
                .with_field("state"),
        );
        fragments.push(milestone_frag);

        let milestones_field = Field::new("milestones")
            .with_arg("first", 100)
            .with_arg(
                "states",
                Value::List(vec![
                    Value::Enum("OPEN".into()),
                    Value::Enum("CLOSED".into()),
                ]),
            )
            .with_selection_set(
                SelectionSet::new()
                    .with(
                        Field::new("pageInfo")
                            .with_selection_set(
                                SelectionSet::new()
                                    .with_field("hasNextPage")
                                    .with_field("endCursor"),
                            ),
                    )
                    .with(
                        Field::new("nodes").with_selection_set(
                            SelectionSet::new()
                                .with(Selection::FragmentSpread("MilestoneFields".into())),
                        ),
                    ),
            );

        repo_fields = repo_fields.with(milestones_field);
    }

    let repo_field = Field::new("repository")
        .with_arg("owner", Value::Variable("owner".into()))
        .with_arg("name", Value::Variable("name".into()))
        .with_selection_set(repo_fields);

    let mut op = OperationDefinition::new(
        OperationType::Query,
        Some("RepositoryState"),
        SelectionSet::new().with(repo_field),
    );
    op.add_variable(VariableDefinition::new("owner", "String!"));
    op.add_variable(VariableDefinition::new("name", "String!"));

    let mut doc = Document::new().with_definition(op);
    for frag in fragments {
        doc.add_definition(frag);
    }
    doc
}

// ---------------------------------------------------------------------------
// Serde helper models for GraphQL deserialization
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQLResponse {
    data: Option<GraphQLData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQLData {
    repository: Option<RepositoryNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryNode {
    merge_commit_allowed: Option<bool>,
    squash_merge_allowed: Option<bool>,
    rebase_merge_allowed: Option<bool>,
    delete_branch_on_merge: Option<bool>,
    merge_commit_title: Option<String>,
    merge_commit_message: Option<String>,
    labels: Option<LabelsConnection>,
    milestones: Option<MilestonesConnection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabelsConnection {
    page_info: PageInfo,
    nodes: Vec<LabelNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MilestonesConnection {
    page_info: PageInfo,
    nodes: Vec<MilestoneNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabelNode {
    id: Option<String>,
    name: String,
    color: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MilestoneNode {
    number: u64,
    title: String,
    description: Option<String>,
    state: String,
}
