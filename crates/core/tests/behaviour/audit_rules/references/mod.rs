//! No text of an issue or a pull request names an ADR by an old number or a
//! `spdd/` file.

use tools_core::domain::Surface;
use tools_core::domain::audit::rules::references::LOG_CARRIER;

use crate::support::audit::{lines, survey_of_texts, text};

const OLD_NUMBER: &str = "old ADR number in the";
const OLD_ADVICE: &str = "link the ADR by its title (only formerly ADR-NNNN may keep one)";

fn old_number_line(item: u64, surface: &str) -> String {
    format!("#{item} old-adr-number: {OLD_NUMBER} {surface}; {OLD_ADVICE}")
}

fn deleted_path_line(item: u64, surface: &str) -> String {
    format!(
        "#{item} deleted-spdd-path: spdd file named in the {surface}; canvases are not committed, name the issue as #<n> or link the file at a commit"
    )
}

#[test]
fn reports_an_old_number_in_a_body_and_passes_once_repaired() {
    let broken = survey_of_texts(vec![text(10, Surface::Body, "See ADR-0013.")]);
    assert_eq!(lines(&broken), [old_number_line(10, "body")]);
    let repaired = survey_of_texts(vec![text(
        10,
        Surface::Body,
        "See [the ADR](docs/adr/x.md).",
    )]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn names_the_surface_of_each_finding() {
    let surfaces = [
        (Surface::Title, "title"),
        (Surface::Body, "body"),
        (Surface::Comment, "comment"),
        (Surface::Review, "review"),
        (Surface::ReviewComment, "review comment"),
    ];
    for (surface, label) in surfaces {
        let broken = survey_of_texts(vec![text(10, surface, "ADR-0013")]);
        assert_eq!(lines(&broken), [old_number_line(10, label)]);
    }
}

#[test]
fn reports_a_surface_once_however_many_numbers_it_holds() {
    let broken = survey_of_texts(vec![text(
        10,
        Surface::Body,
        "ADR-0013, ADR-0014 and ADR-0015",
    )]);
    assert_eq!(lines(&broken), [old_number_line(10, "body")]);
}

#[test]
fn reports_each_surface_that_holds_a_number() {
    let broken = survey_of_texts(vec![
        text(10, Surface::Body, "ADR-0013"),
        text(10, Surface::Comment, "ADR-0013"),
    ]);
    assert_eq!(
        lines(&broken),
        [old_number_line(10, "body"), old_number_line(10, "comment")]
    );
}

#[test]
fn accepts_the_formerly_form() {
    let accepted = survey_of_texts(vec![text(10, Surface::Body, "Merged (formerly ADR-0013).")]);
    assert!(lines(&accepted).is_empty());
}

#[test]
fn reports_a_spdd_path_and_passes_once_repaired() {
    let words = "see spdd/analysis/GH-6-202610010930-[Feat]-stock.md";
    let broken = survey_of_texts(vec![text(10, Surface::Body, words)]);
    assert_eq!(lines(&broken), [deleted_path_line(10, "body")]);
    let repaired = survey_of_texts(vec![text(10, Surface::Body, "see #6")]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn reports_both_kinds_of_one_surface_in_rule_order() {
    let words = "ADR-0013 and spdd/prompt/GH-6-x.md";
    let broken = survey_of_texts(vec![text(10, Surface::Body, words)]);
    assert_eq!(
        lines(&broken),
        [old_number_line(10, "body"), deleted_path_line(10, "body")]
    );
}

#[test]
fn sorts_the_findings_by_number_and_then_by_rule() {
    let broken = survey_of_texts(vec![
        text(12, Surface::Body, "ADR-0013"),
        text(11, Surface::Title, "spdd/prompt/GH-6-x.md"),
        text(11, Surface::Body, "ADR-0013"),
    ]);
    assert_eq!(
        lines(&broken),
        [
            old_number_line(11, "body"),
            deleted_path_line(11, "title"),
            old_number_line(12, "body"),
        ]
    );
}

#[test]
fn a_repository_without_texts_has_no_findings() {
    assert!(lines(&survey_of_texts(Vec::new())).is_empty());
}

#[test]
fn never_reports_the_text_of_the_log_carrier() {
    let carrier = LOG_CARRIER.value();
    let survey = survey_of_texts(vec![
        text(carrier, Surface::Body, "ADR-0013"),
        text(
            carrier,
            Surface::Comment,
            "spdd/prompt/GH-6-x.md and ADR-0014",
        ),
    ]);
    assert!(lines(&survey).is_empty());
}

#[test]
fn still_reports_the_neighbours_of_the_log_carrier() {
    let carrier = LOG_CARRIER.value();
    let survey = survey_of_texts(vec![
        text(carrier.saturating_sub(1), Surface::Body, "ADR-0013"),
        text(carrier.saturating_add(1), Surface::Body, "ADR-0013"),
    ]);
    assert_eq!(lines(&survey).len(), 2);
}
