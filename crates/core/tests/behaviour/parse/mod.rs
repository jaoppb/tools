//! The reply parsers, read against recorded and hand-written replies.

use tools_core::adapters::github::parse::{
    parse_item_id, parse_items, parse_pulls, parse_status_field,
};
use tools_core::adapters::github::{GhError, GhRunner, Target};
use tools_core::domain::{Cards, Closing, Plan, Snapshot};

use crate::support::{gaps_of, moves_of, number};

const LIVE: &str = include_str!("../../fixtures/items_live.json");
const EDGE: &str = include_str!("../../fixtures/items_edge.json");
const UNKNOWN_STATUS: &str = include_str!("../../fixtures/items_unknown_status.json");
const NEXT_PAGE: &str = include_str!("../../fixtures/items_next_page.json");
const PULLS_LIVE: &str = include_str!("../../fixtures/pulls_live.json");
const PULLS_MANY: &str = include_str!("../../fixtures/pulls_many.json");
const ITEM_OF_ISSUE: &str = include_str!("../../fixtures/item_of_issue.json");
const STATUS_FIELD: &str = include_str!("../../fixtures/status_field.json");

fn plan_of(json: &str, closing: &[u64]) -> Result<Plan, GhError> {
    let page = parse_items(json, &Target::cosmo())?;
    let issues = closing.iter().map(|raw| number(*raw)).collect();
    let cards = Cards::new(page.cards().to_vec());
    Ok(Snapshot::new(cards, Closing::new(issues)).plan())
}

#[test]
fn a_page_of_items_becomes_cards() {
    let page = parse_items(LIVE, &Target::cosmo()).expect("the recorded page parses");
    let numbers: Vec<u64> = page
        .cards()
        .iter()
        .map(|card| card.number().value())
        .collect();
    assert_eq!(numbers, [3, 20, 31, 85, 92]);
    let plan = plan_of(LIVE, &[92, 20]).expect("the recorded page parses");
    assert_eq!(
        moves_of(&plan),
        ["#92 In progress -> In review (pull request open)"]
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
    assert!(moves_of(&plan).contains(&"#102 Ready -> Done (closed)".to_owned()));
}

#[test]
fn an_item_that_is_not_an_issue_is_dropped() {
    let page = parse_items(LIVE, &Target::cosmo()).expect("the recorded page parses");
    assert_eq!(page.cards().len(), 5);
}

#[test]
fn an_item_without_a_status_is_in_backlog() {
    let plan = plan_of(EDGE, &[]).expect("the edge page parses");
    assert!(moves_of(&plan).contains(&"#101 Backlog -> Ready (gate holds)".to_owned()));
}

#[test]
fn a_missing_priority_is_none() {
    let plan = plan_of(EDGE, &[]).expect("the edge page parses");
    assert_eq!(gaps_of(&plan), ["#104 needs triage: no Priority"]);
}

#[test]
fn an_issue_of_another_repository_is_dropped() {
    let page = parse_items(EDGE, &Target::cosmo()).expect("the edge page parses");
    assert!(page.cards().iter().all(|card| card.number() != number(105)));
}

#[test]
fn a_blocker_from_another_repository_counts_as_open() {
    let plan = plan_of(EDGE, &[]).expect("the edge page parses");
    assert!(
        moves_of(&plan)
            .iter()
            .all(|line| !line.starts_with("#103 "))
    );
}

#[test]
fn a_parent_that_is_an_epic_has_the_epic_role() {
    let plan = plan_of(EDGE, &[]).expect("the edge page parses");
    let moves = moves_of(&plan);
    assert!(moves.contains(&"#106 Backlog -> Ready (gate holds)".to_owned()));
    assert!(moves.iter().all(|line| !line.starts_with("#108 ")));
}

#[test]
fn the_end_cursor_of_a_page_is_read() {
    let last = parse_items(LIVE, &Target::cosmo()).expect("the recorded page parses");
    assert_eq!(last.next(), None);
    let more = parse_items(NEXT_PAGE, &Target::cosmo()).expect("the next page parses");
    assert_eq!(more.next(), Some("cursor-next"));
}

#[test]
fn a_page_of_pull_requests_lists_the_issues_they_close() {
    let live = parse_pulls(PULLS_LIVE).expect("the recorded pulls parse");
    assert_eq!(live.closing(), [number(73)]);
    let many = parse_pulls(PULLS_MANY).expect("the pulls parse");
    assert_eq!(many.closing(), [number(10), number(11), number(12)]);
    assert_eq!(many.next(), Some("pulls-next"));
}

#[test]
fn an_unknown_status_name_is_reported_with_its_value() {
    let failure = parse_items(UNKNOWN_STATUS, &Target::cosmo()).expect_err("Doing is unknown");
    assert_eq!(
        failure.to_string(),
        "the Project has a Status named Doing that this tool does not know"
    );
}

#[test]
fn a_reply_that_is_not_json_is_a_bad_reply() {
    let failure = parse_items("not json", &Target::cosmo()).expect_err("not json");
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
