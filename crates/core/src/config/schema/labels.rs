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
