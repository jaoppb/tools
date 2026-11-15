//! The `[actions]` table of `tools.toml`: which workflow-reference rules are on.

use serde::Deserialize;

use crate::domain::actions::{PinFormat, Rules};

/// The ref style `pin_format` demands, as spelled in `tools.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PinFormatConfig {
    /// A full commit SHA with a trailing `# vX` comment.
    #[serde(rename = "sha-with-comment")]
    ShaWithComment,
    /// A version tag (`@vX`).
    #[serde(rename = "tag")]
    Tag,
}

/// `[actions]`; every rule is off until set.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionsConfig {
    /// The referenced major must be the latest major.
    #[serde(default)]
    latest_major: bool,
    /// The demanded ref style.
    #[serde(default)]
    pin_format: Option<PinFormatConfig>,
    /// A SHA must be a tag of the major its comment names.
    #[serde(default)]
    sha_matches_comment: bool,
    /// Only these repositories may be used.
    #[serde(default)]
    allow: Option<Vec<String>>,
    /// Repositories that may not be used.
    #[serde(default)]
    deny: Vec<String>,
    /// Repositories exempt from every rule.
    #[serde(default)]
    ignore: Vec<String>,
}

impl From<ActionsConfig> for Rules {
    fn from(config: ActionsConfig) -> Self {
        Self {
            latest_major: config.latest_major,
            pin_format: config.pin_format.map(|format| match format {
                PinFormatConfig::ShaWithComment => PinFormat::ShaWithComment,
                PinFormatConfig::Tag => PinFormat::Tag,
            }),
            sha_matches_comment: config.sha_matches_comment,
            allow: config.allow,
            deny: config.deny,
            ignore: config.ignore,
        }
    }
}
