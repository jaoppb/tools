//! `[github.labels]` and `[github.overrides.<repo>.labels]`.

use super::{UnmanagedConfig, default_labels};
use crate::domain::repo::{RepoLabel, RepoLabelsOverlay, RepoLabelsSpec};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};

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
    /// Docs, relative to the directory holding `tools.toml`, whose label block `tools repo check`
    /// compares with the declared labels. Configuration only: it never reaches the domain.
    #[serde(default, deserialize_with = "unique_docs")]
    docs: Vec<String>,
}

impl LabelsConfig {
    /// Mappings from legacy issue labels to canonical labels.
    #[must_use]
    pub const fn migrations(&self) -> &BTreeMap<String, String> {
        &self.migrations
    }

    /// The docs whose label block is checked; empty means no doc is checked.
    #[must_use]
    pub fn docs(&self) -> &[String] {
        &self.docs
    }
}

/// The `docs` list, rejecting the same string listed twice (paths are compared as written).
fn unique_docs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let paths = Vec::<String>::deserialize(deserializer)?;
    let mut seen = BTreeSet::new();
    match paths.iter().find(|path| !seen.insert(path.as_str())) {
        Some(path) => Err(serde::de::Error::custom(format!(
            "`{path}` is listed twice in docs"
        ))),
        None => Ok(paths),
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
        let config = toml::from_str::<LabelsConfig>("").unwrap();
        assert!(config.docs().is_empty());
        assert_eq!(RepoLabelsSpec::from(config), RepoLabelsSpec::default());
    }

    #[test]
    fn docs_are_kept_in_order_and_a_duplicate_is_rejected() {
        let config: LabelsConfig =
            toml::from_str(r#"docs = ["CONTRIBUTING.md", "AGENTS.md"]"#).unwrap();
        assert_eq!(config.docs(), ["CONTRIBUTING.md", "AGENTS.md"]);

        let error = toml::from_str::<LabelsConfig>(r#"docs = ["AGENTS.md", "AGENTS.md"]"#)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("`AGENTS.md` is listed twice in docs"),
            "{error}"
        );
    }

    #[test]
    fn an_overlay_rejects_docs() {
        assert!(toml::from_str::<LabelsOverlayConfig>(r#"docs = ["AGENTS.md"]"#).is_err());
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
