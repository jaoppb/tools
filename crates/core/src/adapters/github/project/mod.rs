//! The real [`ProjectState`]: the Cosmo Project read and written through `gh`.

use crate::adapters::github::error::GhError;
use crate::adapters::github::parse::{
    StatusField, parse_item_id, parse_items, parse_pulls, parse_status_field,
};
use crate::adapters::github::query;
use crate::adapters::github::runner::GhRunner;
use crate::adapters::github::target::Target;
use crate::application::{ProjectState, SyncError};
use crate::domain::{Card, Cards, Closing, IssueNumber, Snapshot, Status};

/// Reads and writes one Project through a `gh` runner.
#[derive(Debug, Clone)]
pub struct GhProject {
    runner: GhRunner,
    target: Target,
}

impl GhProject {
    /// A Project reached through the installed `gh`.
    #[must_use]
    pub fn new(target: Target) -> Self {
        Self::with_runner(GhRunner::default(), target)
    }

    /// A Project reached through this runner.
    #[must_use]
    pub const fn with_runner(runner: GhRunner, target: Target) -> Self {
        Self { runner, target }
    }

    fn read(&self) -> Result<Snapshot, GhError> {
        self.target.required_project()?;
        let cards = self.cards_from(None, Vec::new())?;
        let closing = self.closing_from(None, Vec::new())?;
        Ok(Snapshot::new(Cards::new(cards), Closing::new(closing)))
    }

    fn cards_from(&self, cursor: Option<&str>, mut cards: Vec<Card>) -> Result<Vec<Card>, GhError> {
        let reply = self
            .runner
            .graphql(&query::items(&self.target), &cursor_fields(cursor))?;
        let page = parse_items(&reply, &self.target)?;
        cards.extend_from_slice(page.cards());
        match page.next() {
            Some(next) => self.cards_from(Some(next), cards),
            None => Ok(cards),
        }
    }

    fn closing_from(
        &self,
        cursor: Option<&str>,
        mut closing: Vec<IssueNumber>,
    ) -> Result<Vec<IssueNumber>, GhError> {
        let reply = self
            .runner
            .graphql(&query::pulls(&self.target), &cursor_fields(cursor))?;
        let page = parse_pulls(&reply)?;
        closing.extend_from_slice(page.closing());
        match page.next() {
            Some(next) => self.closing_from(Some(next), closing),
            None => Ok(closing),
        }
    }

    fn write(&self, number: IssueNumber, status: Status) -> Result<(), GhError> {
        let item = self.item_id(number)?;
        let field = self.status_field()?;
        let option = field
            .option(status.name())
            .ok_or_else(|| GhError::NoStatusOption {
                name: status.name().to_owned(),
            })?;
        self.runner
            .graphql(query::SET_STATUS, &assignment(&field, &item, option))?;
        Ok(())
    }

    fn item_id(&self, number: IssueNumber) -> Result<String, GhError> {
        let project = self.target.required_project()?;
        let request = query::item_of_issue(&self.target, number.value());
        let reply = self.runner.graphql(&request, &[])?;
        parse_item_id(&reply, project)?.ok_or(GhError::NoItem { number })
    }

    fn status_field(&self) -> Result<StatusField, GhError> {
        let reply = self
            .runner
            .graphql(&query::status_field(&self.target), &[])?;
        parse_status_field(&reply)
    }
}

pub(crate) fn cursor_fields(cursor: Option<&str>) -> Vec<String> {
    cursor
        .map(|value| format!("cursor={value}"))
        .into_iter()
        .collect()
}

fn assignment(field: &StatusField, item: &str, option: &str) -> Vec<String> {
    vec![
        format!("project={}", field.project()),
        format!("item={item}"),
        format!("field={}", field.field()),
        format!("option={option}"),
    ]
}

impl ProjectState for GhProject {
    fn snapshot(&self) -> Result<Snapshot, SyncError> {
        self.read().map_err(|error| SyncError::Read {
            reason: error.to_string(),
        })
    }

    fn set_status(&mut self, number: IssueNumber, status: Status) -> Result<(), SyncError> {
        self.write(number, status)
            .map_err(|error| SyncError::Write {
                number,
                reason: error.to_string(),
            })
    }
}
