//! The repository-issues reply parser and the paging check, read against
//! recorded and hand-written replies.

use tools_core::adapters::github::parse::{IssuesPage, parse_issues};
use tools_core::adapters::github::{Gathered, GhError, Target};
use tools_core::domain::{BlockedBy, Lifecycle, Milestone, TreeIssue};

use crate::support::audit::labels;
use crate::support::number;

const LIVE: &str = include_str!("../../fixtures/issues_live.json");
const EDGE: &str = include_str!("../../fixtures/issues_edge.json");
const FIRST_OF_TWO: &str = include_str!("../../fixtures/issues_first_of_two.json");
const LAST_OF_TWO: &str = include_str!("../../fixtures/issues_last_of_two.json");
const NEXT_PAGE: &str = include_str!("../../fixtures/issues_next_page.json");
const UNKNOWN_STATE: &str = include_str!("../../fixtures/issues_unknown_state.json");
const TRUNCATED: &str = include_str!("../../fixtures/issues_truncated_blockers.json");

const RECORDED: &str = "the recorded reply parses";

fn page(json: &str) -> Result<IssuesPage, GhError> {
    parse_issues(json, &Target::cosmo())
}

fn tooling(raw: u64, lifecycle: Lifecycle) -> TreeIssue {
    TreeIssue::new(number(raw), lifecycle)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_milestone(Milestone::new("v1"))
}

#[test]
fn a_page_of_issues_becomes_tree_issues() {
    let issues = page(LIVE).expect(RECORDED);
    let epic =
        tooling(98, Lifecycle::Open).with_labels(labels(&["epic", "type:chore", "area:tooling"]));
    let story = tooling(91, Lifecycle::Open)
        .with_parent(number(98))
        .with_blocked_by(BlockedBy::new(vec![number(92)]));
    assert_eq!(issues.issues().first(), Some(&epic));
    assert_eq!(issues.issues().get(1), Some(&story));
    assert_eq!(issues.total(), 3);
    assert_eq!(issues.next(), None);
}

#[test]
fn reads_a_closed_issue() {
    let closed = tooling(92, Lifecycle::Closed).with_parent(number(91));
    assert_eq!(page(LIVE).expect(RECORDED).issues().last(), Some(&closed));
}

#[test]
fn reads_an_issue_with_no_parent_and_no_milestone() {
    let bare = TreeIssue::new(number(40), Lifecycle::Open).with_labels(labels(&[]));
    assert_eq!(page(EDGE).expect(RECORDED).issues().first(), Some(&bare));
}

#[test]
fn drops_a_blocker_of_another_repository() {
    let issues = page(EDGE).expect(RECORDED);
    let foreign = tooling(41, Lifecycle::Open)
        .with_parent(number(98))
        .with_blocked_by(BlockedBy::new(vec![number(91)]));
    assert_eq!(issues.issues().get(1), Some(&foreign));
}

#[test]
fn gives_the_cursor_of_the_next_page() {
    let issues = page(NEXT_PAGE).expect(RECORDED);
    assert!(issues.next().is_some());
    assert_eq!(issues.issues().len(), 3);
}

#[test]
fn refuses_a_reply_with_an_unknown_state() {
    let failure = page(UNKNOWN_STATE).expect_err("LOCKED is not a state");
    assert!(matches!(failure, GhError::BadReply { .. }));
}

#[test]
fn refuses_an_issue_whose_blockers_were_cut_off() {
    let failure = page(TRUNCATED).expect_err("the blocker list is cut off");
    assert_eq!(
        failure.to_string(),
        "gh answered only part of the data: issue #12 has 1 of 61 blockers"
    );
}

#[test]
fn refuses_an_unreadable_reply() {
    let failure = page("not json").expect_err("not a reply");
    assert!(
        failure
            .to_string()
            .starts_with("cannot read the answer of gh: ")
    );
}

#[test]
fn reads_every_page_of_issues() {
    let tree = Gathered::default()
        .with(&page(FIRST_OF_TWO).expect(RECORDED))
        .with(&page(LAST_OF_TWO).expect(RECORDED))
        .into_tree()
        .expect("both pages together are every issue");
    assert_eq!(tree.iter().count(), 3);
}

#[test]
fn refuses_a_read_that_returned_fewer_issues_than_the_repository_has() {
    let failure = Gathered::default()
        .with(&page(NEXT_PAGE).expect(RECORDED))
        .into_tree()
        .expect_err("only the first page was read");
    assert_eq!(
        failure.to_string(),
        "gh answered only part of the data: read 3 of 74 issues"
    );
}
