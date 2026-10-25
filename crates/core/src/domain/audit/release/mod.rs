//! The release a milestone title names, ordered so two releases can be compared.

use crate::domain::facts::Milestone;

/// The major number of a release, such as the 1 of `v1.1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Major(u32);

/// The minor number of a release, such as the 1 of `v1.1`. `v1` has minor 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Minor(u32);

/// A release: `v1` is 1.0 and `v1.1` is 1.1. Releases order by major, then minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Release {
    major: Major,
    minor: Minor,
}

impl Release {
    /// The release a milestone names: `v` and a whole number, optionally a dot
    /// and another whole number. Any other title is not a release.
    #[must_use]
    pub fn of(milestone: &Milestone) -> Option<Self> {
        let title = milestone.to_string();
        let numbers = title.strip_prefix('v')?;
        let (major, minor) = numbers.split_once('.').unwrap_or((numbers, "0"));
        Some(Self {
            major: Major(whole(major)?),
            minor: Minor(whole(minor)?),
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
