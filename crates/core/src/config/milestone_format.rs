//! What a milestone title must look like for the audit to accept it.

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A compiled regular expression that compares by its source text.
#[derive(Debug, Clone)]
pub struct TitlePattern(Regex);

impl TitlePattern {
    /// The pattern for this regular expression.
    ///
    /// # Errors
    ///
    /// Returns the compile error when `source` is not a valid regular expression.
    pub fn new(source: &str) -> Result<Self, regex::Error> {
        Regex::new(source).map(Self)
    }

    /// Whether the pattern matches somewhere in `title`.
    #[must_use]
    pub fn is_match(&self, title: &str) -> bool {
        self.0.is_match(title)
    }

    /// The regular expression source.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl PartialEq for TitlePattern {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for TitlePattern {}

/// The accepted shape of an open issue's milestone title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MilestoneFormat {
    /// A release such as `v0.1.0`; the only format releases can be ordered by.
    #[default]
    Semver,
    /// Any title: phases, sprints, or whatever the organization names them.
    Any,
    /// A title the regular expression matches somewhere.
    Pattern(TitlePattern),
}

impl<'de> Deserialize<'de> for MilestoneFormat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        match text.as_str() {
            "semver" => Ok(Self::Semver),
            "any" => Ok(Self::Any),
            source => TitlePattern::new(source)
                .map(Self::Pattern)
                .map_err(serde::de::Error::custom),
        }
    }
}

impl Serialize for MilestoneFormat {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::Semver => "semver",
            Self::Any => "any",
            Self::Pattern(pattern) => pattern.as_str(),
        })
    }
}
