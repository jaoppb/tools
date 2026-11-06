//! What a milestone title must look like for the audit to accept it.

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The accepted shape of an open issue's milestone title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MilestoneFormat {
    /// A release such as `v0.1.0`; the only format releases can be ordered by.
    #[default]
    Semver,
    /// Any title: phases, sprints, or whatever the organization names them.
    Any,
    /// A title the regular expression matches somewhere.
    Pattern(String),
}

impl MilestoneFormat {
    /// Whether this title is acceptable. A pattern that does not compile accepts nothing;
    /// loading the configuration already rejects such patterns.
    #[must_use]
    pub fn accepts(&self, title: &str, is_release: bool) -> bool {
        match self {
            Self::Semver => is_release,
            Self::Any => true,
            Self::Pattern(pattern) => Regex::new(pattern).is_ok_and(|regex| regex.is_match(title)),
        }
    }
}

impl<'de> Deserialize<'de> for MilestoneFormat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        match text.as_str() {
            "semver" => Ok(Self::Semver),
            "any" => Ok(Self::Any),
            pattern => Regex::new(pattern)
                .map(|_| Self::Pattern(text.clone()))
                .map_err(serde::de::Error::custom),
        }
    }
}

impl Serialize for MilestoneFormat {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::Semver => "semver",
            Self::Any => "any",
            Self::Pattern(pattern) => pattern,
        })
    }
}
