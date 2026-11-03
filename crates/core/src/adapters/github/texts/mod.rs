//! Accumulated texts from issues and pull requests parsed from pages.

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::TextsPage;
use crate::domain::TextItem;

/// The pages of one kind read so far. It accepts them only when together they
/// are every issue or pull request the repository says it has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GatheredTexts {
    items: Vec<TextItem>,
    read: u64,
    total: u64,
    next: Option<String>,
}

impl GatheredTexts {
    /// These pages with one more. The total is the one of the latest page.
    #[must_use]
    pub fn with(self, page: &TextsPage) -> Self {
        let mut items = self.items;
        items.extend_from_slice(page.items());
        Self {
            items,
            read: self.read.saturating_add(page.read()),
            total: page.total(),
            next: page.next().map(str::to_owned),
        }
    }

    /// The texts gathered. `what` names the kind read, such as `issues`.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::Incomplete`] when fewer issues or pull requests were
    /// read than the repository says it has.
    pub fn into_items(self, what: &str) -> Result<Vec<TextItem>, GhError> {
        (self.read >= self.total)
            .then_some(self.items)
            .ok_or_else(|| GhError::Incomplete {
                what: format!("read {} of {} {what}", self.read, self.total),
            })
    }
}
