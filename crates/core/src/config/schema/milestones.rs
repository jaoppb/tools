//! `[github.milestones]` and `[github.overrides.<repo>.milestones]`.

use super::{UnmanagedConfig, default_milestones};
use crate::domain::repo::{
    MilestoneState, RepoMilestoneSpec, RepoMilestonesOverlay, RepoMilestonesSpec,
};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Whether a declared milestone is open or closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum MilestoneStateConfig {
    #[default]
    Open,
    Closed,
}

impl From<MilestoneStateConfig> for MilestoneState {
    fn from(config: MilestoneStateConfig) -> Self {
        match config {
            MilestoneStateConfig::Open => Self::Open,
            MilestoneStateConfig::Closed => Self::Closed,
        }
    }
}

/// One declared milestone.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MilestoneConfig {
    /// Milestone title (e.g. `v0`).
    title: String,
    /// Milestone description.
    #[serde(default)]
    description: String,
    /// Milestone state.
    #[serde(default)]
    state: MilestoneStateConfig,
}

impl From<MilestoneConfig> for RepoMilestoneSpec {
    fn from(config: MilestoneConfig) -> Self {
        Self {
            title: config.title,
            description: config.description,
            state: config.state.into(),
        }
    }
}

/// The shared `[github.milestones]` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilestonesConfig {
    /// Milestone titles to close/prune.
    #[serde(default)]
    deprecated: Vec<String>,
    /// Rename mappings from old milestone title to new milestone title.
    #[serde(default)]
    renames: BTreeMap<String, String>,
    /// What to do with milestones that exist on GitHub but are not declared.
    #[serde(default = "default_milestones")]
    unmanaged: UnmanagedConfig,
    /// Desired milestones to enforce.
    #[serde(default)]
    items: Vec<MilestoneConfig>,
}

impl From<MilestonesConfig> for RepoMilestonesSpec {
    fn from(config: MilestonesConfig) -> Self {
        Self {
            deprecated: config.deprecated,
            renames: config.renames,
            unmanaged: config.unmanaged.into(),
            items: config.items.into_iter().map(Into::into).collect(),
        }
    }
}

/// A repository's `[github.overrides.<repo>.milestones]` overlay.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilestonesOverlayConfig {
    /// Additional milestone titles to close/prune.
    #[serde(default)]
    deprecated: Vec<String>,
    /// Additional or overriding rename mappings.
    #[serde(default)]
    renames: BTreeMap<String, String>,
    /// Overrides what to do with undeclared milestones when set.
    #[serde(default)]
    unmanaged: Option<UnmanagedConfig>,
    /// Additional or overriding milestones.
    #[serde(default)]
    items: Vec<MilestoneConfig>,
}

impl From<MilestonesOverlayConfig> for RepoMilestonesOverlay {
    fn from(config: MilestonesOverlayConfig) -> Self {
        Self {
            deprecated: config.deprecated,
            renames: config.renames,
            unmanaged: config.unmanaged.map(Into::into),
            items: config.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::Unmanaged;

    #[test]
    fn unmanaged_defaults_to_error_for_milestones() {
        let config: MilestonesConfig = toml::from_str("items = []").unwrap();
        assert_eq!(RepoMilestonesSpec::from(config).unmanaged, Unmanaged::Error);
    }

    #[test]
    fn unmanaged_policy_parses_every_value_and_rejects_others() {
        for (word, policy) in [
            ("ignore", Unmanaged::Ignore),
            ("warn", Unmanaged::Warn),
            ("error", Unmanaged::Error),
            ("prune", Unmanaged::Prune),
        ] {
            let config: MilestonesConfig =
                toml::from_str(&format!("unmanaged = \"{word}\"")).unwrap();
            assert_eq!(RepoMilestonesSpec::from(config).unmanaged, policy);
        }
        assert!(toml::from_str::<MilestonesConfig>("unmanaged = \"loud\"").is_err());
        assert!(toml::from_str::<MilestonesConfig>("prune_unmanaged = true").is_err());
    }
}
