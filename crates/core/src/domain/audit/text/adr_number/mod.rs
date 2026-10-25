//! Old ADR numbers in a text. Only `formerly ADR-NNNN` may keep one.
//!
//! This is a small copy of the scan of `cosmo-architecture`, which reads the
//! repository's files: the two tools do not depend on each other, so the rule
//! is stated in both and review keeps them aligned.

use std::ops::RangeInclusive;

/// The numbers the ADRs had before they were named after their issue.
const OLD_NUMBERS: RangeInclusive<u16> = 1..=76;
const NUMBER_DIGITS: usize = 4;
const ALLOWED_BEFORE: &str = "formerly adr-";

/// The text before a run of digits.
struct Before<'text>(&'text str);

/// The text after a run of digits.
struct After<'text>(&'text str);

/// A run of digits.
struct Digits<'text>(&'text str);

/// A run of digits and the text around it.
struct Run<'text> {
    before: Before<'text>,
    digits: Digits<'text>,
    after: After<'text>,
}

impl Before<'_> {
    fn is_allowed(&self) -> bool {
        let start = self.0.len().saturating_sub(ALLOWED_BEFORE.len());
        self.0
            .get(start..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(ALLOWED_BEFORE))
    }

    fn names_adr(&self) -> bool {
        self.0.ends_with("ADR-") || self.0.ends_with("ADR ")
    }

    /// Whether a letter other than the `ADR` prefix touches the run, as in a
    /// digest or a commit hash.
    fn is_glued(&self) -> bool {
        let touches = self.0.chars().next_back().is_some_and(char::is_alphabetic);
        touches && !self.0.ends_with("ADR")
    }

    /// Whether a hyphen ties the run to digits before it.
    fn is_tied(&self) -> bool {
        self.0
            .strip_suffix('-')
            .and_then(|rest| rest.chars().next_back())
            .is_some_and(|character| character.is_ascii_digit())
    }
}

impl After<'_> {
    /// Whether a letter touches the run, as in a digest or a commit hash.
    fn is_glued(&self) -> bool {
        self.0.chars().next().is_some_and(char::is_alphabetic)
    }

    /// Whether a hyphen ties the run to digits after it.
    fn is_tied(&self) -> bool {
        self.0
            .strip_prefix('-')
            .and_then(|rest| rest.chars().next())
            .is_some_and(|character| character.is_ascii_digit())
    }
}

impl Digits<'_> {
    const fn is_number_sized(&self) -> bool {
        self.0.len() == NUMBER_DIGITS
    }

    fn is_old_number(&self) -> bool {
        self.0
            .parse::<u16>()
            .is_ok_and(|value| OLD_NUMBERS.contains(&value))
    }
}

impl Run<'_> {
    /// Whether a hyphen ties the run to digits on the other side, as in a CNPJ
    /// branch or an advisory id, which are not ADR numbers.
    fn joins_digits(&self) -> bool {
        self.before.is_tied() || self.after.is_tied()
    }

    /// Whether a letter touches the run, so it is part of a word or of a
    /// hexadecimal digest and not an ADR number.
    fn is_glued(&self) -> bool {
        self.before.is_glued() || self.after.is_glued()
    }

    fn is_reported(&self) -> bool {
        let counts = self.digits.is_number_sized() && !self.before.is_allowed();
        let bare = self.digits.is_old_number() && !self.is_glued();
        counts && !self.joins_digits() && (self.before.names_adr() || bare)
    }
}

/// Whether `text` names an ADR by an old number.
pub(super) fn names_old_number(text: &str) -> bool {
    runs(text).iter().any(Run::is_reported)
}

fn runs(text: &str) -> Vec<Run<'_>> {
    let offsets: Vec<usize> = text
        .char_indices()
        .filter_map(|(offset, character)| character.is_ascii_digit().then_some(offset))
        .collect();
    offsets
        .chunk_by(|left, right| left.saturating_add(1) == *right)
        .filter_map(|chunk| run_of(text, chunk))
        .collect()
}

fn run_of<'text>(text: &'text str, chunk: &[usize]) -> Option<Run<'text>> {
    let start = *chunk.first()?;
    let end = chunk.last()?.saturating_add(1);
    Some(Run {
        before: Before(text.get(..start)?),
        digits: Digits(text.get(start..end)?),
        after: After(text.get(end..)?),
    })
}
