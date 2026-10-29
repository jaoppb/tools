//! The release a milestone title names, ordered so two releases can be compared.

use crate::domain::facts::Milestone;

/// A release: `v1` is 1.0.0 and `v0.1.0` is 0.1.0. Releases order by major, minor, then patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Release {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Release {
    /// The release a milestone names: `v` and up to three dot-separated whole numbers.
    /// Missing minor and patch components default to 0. Any other title is not a release.
    #[must_use]
    pub fn of(milestone: &Milestone) -> Option<Self> {
        let title = milestone.to_string();
        let numbers = title.strip_prefix('v')?;
        let mut parts = numbers.split('.');
        let major = whole(parts.next()?)?;
        let parse_part = |part: Option<&str>| match part {
            Some(s) => whole(s),
            None => Some(0),
        };
        let minor = parse_part(parts.next())?;
        let patch = parse_part(parts.next())?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    /// Whether this release comes after `other`.
    #[must_use]
    pub fn is_later_than(&self, other: &Self) -> bool {
        self > other
    }
}

/// A whole number written with digits only: no sign, no blank, no text.
fn whole(text: &str) -> Option<u32> {
    let digits = !text.is_empty() && text.chars().all(|character| character.is_ascii_digit());
    digits.then(|| text.parse().ok()).flatten()
}
