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

#[test]
fn accepts_accented_area_label() {
    let accented = ["type:feat", "área:segurança"];
    let pass = survey(vec![epic(1), labelled(story(10, 1), &accented)]);
    assert!(lines(&pass).is_empty());
}

#[test]
fn accepts_configured_area_prefix_alias() {
    let custom = ["type:feat", "domain:auth"];
    let survey_item = survey(vec![epic(1), labelled(story(10, 1), &custom)]);

    // Fails under default rules (area_prefixes = ["area:"])
    assert_eq!(lines(&survey_item), ["#10 no-area: no area label"]);

    // Passes under custom rules where domain: is an alias
    let custom_rules = tools_core::config::RulesConfig {
        area_prefixes: vec!["area:".to_string(), "domain:".to_string()],
        ..tools_core::config::RulesConfig::default()
    };
    let custom_lines: Vec<String> = survey_item
        .violations(&custom_rules)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(custom_lines.is_empty());
}
