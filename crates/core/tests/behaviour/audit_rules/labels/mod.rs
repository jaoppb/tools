//! Every issue has exactly one type label and an area label.

use crate::support::audit::{closed, epic, labelled, lines, story, survey};

const TWO_TYPES: [&str; 3] = ["type:feat", "type:fix", "area:tooling"];
const FIXED: [&str; 2] = ["type:feat", "area:tooling"];

#[test]
fn reports_an_issue_with_two_type_labels() {
    let broken = survey(vec![epic(1), labelled(story(10, 1), &TWO_TYPES)]);
    assert_eq!(
        lines(&broken),
        ["#10 not-one-type: not exactly one type label"]
    );
    let repaired = survey(vec![epic(1), labelled(story(10, 1), &FIXED)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn reports_an_issue_with_no_type_label() {
    let broken = survey(vec![epic(1), labelled(story(10, 1), &["area:tooling"])]);
    assert_eq!(
        lines(&broken),
        ["#10 not-one-type: not exactly one type label"]
    );
}

#[test]
fn reports_an_issue_with_no_area_label() {
    let broken = survey(vec![epic(1), labelled(story(10, 1), &["type:feat"])]);
    assert_eq!(lines(&broken), ["#10 no-area: no area label"]);
    let repaired = survey(vec![epic(1), labelled(story(10, 1), &FIXED)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn reports_a_closed_issue_with_no_area_label() {
    let broken = survey(vec![epic(1), labelled(closed(10, 1), &["type:feat"])]);
    assert_eq!(lines(&broken), ["#10 no-area: no area label"]);
    let repaired = survey(vec![epic(1), labelled(closed(10, 1), &FIXED)]);
    assert!(lines(&repaired).is_empty());
}
