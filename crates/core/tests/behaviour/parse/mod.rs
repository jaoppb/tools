//! The reply parsers, read against recorded and hand-written replies.

use tools_core::adapters::github::parse::{
    parse_item_id, parse_items, parse_pulls, parse_status_field,
};
use tools_core::adapters::github::{GhError, GhRunner, Target};
use tools_core::domain::{Cards, Closing, IssueNumber, Plan, Repo, Snapshot};

use crate::support::{frozen_of, gaps_of, moves_of, number, strict_rules};

const LIVE: &str = include_str!("../../fixtures/items_live.json");
const EDGE: &str = include_str!("../../fixtures/items_edge.json");
const UNKNOWN_STATUS: &str = include_str!("../../fixtures/items_unknown_status.json");
const NEXT_PAGE: &str = include_str!("../../fixtures/items_next_page.json");
const PARENT_CYCLE: &str = include_str!("../../fixtures/items_parent_cycle.json");
const PULLS_LIVE: &str = include_str!("../../fixtures/pulls_live.json");
const PULLS_MANY: &str = include_str!("../../fixtures/pulls_many.json");
const ITEM_OF_ISSUE: &str = include_str!("../../fixtures/item_of_issue.json");
const STATUS_FIELD: &str = include_str!("../../fixtures/status_field.json");
const ITEMS_ORG: &str = include_str!("../../fixtures/items_org.json");
const STATUS_FIELD_ORG: &str = include_str!("../../fixtures/status_field_org.json");

fn plan_of(json: &str, closing: &[u64]) -> Result<Plan, GhError> {
    plan_of_with(json, closing, &tools_core::config::RulesConfig::default())
}

fn plan_of_with(
    json: &str,
    closing: &[u64],
    rules: &tools_core::config::RulesConfig,
) -> Result<Plan, GhError> {
    let page = parse_items(json, &crate::support::fixture_target())?;
    let repo = crate::support::fixture_target().repo_coords().remove(0);
    let issues = closing
        .iter()
        .map(|raw| IssueNumber::new(repo.clone(), *raw))
        .collect();
    let cards = Cards::new(page.cards().to_vec());
    Ok(Snapshot::new(cards, Closing::new(issues)).plan_with(rules))
}

#[test]
fn a_page_of_items_becomes_cards() {
    let page =
        parse_items(LIVE, &crate::support::fixture_target()).expect("the recorded page parses");
    let numbers: Vec<u64> = page
        .cards()
        .iter()
        .map(|card| card.number().value())
        .collect();
    assert_eq!(numbers, [3, 20, 31, 85, 92]);
    let plan = plan_of(LIVE, &[92, 20]).expect("the recorded page parses");
    assert_eq!(
        moves_of(&plan),
        ["cosmo#92 In progress -> In review (pull request open)"]
    );
}

#[test]
fn an_effort_recorded_with_a_decimal_point_is_read() {
    let plan = plan_of(LIVE, &[]).expect("the recorded page parses");
    assert!(gaps_of(&plan).is_empty());
}

#[test]
fn a_closed_as_not_planned_issue_is_closed() {
    let plan = plan_of(EDGE, &[]).expect("the edge page parses");
    assert!(moves_of(&plan).contains(&"cosmo#102 Ready -> Done (closed)".to_owned()));
}

#[test]
fn an_item_that_is_not_an_issue_is_dropped() {
    let page =
        parse_items(LIVE, &crate::support::fixture_target()).expect("the recorded page parses");
    assert_eq!(page.cards().len(), 5);
}

#[test]
fn an_item_without_a_status_is_in_backlog() {
    let plan = plan_of(EDGE, &[]).expect("the edge page parses");
    assert!(moves_of(&plan).contains(&"cosmo#101 Backlog -> Ready (gate holds)".to_owned()));
}

#[test]
fn a_missing_priority_is_none() {
    let plan = plan_of_with(EDGE, &[], &strict_rules()).expect("the edge page parses");
    assert_eq!(gaps_of(&plan), ["cosmo#104 needs triage: no Priority"]);
}

#[test]
fn an_issue_of_another_repository_is_dropped() {
    let page = parse_items(EDGE, &crate::support::fixture_target()).expect("the edge page parses");
    assert!(
        page.cards()
            .iter()
            .all(|card| *card.number() != number(105))
    );
}

#[test]
fn an_issue_of_another_repository_is_kept_when_part_of_multi_target() {
    let target = Target::multi(
        "jaoppb",
        vec!["cosmo".to_owned(), "someone/else".to_owned()],
        Some(1),
    );
    let page = parse_items(EDGE, &target).expect("the edge page parses");
    let target_issue = IssueNumber::new(Repo::new("someone", "else"), 105);
    assert!(
        page.cards()
            .iter()
            .any(|card| card.number() == &target_issue)
    );
}

#[test]
fn a_closed_blocker_from_another_repository_unblocks_issue() {
    let plan = plan_of(EDGE, &[]).expect("the edge page parses");
    assert!(moves_of(&plan).contains(&"cosmo#103 Backlog -> Ready (gate holds)".to_owned()));
}

#[test]
fn an_open_blocker_from_another_repository_keeps_issue_in_backlog() {
    let open_blocker_json = EDGE.replace("\"state\": \"CLOSED\"", "\"state\": \"OPEN\"");
    let plan = plan_of(&open_blocker_json, &[]).expect("the modified edge page parses");
    assert!(
        moves_of(&plan)
            .iter()
            .all(|line| !line.starts_with("cosmo#103 "))
    );
}

#[test]
fn a_parent_that_is_an_epic_has_the_epic_role() {
    let plan = plan_of(EDGE, &[]).expect("the edge page parses");
    let moves = moves_of(&plan);
    assert!(moves.contains(&"cosmo#106 Backlog -> Ready (gate holds)".to_owned()));
    assert!(moves.iter().all(|line| !line.starts_with("cosmo#108 ")));
}

#[test]
fn the_end_cursor_of_a_page_is_read() {
    let last =
        parse_items(LIVE, &crate::support::fixture_target()).expect("the recorded page parses");
    assert_eq!(last.next(), None);
    let more =
        parse_items(NEXT_PAGE, &crate::support::fixture_target()).expect("the next page parses");
    assert_eq!(more.next(), Some("cursor-next"));
}

#[test]
fn a_page_of_pull_requests_lists_the_issues_they_close() {
    let cosmo_repo = crate::support::fixture_target().repo_coords().remove(0);
    let live = parse_pulls(PULLS_LIVE, &cosmo_repo).expect("the recorded pulls parse");
    let cosmo_num = |raw| IssueNumber::new(cosmo_repo.clone(), raw);
    assert_eq!(live.closing(), [cosmo_num(73)]);
    let many = parse_pulls(PULLS_MANY, &cosmo_repo).expect("the pulls parse");
    assert_eq!(
        many.closing(),
        [cosmo_num(10), cosmo_num(11), cosmo_num(12)]
    );
    assert_eq!(many.next(), Some("pulls-next"));
}

#[test]
fn a_pull_request_closing_an_issue_in_another_repository_is_read() {
    let pulls_cross = r#"{
      "data": {
        "repository": {
          "pullRequests": {
            "pageInfo": {
              "hasNextPage": false,
              "endCursor": null
            },
            "nodes": [
              {
                "closingIssuesReferences": {
                  "nodes": [
                    {
                      "number": 42,
                      "repository": {
                        "nameWithOwner": "other/repo"
                      }
                    }
                  ]
                }
              }
            ]
          }
        }
      }
    }"#;
    let cosmo_repo = crate::support::fixture_target().repo_coords().remove(0);
    let page = parse_pulls(pulls_cross, &cosmo_repo).expect("cross repo pulls parse");
    assert_eq!(
        page.closing(),
        [IssueNumber::new(Repo::new("other", "repo"), 42)]
    );
}

#[test]
fn an_unknown_status_name_is_reported_with_its_value() {
    let failure = parse_items(UNKNOWN_STATUS, &crate::support::fixture_target())
        .expect_err("Doing is unknown");
    assert_eq!(
        failure.to_string(),
        "the Project has a Status named Doing that this tool does not know; configure a column alias in tools.toml under [github.project.status_mapping]"
    );
}

#[test]
fn a_reply_that_is_not_json_is_a_bad_reply() {
    let failure = parse_items("not json", &crate::support::fixture_target()).expect_err("not json");
    assert!(
        failure
            .to_string()
            .starts_with("cannot read the answer of gh: ")
    );
}

#[test]
fn a_missing_gh_program_is_reported_as_missing() {
    let runner = GhRunner::new("cosmo-no-such-program");
    let failure = runner
        .graphql("query { viewer { login } }", &[])
        .expect_err("no program");
    assert_eq!(
        failure.to_string(),
        "gh is not installed or is not on the PATH"
    );
}

#[test]
fn the_item_of_an_issue_in_this_project_is_found() {
    let found = parse_item_id(ITEM_OF_ISSUE, 4).expect("the lookup parses");
    assert_eq!(found.as_deref(), Some("PVTI_lAHOA6x_Is4BmIoKzg_RxYk"));
    assert_eq!(
        parse_item_id(ITEM_OF_ISSUE, 9).expect("the lookup parses"),
        None
    );
}

#[test]
fn the_status_field_lists_its_options() {
    let field = parse_status_field(STATUS_FIELD).expect("the field parses");
    assert_eq!(field.option("In review"), Some("571c0c90"));
    assert_eq!(field.option("Doing"), None);
    assert_eq!(field.project(), "PVT_kwHOA6x_Is4BmIoK");
}

#[test]
fn an_organization_page_of_items_becomes_cards() {
    let org_target = Target::new("gatuno-manga", "web", Some(1));
    let page = parse_items(ITEMS_ORG, &org_target).expect("the organization page parses");
    let numbers: Vec<u64> = page
        .cards()
        .iter()
        .map(|card| card.number().value())
        .collect();
    assert_eq!(numbers, [10, 11]);
    assert_eq!(
        page.cards()[0].status(),
        tools_core::domain::Status::InProgress
    );
    assert_eq!(page.cards()[1].status(), tools_core::domain::Status::Ready);
}

#[test]
fn the_status_field_of_an_organization_project_parses() {
    let field = parse_status_field(STATUS_FIELD_ORG).expect("the field parses");
    assert_eq!(field.option("In review"), Some("opt_review"));
    assert_eq!(field.option("Doing"), None);
    assert_eq!(field.project(), "PVT_kwDOEHF8jM4BVBtl");
}

#[test]
fn a_parent_cycle_between_stories_leaves_its_cards_unchanged_and_reported() {
    let plan = plan_of(PARENT_CYCLE, &[]).expect("the fixture parses");
    assert_eq!(
        moves_of(&plan),
        vec!["cosmo#204 Backlog -> Ready (gate holds)"]
    );
    assert_eq!(
        frozen_of(&plan),
        vec![
            "cosmo#201 left unchanged: parent cycle",
            "cosmo#202 left unchanged: parent cycle",
            "cosmo#203 left unchanged: parent cycle",
        ]
    );
    assert!(gaps_of(&plan).is_empty());
}
