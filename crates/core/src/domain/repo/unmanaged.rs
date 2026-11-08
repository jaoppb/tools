//! What to do with items that exist on GitHub but are not declared in `tools.toml`.

use serde::{Deserialize, Serialize};

/// Per-kind policy for remote items that `tools.toml` does not declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unmanaged {
    /// Say nothing.
    Ignore,
    /// Name the item in the output; the run still succeeds.
    Warn,
    /// Name the item in the output; the run fails.
    Error,
    /// Delete the item on `sync`; `check` reports it as drift.
    Prune,
}

impl Unmanaged {
    /// Default for labels: they are many and often added by hand.
    #[must_use]
    pub const fn labels() -> Self {
        Self::Warn
    }

    /// Default for milestones: they are few and always intentional.
    #[must_use]
    pub const fn milestones() -> Self {
        Self::Error
    }

    /// How an unmanaged item is reported under this policy, `None` when it is not reported
    /// (`ignore`) or acted on instead (`prune`).
    #[must_use]
    pub const fn severity(self) -> Option<Severity> {
        match self {
            Self::Warn => Some(Severity::Warn),
            Self::Error => Some(Severity::Error),
            Self::Ignore | Self::Prune => None,
        }
    }
}

/// How serious a reported unmanaged item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Reported, run succeeds.
    Warn,
    /// Reported, run fails.
    Error,
}

pub(crate) const fn default_labels() -> Unmanaged {
    Unmanaged::labels()
}

pub(crate) const fn default_milestones() -> Unmanaged {
    Unmanaged::milestones()
}

impl std::fmt::Display for Severity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warn => write!(formatter, "warning"),
            Self::Error => write!(formatter, "error"),
        }
    }
}
