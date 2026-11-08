//! `[github.labels]` and `[github.overrides.<repo>.labels]`.

use super::{UnmanagedConfig, default_labels};
use crate::domain::repo::{RepoLabel, RepoLabelsOverlay, RepoLabelsSpec};
use serde::Deserialize;
use std::collections::BTreeMap;

/// One declared label.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LabelConfig {
    /// Label name (e.g. `type:feat`).
    name: String,
    /// Hex color, with or without `#`, any case.
    color: String,
    /// Label description.
    #[serde(default)]
    description: String,
}

impl From<LabelConfig> for RepoLabel {
    fn from(config: LabelConfig) -> Self {
        Self::new(config.name, &config.color, config.description)
    }
}

/// The shared `[github.labels]` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelsConfig {
    /// Label names to prune.
    #[serde(default)]
    deprecated: Vec<String>,
    /// Rename mappings from old label name to new label name.
    #[serde(default)]
    renames: BTreeMap<String, String>,
    /// What to do with labels that exist on GitHub but are not declared.
    #[serde(default = "default_labels")]
    unmanaged: UnmanagedConfig,
    /// Mappings from legacy issue labels to canonical labels.
    #[serde(default)]
    migrations: BTreeMap<String, String>,
    /// Desired labels to enforce.
    #[serde(default)]
    items: Vec<LabelConfig>,
}

impl LabelsConfig {
    /// Mappings from legacy issue labels to canonical labels.
    #[must_use]
    pub const fn migrations(&self) -> &BTreeMap<String, String> {
        &self.migrations
    }
}

impl From<LabelsConfig> for RepoLabelsSpec {
    fn from(config: LabelsConfig) -> Self {
        Self {
            deprecated: config.deprecated,
            renames: config.renames,
            unmanaged: config.unmanaged.into(),
            migrations: config.migrations,
            items: config.items.into_iter().map(Into::into).collect(),
        }
    }
}

/// A repository's `[github.overrides.<repo>.labels]` overlay.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelsOverlayConfig {
    /// Additional label names to prune.
    #[serde(default)]
    deprecated: Vec<String>,
    /// Additional or overriding rename mappings.
    #[serde(default)]
    renames: BTreeMap<String, String>,
    /// Overrides what to do with undeclared labels when set.
    #[serde(default)]
    unmanaged: Option<UnmanagedConfig>,
    /// Additional or overriding migration mappings.
    #[serde(default)]
    migrations: BTreeMap<String, String>,
    /// Additional or overriding labels.
    #[serde(default)]
    items: Vec<LabelConfig>,
}

impl From<LabelsOverlayConfig> for RepoLabelsOverlay {
    fn from(config: LabelsOverlayConfig) -> Self {
        Self {
            deprecated: config.deprecated,
            renames: config.renames,
            unmanaged: config.unmanaged.map(Into::into),
            migrations: config.migrations,
            items: config.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repo::Unmanaged;

    #[test]
    fn a_full_table_maps_into_the_domain_spec() {
        let config: LabelsConfig = toml::from_str(
            r##"
            deprecated = ["bug"]
            renames = { old = "new" }
            unmanaged = "ignore"
            migrations = { ui = "area:web" }
            items = [{ name = "type:feat", color = "#1D76DB" }]
            "##,
        )
        .unwrap();
        let spec = RepoLabelsSpec::from(config);
        assert_eq!(spec.deprecated, ["bug"]);
        assert_eq!(spec.renames["old"], "new");
        assert_eq!(spec.unmanaged, Unmanaged::Ignore);
        assert_eq!(spec.migrations["ui"], "area:web");
        assert_eq!(spec.items, [RepoLabel::new("type:feat", "1d76db", "")]);
    }

    #[test]
    fn an_empty_table_takes_the_defaults() {
        let spec = RepoLabelsSpec::from(toml::from_str::<LabelsConfig>("").unwrap());
        assert_eq!(spec, RepoLabelsSpec::default());
    }

    #[test]
    fn strict_tables_reject_unknown_keys() {
        assert!(toml::from_str::<LabelsConfig>("typo = 1").is_err());
        assert!(toml::from_str::<LabelsOverlayConfig>("typo = 1").is_err());
        assert!(toml::from_str::<LabelsConfig>("unmanaged = \"loud\"").is_err());
    }

    #[test]
    fn an_overlay_leaves_unmanaged_unset_unless_given() {
        let overlay: LabelsOverlayConfig = toml::from_str("").unwrap();
        assert_eq!(RepoLabelsOverlay::from(overlay).unmanaged, None);
        let overlay: LabelsOverlayConfig = toml::from_str("unmanaged = \"prune\"").unwrap();
        assert_eq!(
            RepoLabelsOverlay::from(overlay).unmanaged,
            Some(Unmanaged::Prune)
        );
    }
}
