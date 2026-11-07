//! Tests for status mapping and case tolerance.

use tools_core::adapters::github::parse::{parse_items_with, parse_status_field};
use tools_core::config::Config;
use tools_core::domain::{Status, StatusMapping};

const UNKNOWN_STATUS: &str = include_str!("../../fixtures/items_unknown_status.json");
const STATUS_FIELD: &str = include_str!("../../fixtures/status_field.json");

#[test]
fn default_mapping_matches_case_insensitively() {
    let mapping = StatusMapping::new();
    assert_eq!(mapping.status_for("backlog"), Some(Status::Backlog));
    assert_eq!(mapping.status_for("BACKLOG"), Some(Status::Backlog));
    assert_eq!(mapping.status_for("  ready  "), Some(Status::Ready));
    assert_eq!(mapping.status_for("In Progress"), Some(Status::InProgress));
    assert_eq!(mapping.status_for("in_progress"), Some(Status::InProgress));
    assert_eq!(mapping.status_for("in-progress"), Some(Status::InProgress));
    assert_eq!(mapping.status_for("IN PROGRESS"), Some(Status::InProgress));
    assert_eq!(mapping.status_for("In Review"), Some(Status::InReview));
    assert_eq!(mapping.status_for("in_review"), Some(Status::InReview));
    assert_eq!(mapping.status_for("done"), Some(Status::Done));
    assert_eq!(mapping.status_for("DONE"), Some(Status::Done));
    assert_eq!(mapping.status_for("Doing"), None);
}

#[test]
fn custom_column_aliases_map_to_canonical_statuses() {
    let mapping = StatusMapping::from_pairs([
        ("Todo", "Backlog"),
        ("Doing", "In progress"),
        ("Under Review", "in_review"),
        ("Finished", "Done"),
    ])
    .expect("valid mapping");

    assert_eq!(mapping.status_for("Todo"), Some(Status::Backlog));
    assert_eq!(mapping.status_for("todo"), Some(Status::Backlog));
    assert_eq!(mapping.status_for("Doing"), Some(Status::InProgress));
    assert_eq!(mapping.status_for("doing"), Some(Status::InProgress));
    assert_eq!(mapping.status_for("Under Review"), Some(Status::InReview));
    assert_eq!(mapping.status_for("Finished"), Some(Status::Done));
    // Standard statuses still match
    assert_eq!(mapping.status_for("Ready"), Some(Status::Ready));
}

#[test]
fn parses_items_with_custom_mapping() {
    // UNKNOWN_STATUS fixture contains status "Doing"
    let mapping = StatusMapping::from_pairs([("Doing", "In progress")]).expect("valid mapping");
    let page = parse_items_with(UNKNOWN_STATUS, &crate::support::fixture_target(), &mapping)
        .expect("parses with mapped Doing status");

    let card = page.cards().first().expect("contains parsed card");
    assert_eq!(card.status(), Status::InProgress);
}

#[test]
fn loads_status_mapping_from_tools_toml() {
    let toml = r#"
[github.repository]
owner = "jaoppb"
name = "test"

[github.project]
number = 1

[github.project.status_mapping]
"Todo" = "Backlog"
"Doing" = "In progress"
"#;
    let config: Config = toml::from_str(toml).expect("parses toml");
    let mapping = config
        .project()
        .expect("project exists")
        .status_mapping()
        .expect("valid mapping");

    assert_eq!(mapping.status_for("Todo"), Some(Status::Backlog));
    assert_eq!(mapping.status_for("Doing"), Some(Status::InProgress));
}

#[test]
fn resolves_option_for_status_with_fallback_alias() {
    let field = parse_status_field(STATUS_FIELD).expect("parses field");
    // STATUS_FIELD has: Backlog, Ready, In progress, In review, Done
    let mapping = StatusMapping::from_pairs([("Backlog", "Ready")]).expect("valid mapping");

    // Standard name matches directly
    assert_eq!(
        field.option_for_status(Status::Backlog, &mapping),
        field.option("Backlog")
    );

    // Case-insensitive match on canonical name
    assert_eq!(
        field.option_for_status(Status::InProgress, &mapping),
        field.option("In progress")
    );
}

#[test]
fn resolves_mapped_alias_when_canonical_option_missing() {
    let json = r#"{
        "data": {
            "repositoryOwner": {
                "projectV2": {
                    "id": "PROJ_123",
                    "field": {
                        "id": "FIELD_456",
                        "options": [
                            { "id": "OPT_TODO", "name": "Todo" },
                            { "id": "OPT_DOING", "name": "In Progress" },
                            { "id": "OPT_DONE", "name": "Done" }
                        ]
                    }
                }
            }
        }
    }"#;
    let field = parse_status_field(json).expect("parses custom field");
    let mapping = StatusMapping::from_pairs([("Todo", "Backlog")]).expect("valid mapping");

    // Backlog is not in options, but Todo is mapped to Backlog!
    assert_eq!(
        field.option_for_status(Status::Backlog, &mapping),
        Some("OPT_TODO")
    );
    // InProgress matches "In Progress" case-insensitively
    assert_eq!(
        field.option_for_status(Status::InProgress, &mapping),
        Some("OPT_DOING")
    );
}

#[test]
fn invalid_status_mapping_target_fails_cleanly() {
    let toml = r#"
[github.repository]
owner = "jaoppb"
name = "test"

[github.project]
number = 1

[github.project.status_mapping]
"Todo" = "NotAStatus"
"#;
    let config: Config = toml::from_str(toml).expect("parses toml");
    let error = config
        .project()
        .expect("project exists")
        .status_mapping()
        .expect_err("fails on invalid status name");

    assert_eq!(
        error.to_string(),
        "unknown canonical status 'NotAStatus'; valid statuses are Backlog, Ready, In progress, In review, Done"
    );
}

#[test]
fn resolves_option_for_status_with_separator_variations() {
    let json = r#"{
        "data": {
            "repositoryOwner": {
                "projectV2": {
                    "id": "PROJ_123",
                    "field": {
                        "id": "FIELD_456",
                        "options": [
                            { "id": "OPT_IN_PROG", "name": "in_progress" },
                            { "id": "OPT_IN_REV", "name": "in-review" }
                        ]
                    }
                }
            }
        }
    }"#;
    let field = parse_status_field(json).expect("parses custom field");
    let mapping = StatusMapping::new();

    assert_eq!(
        field.option_for_status(Status::InProgress, &mapping),
        Some("OPT_IN_PROG")
    );
    assert_eq!(
        field.option_for_status(Status::InReview, &mapping),
        Some("OPT_IN_REV")
    );
}
