//! The reply parsers of issue texts and pull request texts, and the check that
//! the pages read are all of them, read against recorded and hand-written
//! replies.

use tools_core::adapters::github::parse::{
    CutOff, TextsPage, parse_issue_page, parse_issue_texts, parse_item_comments, parse_pull_page,
    parse_pull_review_threads, parse_pull_texts, parse_thread_comments,
};
use tools_core::adapters::github::{GatheredTexts, GhError};
use tools_core::domain::{
    IssueNumber, IssueTree, Members, Repo, Surface, Survey, Text, TextItem, Texts,
};

use crate::support::audit::lines;

fn cosmo() -> Repo {
    Repo::new("jaoppb", "cosmo")
}

const ISSUES_LIVE: &str = include_str!("../../fixtures/texts_issues_live.json");
const ISSUES_FIRST: &str = include_str!("../../fixtures/texts_issues_first_of_two.json");
const ISSUES_LAST: &str = include_str!("../../fixtures/texts_issues_last_of_two.json");
const ISSUES_CUT_OFF: &str = include_str!("../../fixtures/texts_issues_cut_off.json");
const PULLS_LIVE: &str = include_str!("../../fixtures/texts_pulls_live.json");
const PULLS_SURFACES: &str = include_str!("../../fixtures/texts_pulls_surfaces.json");
const PULLS_CUT_OFF: &str = include_str!("../../fixtures/texts_pulls_cut_off.json");
const PULLS_THREAD_CUT_OFF: &str = include_str!("../../fixtures/texts_pulls_thread_cut_off.json");
const ISSUE_COMMENTS_FOLLOW_UP: &str =
    include_str!("../../fixtures/texts_issue_comments_follow_up.json");
const PULL_COMMENTS_FOLLOW_UP: &str =
    include_str!("../../fixtures/texts_pull_comments_follow_up.json");
const PULL_REVIEWS_FOLLOW_UP: &str =
    include_str!("../../fixtures/texts_pull_reviews_follow_up.json");
const PULL_THREADS_FOLLOW_UP: &str =
    include_str!("../../fixtures/texts_pull_threads_follow_up.json");
const THREAD_COMMENTS_FOLLOW_UP: &str =
    include_str!("../../fixtures/texts_thread_comments_follow_up.json");

const RECORDED: &str = "the recorded reply parses";

#[test]
fn a_page_of_issues_becomes_title_body_and_comment_texts() {
    let page = parse_issue_texts(ISSUES_LIVE, &cosmo()).expect(RECORDED);
    assert_eq!(page.items().len(), 5);
    assert_eq!(page.read(), 2);
    assert_eq!(page.total(), 106);
    assert!(page.next().is_some());
}

#[test]
fn a_page_of_pull_requests_becomes_title_body_comment_review_and_thread_texts() {
    let page = parse_pull_texts(PULLS_LIVE, &cosmo()).expect(RECORDED);
    assert_eq!(page.items().len(), 9);
    assert_eq!(page.read(), 2);
    assert_eq!(page.total(), 43);
}

#[test]
fn keeps_each_text_on_its_own_surface() {
    let page = parse_pull_texts(PULLS_SURFACES, &cosmo()).expect(RECORDED);
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
            "cosmo#9 old-adr-number: old ADR number in the title",
            "cosmo#9 old-adr-number: old ADR number in the body",
            "cosmo#9 old-adr-number: old ADR number in the comment",
            "cosmo#9 old-adr-number: old ADR number in the review",
            "cosmo#9 old-adr-number: old ADR number in the review comment",
        ]
    );
}

#[test]
fn an_empty_review_body_gives_no_text() {
    let page = parse_pull_texts(PULLS_SURFACES, &cosmo()).expect(RECORDED);
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
        refusal(parse_issue_texts(ISSUES_CUT_OFF, &cosmo())),
        "gh answered only part of the data: cosmo#2 has more comments than the page returned"
    );
}

#[test]
fn refuses_a_pull_request_whose_comments_were_cut_off() {
    assert_eq!(
        refusal(parse_pull_texts(PULLS_CUT_OFF, &cosmo())),
        "gh answered only part of the data: cosmo#7 has more comments than the page returned"
    );
}

#[test]
fn refuses_a_review_thread_whose_comments_were_cut_off() {
    assert_eq!(
        refusal(parse_pull_texts(PULLS_THREAD_CUT_OFF, &cosmo())),
        "gh answered only part of the data: cosmo#8 has more comments in a review thread than the page returned"
    );
}

#[test]
fn refuses_an_unreadable_reply() {
    assert!(
        refusal(parse_issue_texts("not json", &cosmo()))
            .starts_with("cannot read the answer of gh: ")
    );
}

#[test]
fn reads_every_page_of_issues() {
    let items = GatheredTexts::default()
        .with(&parse_issue_texts(ISSUES_FIRST, &cosmo()).expect(RECORDED))
        .with(&parse_issue_texts(ISSUES_LAST, &cosmo()).expect(RECORDED))
        .into_items("issues")
        .expect("both pages together are every issue");
    assert_eq!(items.len(), 7);
}

#[test]
fn refuses_a_read_that_returned_fewer_pull_requests_than_the_repository_has() {
    let failure = GatheredTexts::default()
        .with(&parse_pull_texts(PULLS_LIVE, &cosmo()).expect(RECORDED))
        .into_items("pull requests")
        .expect_err("only the first page was read");
    assert_eq!(
        failure.to_string(),
        "gh answered only part of the data: read 2 of 43 pull requests"
    );
}

#[test]
fn parses_follow_up_issue_comments() {
    let number = IssueNumber::new(cosmo(), 2);
    let (items, tail) =
        parse_item_comments(ISSUE_COMMENTS_FOLLOW_UP, &number, Surface::Comment).expect(RECORDED);
    assert_eq!(
        items,
        [TextItem::new(
            number,
            Surface::Comment,
            Text::new("Follow-up comment on issue 2")
        )]
    );
    assert!(!tail.is_cut_off());
    assert_eq!(tail.cursor(), Some("cursor_issue_c2"));
}

#[test]
fn parses_follow_up_pull_comments() {
    let number = IssueNumber::new(cosmo(), 7);
    let (items, tail) =
        parse_item_comments(PULL_COMMENTS_FOLLOW_UP, &number, Surface::Comment).expect(RECORDED);
    assert_eq!(
        items,
        [TextItem::new(
            number,
            Surface::Comment,
            Text::new("Follow-up comment on pull 7")
        )]
    );
    assert!(!tail.is_cut_off());
}

#[test]
fn parses_follow_up_pull_reviews() {
    let number = IssueNumber::new(cosmo(), 7);
    let (items, tail) =
        parse_item_comments(PULL_REVIEWS_FOLLOW_UP, &number, Surface::Review).expect(RECORDED);
    assert_eq!(
        items,
        [TextItem::new(
            number,
            Surface::Review,
            Text::new("Follow-up review body on pull 7")
        )]
    );
    assert!(!tail.is_cut_off());
}

#[test]
fn parses_follow_up_pull_review_threads() {
    let number = IssueNumber::new(cosmo(), 7);
    let (items, cut_offs, tail) =
        parse_pull_review_threads(PULL_THREADS_FOLLOW_UP, &number).expect(RECORDED);
    assert_eq!(
        items,
        [TextItem::new(
            number,
            Surface::ReviewComment,
            Text::new("Thread comment from paginated thread")
        )]
    );
    assert!(cut_offs.is_empty());
    assert!(!tail.is_cut_off());
}

#[test]
fn parses_follow_up_thread_comments() {
    let number = IssueNumber::new(cosmo(), 8);
    let (items, tail) = parse_thread_comments(THREAD_COMMENTS_FOLLOW_UP, &number).expect(RECORDED);
    assert_eq!(
        items,
        [TextItem::new(
            number,
            Surface::ReviewComment,
            Text::new("Follow-up thread comment")
        )]
    );
    assert!(!tail.is_cut_off());
}

#[test]
fn issue_page_identifies_cut_off_comments_for_pagination() {
    let parsed = parse_issue_page(ISSUES_CUT_OFF, &cosmo()).expect(RECORDED);
    let (page, cut_offs) = parsed.into_parts();
    assert_eq!(page.items().len(), 5);
    assert_eq!(cut_offs.len(), 1);
    match &cut_offs[0] {
        CutOff::IssueComments(number, tail) => {
            assert_eq!(*number, IssueNumber::new(cosmo(), 2));
            assert!(tail.is_cut_off());
        }
        other => panic!("expected IssueComments cut-off, got {other:?}"),
    }
}

#[test]
fn pull_page_identifies_cut_off_comments_and_threads_for_pagination() {
    let parsed = parse_pull_page(PULLS_CUT_OFF, &cosmo()).expect(RECORDED);
    let (_, cut_offs) = parsed.into_parts();
    assert_eq!(cut_offs.len(), 1);
    match &cut_offs[0] {
        CutOff::PullComments(number, tail) => {
            assert_eq!(*number, IssueNumber::new(cosmo(), 7));
            assert!(tail.is_cut_off());
        }
        other => panic!("expected PullComments cut-off, got {other:?}"),
    }

    let thread_parsed = parse_pull_page(PULLS_THREAD_CUT_OFF, &cosmo()).expect(RECORDED);
    let (_, thread_cut_offs) = thread_parsed.into_parts();
    assert_eq!(thread_cut_offs.len(), 1);
    match &thread_cut_offs[0] {
        CutOff::ThreadComments(number, id, tail) => {
            assert_eq!(*number, IssueNumber::new(cosmo(), 8));
            assert!(id.is_none());
            assert!(tail.is_cut_off());
        }
        other => panic!("expected ThreadComments cut-off, got {other:?}"),
    }
}
