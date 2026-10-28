//! The real [`RepositoryText`]: the title, body, comments, reviews and review
//! threads of every issue and pull request, read through `gh`.

use crate::adapters::github::cursor_fields;
use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{TextsPage, parse_issue_texts, parse_pull_texts};
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::application::{AuditError, RepositoryText};
use crate::domain::{TextItem, Texts};

/// One kind of item to read: its query, its parser and what to call it in a
/// message.
struct Feed {
    query: String,
    parse: fn(&str) -> Result<TextsPage, GhError>,
    what: &'static str,
}

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

/// Reads the text of one repository through a `gh` runner.
#[derive(Debug, Clone)]
pub struct GhText {
    runner: GhRunner,
    target: Target,
}

impl GhText {
    /// The text of this repository, reached through the installed `gh`.
    #[must_use]
    pub fn new(target: Target) -> Self {
        Self::with_runner(GhRunner::default(), target)
    }

    /// The text of this repository, reached through this runner.
    #[must_use]
    pub const fn with_runner(runner: GhRunner, target: Target) -> Self {
        Self { runner, target }
    }

    fn read(&self) -> Result<Texts, GhError> {
        let issues = Feed {
            query: query::texts_issues(&self.target),
            parse: parse_issue_texts,
            what: "issues",
        };
        let pulls = Feed {
            query: query::texts_pulls(&self.target),
            parse: parse_pull_texts,
            what: "pull requests",
        };
        let mut items = self.feed(&issues)?;
        items.extend(self.feed(&pulls)?);
        Ok(Texts::new(items))
    }

    fn feed(&self, feed: &Feed) -> Result<Vec<TextItem>, GhError> {
        self.gather(feed, GatheredTexts::default())?
            .into_items(feed.what)
    }

    fn gather(&self, feed: &Feed, gathered: GatheredTexts) -> Result<GatheredTexts, GhError> {
        let cursor = gathered.next.as_deref();
        let reply = self.runner.graphql(&feed.query, &cursor_fields(cursor))?;
        let gathered = gathered.with(&(feed.parse)(&reply)?);
        match gathered.next {
            Some(_) => self.gather(feed, gathered),
            None => Ok(gathered),
        }
    }
}

impl RepositoryText for GhText {
    fn texts(&self) -> Result<Texts, AuditError> {
        self.read().map_err(|error| AuditError::Text {
            reason: error.to_string(),
        })
    }
}
