//! The reply parsers of issue texts and pull request texts, and the check that
//! the pages read are all of them, read against recorded and hand-written
//! replies.

use tools_core::adapters::github::parse::{TextsPage, parse_issue_texts, parse_pull_texts};
use tools_core::adapters::github::{GatheredTexts, GhError};
use tools_core::domain::{IssueTree, Members, Survey, Texts};

use crate::support::audit::lines;

const ISSUES_LIVE: &str = include_str!("../../fixtures/texts_issues_live.json");
const ISSUES_FIRST: &str = include_str!("../../fixtures/texts_issues_first_of_two.json");
const ISSUES_LAST: &str = include_str!("../../fixtures/texts_issues_last_of_two.json");
const ISSUES_CUT_OFF: &str = include_str!("../../fixtures/texts_issues_cut_off.json");
const PULLS_LIVE: &str = include_str!("../../fixtures/texts_pulls_live.json");
const PULLS_SURFACES: &str = include_str!("../../fixtures/texts_pulls_surfaces.json");
const PULLS_CUT_OFF: &str = include_str!("../../fixtures/texts_pulls_cut_off.json");
const PULLS_THREAD_CUT_OFF: &str = include_str!("../../fixtures/texts_pulls_thread_cut_off.json");

const RECORDED: &str = "the recorded reply parses";

#[test]
fn a_page_of_issues_becomes_title_body_and_comment_texts() {
    let page = parse_issue_texts(ISSUES_LIVE).expect(RECORDED);
    assert_eq!(page.items().len(), 5);
    assert_eq!(page.read(), 2);
    assert_eq!(page.total(), 106);
    assert!(page.next().is_some());
}

#[test]
fn a_page_of_pull_requests_becomes_title_body_comment_review_and_thread_texts() {
    let page = parse_pull_texts(PULLS_LIVE).expect(RECORDED);
    assert_eq!(page.items().len(), 9);
    assert_eq!(page.read(), 2);
    assert_eq!(page.total(), 43);
}

#[test]
fn keeps_each_text_on_its_own_surface() {
    let page = parse_pull_texts(PULLS_SURFACES).expect(RECORDED);
    let survey = Survey::new(
        IssueTree::default(),
        Members::default(),
        Texts::new(page.items().to_vec()),
    );
    let surfaces: Vec<String> = lines(&survey)
        .iter()
        .map(|line| line.split(';').next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        surfaces,
        [
            "#9 old-adr-number: old ADR number in the title",
            "#9 old-adr-number: old ADR number in the body",
            "#9 old-adr-number: old ADR number in the comment",
            "#9 old-adr-number: old ADR number in the review",
            "#9 old-adr-number: old ADR number in the review comment",
        ]
    );
}

#[test]
fn an_empty_review_body_gives_no_text() {
    let page = parse_pull_texts(PULLS_SURFACES).expect(RECORDED);
    assert_eq!(page.items().len(), 5);
}

/// What the parser refused with, or nothing when it accepted the reply.
fn refusal(result: Result<TextsPage, GhError>) -> String {
    result
        .err()
        .map(|failure| failure.to_string())
        .unwrap_or_default()
}

#[test]
fn refuses_an_issue_whose_comments_were_cut_off() {
    assert_eq!(
        refusal(parse_issue_texts(ISSUES_CUT_OFF)),
        "gh answered only part of the data: #2 has more comments than the page returned"
    );
}

#[test]
fn refuses_a_pull_request_whose_comments_were_cut_off() {
    assert_eq!(
        refusal(parse_pull_texts(PULLS_CUT_OFF)),
        "gh answered only part of the data: #7 has more comments than the page returned"
    );
}

#[test]
fn refuses_a_review_thread_whose_comments_were_cut_off() {
    assert_eq!(
        refusal(parse_pull_texts(PULLS_THREAD_CUT_OFF)),
        "gh answered only part of the data: #8 has more comments in a review thread than the page returned"
    );
}

#[test]
fn refuses_an_unreadable_reply() {
    assert!(refusal(parse_issue_texts("not json")).starts_with("cannot read the answer of gh: "));
}

#[test]
fn reads_every_page_of_issues() {
    let items = GatheredTexts::default()
        .with(&parse_issue_texts(ISSUES_FIRST).expect(RECORDED))
        .with(&parse_issue_texts(ISSUES_LAST).expect(RECORDED))
        .into_items("issues")
        .expect("both pages together are every issue");
    assert_eq!(items.len(), 7);
}

#[test]
fn refuses_a_read_that_returned_fewer_pull_requests_than_the_repository_has() {
    let failure = GatheredTexts::default()
        .with(&parse_pull_texts(PULLS_LIVE).expect(RECORDED))
        .into_items("pull requests")
        .expect_err("only the first page was read");
    assert_eq!(
        failure.to_string(),
        "gh answered only part of the data: read 2 of 43 pull requests"
    );
}
