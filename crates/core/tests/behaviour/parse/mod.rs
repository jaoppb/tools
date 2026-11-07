//! The reply parsers, read against recorded and hand-written replies.

use tools_core::adapters::github::parse::{
    parse_item_id, parse_items, parse_pulls, parse_status_field,
};
use tools_core::adapters::github::{GhError, GhRunner, Target};
use tools_core::domain::{Cards, Closes, Closing, IssueNumber, Repo, Snapshot};
use tools_core::workflow::Outcome;

use crate::support::{fixture_target, number, status_name, statuses};
use crate::workflow::support::{Remote, cosmo, evaluate, fixture_remote};

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

/// The moves and frozen cards the cosmo preset decides for a recorded page,
/// as the report prints them. An open pull request (number 1000 + issue)
/// closes each of `closing`.
fn report_of(json: &str, closing: &[u64], remote: &Remote) -> Result<Vec<String>, GhError> {
    let page = parse_items(json, &fixture_target(), &statuses())?;
    let repo = fixture_target().repo_coords().remove(0);
    let references = closing
        .iter()
        .map(|raw| {
            Closes::new(
                IssueNumber::new(repo.clone(), 1000 + *raw),
                IssueNumber::new(repo.clone(), *raw),
            )
        })
        .collect();
    let snapshot = Snapshot::new(Cards::new(page.cards().to_vec()), Closing::new(references));
    let evaluation = evaluate(&cosmo(), &snapshot, remote);
    Ok(evaluation
        .iter()
        .filter_map(|decision| match decision.outcome() {
            Outcome::Move { from, to, rule } => {
                Some(format!("{} {from} -> {to} ({rule})", decision.number()))
            }
            Outcome::Frozen { cause } => Some(format!("{} frozen: {cause}", decision.number())),
            Outcome::Keep { .. } => None,
        })
        .collect())
}

#[test]
fn a_page_of_items_becomes_cards() {
    let page = parse_items(LIVE, &crate::support::fixture_target(), &statuses())
        .expect("the recorded page parses");
    let numbers: Vec<u64> = page
        .cards()
        .iter()
        .map(|card| card.number().value())
        .collect();
    assert_eq!(numbers, [3, 20, 31, 85, 92]);
    let report = report_of(LIVE, &[92, 20], &fixture_remote()).expect("the recorded page parses");
    assert_eq!(
        report,
        ["cosmo#92 In progress -> In review (started-with-pull-request)"]
    );
}

#[test]
fn an_effort_recorded_with_a_decimal_point_is_read() {
    let page = parse_items(LIVE, &fixture_target(), &statuses()).expect("the recorded page parses");
    let efforts: Vec<_> = page
        .cards()
        .iter()
        .map(|card| card.facts().effort_points())
        .collect();
    assert!(efforts.contains(&Some(5)), "{efforts:?}");
}

#[test]
fn a_closed_as_not_planned_issue_is_closed() {
    let report = report_of(EDGE, &[], &fixture_remote()).expect("the edge page parses");
    assert!(report.contains(&"cosmo#102 Ready -> Done (closed-is-done)".to_owned()));
}

#[test]
fn an_item_that_is_not_an_issue_is_dropped() {
    let page = parse_items(LIVE, &crate::support::fixture_target(), &statuses())
        .expect("the recorded page parses");
    assert_eq!(page.cards().len(), 5);
}

#[test]
fn an_item_without_a_status_is_in_backlog() {
    let report = report_of(EDGE, &[], &fixture_remote()).expect("the edge page parses");
    assert!(report.contains(&"cosmo#101 Backlog -> Ready (gate-holds)".to_owned()));
}

#[test]
fn a_missing_priority_is_none() {
    let page = parse_items(EDGE, &fixture_target(), &statuses()).expect("the edge page parses");
    let card = page
        .cards()
        .iter()
        .find(|card| card.number().value() == 104)
        .expect("the edge page has #104");
    assert_eq!(card.facts().priority_name(), None);
}

#[test]
fn an_issue_of_another_repository_is_dropped() {
    let page = parse_items(EDGE, &crate::support::fixture_target(), &statuses())
        .expect("the edge page parses");
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
    let page = parse_items(EDGE, &target, &statuses()).expect("the edge page parses");
    let target_issue = IssueNumber::new(Repo::new("someone", "else"), 105);
    assert!(
        page.cards()
            .iter()
            .any(|card| card.number() == &target_issue)
    );
}

#[test]
fn a_closed_blocker_from_another_repository_unblocks_issue() {
    let report = report_of(EDGE, &[], &fixture_remote()).expect("the edge page parses");
    assert!(report.contains(&"cosmo#103 Backlog -> Ready (gate-holds)".to_owned()));
}

#[test]
fn an_open_blocker_from_another_repository_keeps_issue_in_backlog() {
    let open_blocker_json = EDGE.replace("\"state\": \"CLOSED\"", "\"state\": \"OPEN\"");
    let report = report_of(&open_blocker_json, &[], &fixture_remote())
        .expect("the modified edge page parses");
    assert!(report.iter().all(|line| !line.starts_with("cosmo#103 ")));
}

#[test]
fn a_parent_that_is_an_epic_has_the_epic_role() {
    let report = report_of(EDGE, &[], &fixture_remote()).expect("the edge page parses");
    assert!(report.contains(&"cosmo#106 Backlog -> Ready (gate-holds)".to_owned()));
    assert!(report.iter().all(|line| !line.starts_with("cosmo#108 ")));
}

#[test]
fn the_end_cursor_of_a_page_is_read() {
    let last = parse_items(LIVE, &crate::support::fixture_target(), &statuses())
        .expect("the recorded page parses");
    assert_eq!(last.next(), None);
    let more = parse_items(NEXT_PAGE, &crate::support::fixture_target(), &statuses())
        .expect("the next page parses");
    assert_eq!(more.next(), Some("cursor-next"));
}

#[test]
fn a_page_of_pull_requests_lists_the_issues_they_close() {
    let cosmo_repo = crate::support::fixture_target().repo_coords().remove(0);
    let live = parse_pulls(PULLS_LIVE, &cosmo_repo).expect("the recorded pulls parse");
    let cosmo_num = |raw| IssueNumber::new(cosmo_repo.clone(), raw);
    assert_eq!(live.closing(), [Closes::new(cosmo_num(74), cosmo_num(73))]);
    let many = parse_pulls(PULLS_MANY, &cosmo_repo).expect("the pulls parse");
    assert_eq!(
        many.closing(),
        [
            Closes::new(cosmo_num(201), cosmo_num(10)),
            Closes::new(cosmo_num(201), cosmo_num(11)),
            Closes::new(cosmo_num(203), cosmo_num(12)),
        ]
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
                "number": 7,
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
        [Closes::new(
            IssueNumber::new(cosmo_repo.clone(), 7),
            IssueNumber::new(Repo::new("other", "repo"), 42)
        )]
    );
}

#[test]
fn an_unknown_status_name_is_reported_with_its_value() {
    let failure = parse_items(
        UNKNOWN_STATUS,
        &crate::support::fixture_target(),
        &statuses(),
    )
    .expect_err("Doing is unknown");
    assert_eq!(
        failure.to_string(),
        "the Project has a Status named Doing that the workflow does not declare; list it in `statuses` under [workflow] or rename the column"
    );
}

#[test]
fn a_reply_that_is_not_json_is_a_bad_reply() {
    let failure = parse_items("not json", &crate::support::fixture_target(), &statuses())
        .expect_err("not json");
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
    let page =
        parse_items(ITEMS_ORG, &org_target, &statuses()).expect("the organization page parses");
    let numbers: Vec<u64> = page
        .cards()
        .iter()
        .map(|card| card.number().value())
        .collect();
    assert_eq!(numbers, [10, 11]);
    assert_eq!(page.cards()[0].status(), "In progress");
    assert_eq!(page.cards()[1].status(), "Ready");
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
    let report = report_of(PARENT_CYCLE, &[], &Remote::default()).expect("the fixture parses");
    assert_eq!(
        report,
        vec![
            "cosmo#201 frozen: the issue is in a parent cycle",
            "cosmo#202 frozen: the issue is in a parent cycle",
            "cosmo#203 frozen: parent cosmo#201 is frozen",
            "cosmo#204 Backlog -> Ready (gate-holds)",
        ]
    );
}

#[test]
fn a_parent_in_another_repository_keeps_its_repository() {
    let json = EDGE.replace(
        "\"number\": 107,",
        "\"number\": 107, \"repository\": { \"nameWithOwner\": \"other/repo\" },",
    );
    let page = parse_items(&json, &fixture_target(), &statuses()).expect("the edge page parses");
    let child = page
        .cards()
        .iter()
        .find(|card| {
            card.links()
                .parent_number()
                .is_some_and(|p| p.value() == 107)
        })
        .expect("a card has parent 107");
    let parent = child.links().parent_number().expect("it has a parent");
    assert_eq!(parent.repo(), &Repo::new("other", "repo"));
}

#[test]
fn a_status_is_written_to_the_option_that_has_exactly_its_name() {
    let field = parse_status_field(STATUS_FIELD).expect("the field parses");
    let in_review = status_name("In review");
    assert_eq!(
        field.option_for_status(&in_review),
        field.option("In review")
    );
    assert!(field.option_for_status(&in_review).is_some());
    let undeclared = tools_core::domain::Statuses::new(&["Doing"], "Doing")
        .expect("valid")
        .parse("Doing")
        .expect("declared");
    assert_eq!(field.option_for_status(&undeclared), None);
}
