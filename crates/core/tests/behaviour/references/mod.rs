//! What the scanner finds in a text: ADRs named by an old number and `spdd/`
//! files that were deleted. The forms are taken from the real texts of the
//! repository.

use tools_core::domain::{Reference, Text};

fn found(words: &str) -> Vec<Reference> {
    Text::new(words).references()
}

const OLD: [Reference; 1] = [Reference::OldAdrNumber];
const DELETED: [Reference; 1] = [Reference::DeletedSpddPath];

#[test]
fn reports_a_number_after_adr_and_a_hyphen() {
    assert_eq!(found("Decided in ADR-0013."), OLD);
}

#[test]
fn reports_a_number_after_adr_and_a_space() {
    assert_eq!(found("the title of ADR 0052 changes"), OLD);
}

#[test]
fn reports_a_number_after_adr_even_when_it_is_out_of_range() {
    assert_eq!(found("ADR-0100 was never written"), OLD);
}

#[test]
fn reports_a_bare_number_in_a_list_item() {
    assert_eq!(found("- 0029: the database file"), OLD);
}

#[test]
fn reports_each_end_of_a_range_written_with_words() {
    assert_eq!(found("ADRs 0001 to 0010 were merged"), OLD);
}

#[test]
fn reports_a_range_written_with_an_en_dash() {
    assert_eq!(found("see 0029\u{2013}0033"), OLD);
}

#[test]
fn reports_a_number_in_an_adr_file_stem() {
    assert_eq!(found("docs/adr/0013-use-turso.md"), OLD);
}

#[test]
fn accepts_the_formerly_form_in_any_letter_case() {
    assert!(found("formerly ADR-0013").is_empty());
    assert!(found("Formerly ADR-0013").is_empty());
    assert!(found("FORMERLY ADR-0013").is_empty());
}

#[test]
fn reports_a_formerly_form_that_is_not_adjacent_to_the_number() {
    assert_eq!(found("formerly the ADR-0013 record"), OLD);
}

#[test]
fn ignores_digits_that_a_hyphen_ties_to_other_digits() {
    assert!(found("CNPJ 12.345.678/0001-95").is_empty());
    assert!(found("advisory 2026-0013").is_empty());
}

#[test]
fn ignores_a_year_a_count_and_a_number_of_another_length() {
    assert!(found("in 2026 there were 0100 items, id 00130 and 13").is_empty());
}

#[test]
fn ignores_four_digits_inside_a_sha256_digest() {
    assert!(
        found("image sha256:bebcdfd8c9c9d0014d0bea11dbb3c8d1b6a94a262d53f0abe03dc0aa00b22649")
            .is_empty()
    );
}

#[test]
fn ignores_four_digits_inside_a_commit_hash() {
    assert!(found("fixed in e0013a5cfafe91a99c34b52e32c1feeda86a34bf").is_empty());
    assert!(
        found("https://github.com/jaoppb/cosmo/commit/e0013a5cfafe91a99c34b52e32c1feeda86a34bf")
            .is_empty()
    );
}

#[test]
fn still_reports_a_number_next_to_punctuation_or_adr() {
    assert_eq!(found("(0014)"), OLD);
    assert_eq!(found("`0014`, and more"), OLD);
    assert_eq!(found("ADR0014"), OLD);
}

#[test]
fn a_text_with_three_numbers_still_names_one_reference() {
    assert_eq!(found("ADR-0013, ADR-0014 and 0015"), OLD);
}

#[test]
fn reports_a_spdd_path() {
    let words = "see spdd/analysis/GH-6-202610010930-[Feat]-stock.md";
    assert_eq!(found(words), DELETED);
}

#[test]
fn reports_a_wildcard_and_an_ellipsis_of_a_deleted_issue() {
    assert_eq!(found("spdd/prompt/GH-64-*"), DELETED);
    assert_eq!(found("spdd/analysis/GH-6-\u{2026}"), DELETED);
}

#[test]
fn reports_a_spdd_path_whether_or_not_the_issue_has_a_file() {
    let words = "spdd/prompt/GH-144-202610150853-[Chore]-tooling.md";
    assert_eq!(found(words), DELETED);
    assert_eq!(found("spdd/prompt/GH-143-x.md"), DELETED);
}

#[test]
fn ignores_a_placeholder_and_the_bare_directory() {
    assert!(found("spdd/analysis/GH-<n>-<slug>.md").is_empty());
    assert!(found("the spdd/analysis/ directory").is_empty());
}

#[test]
fn ignores_spdd_glued_to_a_word_or_a_path() {
    assert!(found("myspdd/prompt/GH-6-x.md").is_empty());
    let link = "https://github.com/jaoppb/cosmo/blob/abc123/spdd/prompt/GH-6-x.md";
    assert!(found(link).is_empty());
}

#[test]
fn reports_both_kinds_in_the_order_of_the_rules() {
    let words = "ADR-0013 and spdd/prompt/GH-6-x.md";
    assert_eq!(
        found(words),
        [Reference::OldAdrNumber, Reference::DeletedSpddPath]
    );
}

#[test]
fn a_text_with_no_reference_names_nothing() {
    assert!(found("Links [the ADR](docs/adr/gh-143-x.md) by title.").is_empty());
    assert!(found("").is_empty());
}
