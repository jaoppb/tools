//! The real [`RepositoryText`]: the title, body, comments, reviews and review
//! threads of every issue and pull request, read through `gh`.

use crate::adapters::github::cursor_fields;
use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{
    CutOff, ParsedTextsPage, Tail, TextsPage, parse_issue_page, parse_item_comments,
    parse_pull_page, parse_pull_review_threads, parse_thread_comments,
};
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::application::{AuditError, RepositoryText};
use crate::domain::{IssueNumber, Repo, Surface, TextItem, Texts};

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
        let mut items = Vec::new();
        for repo in self.target.repos() {
            let issues_query = query::texts_issues_for_repo(&self.target, repo);
            let pulls_query = query::texts_pulls_for_repo(&self.target, repo);
            items.extend(self.feed_issues(&issues_query, repo)?);
            items.extend(self.feed_pulls(&pulls_query, repo)?);
        }
        Ok(Texts::new(items))
    }

    fn feed_issues(&self, query: &str, repo: &Repo) -> Result<Vec<TextItem>, GhError> {
        self.gather(query, repo, parse_issue_page, GatheredTexts::default())?
            .into_items("issues")
    }

    fn feed_pulls(&self, query: &str, repo: &Repo) -> Result<Vec<TextItem>, GhError> {
        self.gather(query, repo, parse_pull_page, GatheredTexts::default())?
            .into_items("pull requests")
    }

    fn gather<P>(
        &self,
        query: &str,
        repo: &Repo,
        parse: P,
        gathered: GatheredTexts,
    ) -> Result<GatheredTexts, GhError>
    where
        P: Fn(&str, &Repo) -> Result<ParsedTextsPage, GhError>,
    {
        let cursor = gathered.next.as_deref();
        let reply = self.runner.graphql(query, &cursor_fields(cursor))?;
        let parsed = parse(&reply, repo)?;
        let (page, cut_offs) = parsed.into_parts();
        let mut page_items = page.items().to_vec();
        for cut_off in cut_offs {
            page_items.extend(self.paginate_cut_off(repo, cut_off)?);
        }
        let full_page = TextsPage::new(
            page_items,
            page.next().map(str::to_owned),
            page.read(),
            page.total(),
        );
        let gathered = gathered.with(&full_page);
        match gathered.next {
            Some(_) => self.gather(query, repo, parse, gathered),
            None => Ok(gathered),
        }
    }

    fn paginate_cut_off(&self, repo: &Repo, cut_off: CutOff) -> Result<Vec<TextItem>, GhError> {
        match cut_off {
            CutOff::IssueComments(number, tail) => {
                let query = query::text_issue_comments_for_repo(&self.target, repo, number.value());
                self.paginate_connection(&number, "comments", tail, |cursor| {
                    let reply = self.runner.graphql(&query, &cursor_fields(Some(cursor)))?;
                    parse_item_comments(&reply, &number, Surface::Comment)
                })
            }
            CutOff::PullComments(number, tail) => {
                let query = query::text_pull_comments_for_repo(&self.target, repo, number.value());
                self.paginate_connection(&number, "comments", tail, |cursor| {
                    let reply = self.runner.graphql(&query, &cursor_fields(Some(cursor)))?;
                    parse_item_comments(&reply, &number, Surface::Comment)
                })
            }
            CutOff::PullReviews(number, tail) => {
                let query = query::text_pull_reviews_for_repo(&self.target, repo, number.value());
                self.paginate_connection(&number, "reviews", tail, |cursor| {
                    let reply = self.runner.graphql(&query, &cursor_fields(Some(cursor)))?;
                    parse_item_comments(&reply, &number, Surface::Review)
                })
            }
            CutOff::PullThreads(number, tail) => {
                let query = query::text_pull_threads_for_repo(&self.target, repo, number.value());
                let mut extra = Vec::new();
                let mut current_tail = tail;
                while current_tail.is_cut_off() {
                    let cursor = current_tail.cursor().ok_or_else(|| GhError::Incomplete {
                        what: format!("{number} has more review threads than the page returned"),
                    })?;
                    let reply = self.runner.graphql(&query, &cursor_fields(Some(cursor)))?;
                    let (items, thread_cut_offs, next_tail) =
                        parse_pull_review_threads(&reply, &number)?;
                    extra.extend(items);
                    for nested in thread_cut_offs {
                        extra.extend(self.paginate_cut_off(repo, nested)?);
                    }
                    current_tail = next_tail;
                }
                Ok(extra)
            }
            CutOff::ThreadComments(number, thread_id, tail) => {
                let id = thread_id.ok_or_else(|| GhError::BadReply {
                    reason: format!("{number} review thread missing id in reply"),
                })?;
                self.paginate_connection(&number, "comments in a review thread", tail, |cursor| {
                    let fields = [format!("id={id}"), format!("cursor={cursor}")];
                    let reply = self
                        .runner
                        .graphql(query::text_thread_comments(), &fields)?;
                    parse_thread_comments(&reply, &number)
                })
            }
        }
    }

    fn paginate_connection<F>(
        &self,
        number: &IssueNumber,
        what: &str,
        mut tail: Tail,
        mut fetch_page: F,
    ) -> Result<Vec<TextItem>, GhError>
    where
        F: FnMut(&str) -> Result<(Vec<TextItem>, Tail), GhError>,
    {
        let mut extra = Vec::new();
        while tail.is_cut_off() {
            let cursor = tail.cursor().ok_or_else(|| GhError::Incomplete {
                what: format!("{number} has more {what} than the page returned"),
            })?;
            let (items, next_tail) = fetch_page(cursor)?;
            extra.extend(items);
            tail = next_tail;
        }
        Ok(extra)
    }
}

impl RepositoryText for GhText {
    fn texts(&self) -> Result<Texts, AuditError> {
        self.read().map_err(|error| AuditError::Text {
            reason: error.to_string(),
        })
    }
}
