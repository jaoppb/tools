//! The `tools.toml` schema of repository settings, labels and milestones.
//!
//! These types own every serde attribute of that part of the file (strict tables, defaults,
//! aliases, spelling of the enums). The domain types in [`crate::domain::repo`] know nothing about
//! TOML; each DTO here maps into its domain counterpart through one `From` impl, and
//! `Config::spec_for` is the only caller.

mod labels;
mod milestones;
mod settings;

pub use labels::{LabelsConfig, LabelsOverlayConfig};
pub use milestones::{MilestonesConfig, MilestonesOverlayConfig};
pub use settings::RepositorySettingsConfig;

use crate::domain::repo::Unmanaged;
use serde::Deserialize;

/// What to do with remote items that `tools.toml` does not declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnmanagedConfig {
    /// Say nothing.
    Ignore,
    /// Name the item in the output; the run still succeeds.
    Warn,
    /// Name the item in the output; the run fails.
    Error,
    /// Delete the item on `sync`.
    Prune,
}

impl From<UnmanagedConfig> for Unmanaged {
    fn from(config: UnmanagedConfig) -> Self {
        match config {
            UnmanagedConfig::Ignore => Self::Ignore,
            UnmanagedConfig::Warn => Self::Warn,
            UnmanagedConfig::Error => Self::Error,
            UnmanagedConfig::Prune => Self::Prune,
        }
    }
}

const fn default_labels() -> UnmanagedConfig {
    UnmanagedConfig::Warn
}

const fn default_milestones() -> UnmanagedConfig {
    UnmanagedConfig::Error
}
