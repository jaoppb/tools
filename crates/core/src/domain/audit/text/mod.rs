//! The text of issues and pull requests that the audit reads for references to
//! ADRs by number and to `spdd/` files.
//!
//! The scans are written on `std` only, as the rest of `domain` is.

mod adr_number;
mod spdd_path;

use std::fmt;

use crate::domain::audit::violation::Rule;
use crate::domain::issue::IssueNumber;

/// Where in an issue or a pull request a piece of text sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Surface {
    /// The title.
    Title,
    /// The body.
    Body,
    /// A comment on the conversation.
    Comment,
    /// The body of a pull request review.
    Review,
    /// A comment in a review thread.
    ReviewComment,
}

impl Surface {
    /// The words the report uses for this surface.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Body => "body",
            Self::Comment => "comment",
            Self::Review => "review",
            Self::ReviewComment => "review comment",
        }
    }
}

impl fmt::Display for Surface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// What a text names that the repository must not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reference {
    /// An ADR named by its old running number.
    OldAdrNumber,
    /// A numbered file of `spdd/`, which is never committed.
    DeletedSpddPath,
}

impl Reference {
    /// The rule that a reference of this kind in this surface breaks.
    #[must_use]
    pub const fn rule(self, surface: Surface) -> Rule {
        match self {
            Self::OldAdrNumber => Rule::OldAdrNumber(surface),
            Self::DeletedSpddPath => Rule::DeletedSpddPath(surface),
        }
    }
}

/// A string a person reads on GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text(String);

impl Text {
    /// This text.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        Self(raw.to_owned())
    }

    /// What this text names that no longer exists, once per kind, in the order
    /// of [`Reference`].
    #[must_use]
    pub fn references(&self) -> Vec<Reference> {
        let found = [
            adr_number::names_old_number(&self.0).then_some(Reference::OldAdrNumber),
            spdd_path::names_file(&self.0).then_some(Reference::DeletedSpddPath),
        ];
        found.into_iter().flatten().collect()
    }
}

/// A piece of text on an issue or a pull request: GitHub numbers both from one
/// sequence, so one number names either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextItem {
    number: IssueNumber,
    surface: Surface,
    text: Text,
}

impl TextItem {
    /// This text, in this surface of the issue or pull request.
    #[must_use]
    pub const fn new(number: IssueNumber, surface: Surface, text: Text) -> Self {
        Self {
            number,
            surface,
            text,
        }
    }

    /// The issue or pull request the text is on.
    #[must_use]
    pub const fn number(&self) -> IssueNumber {
        self.number
    }

    /// The rules this text breaks, one per kind of reference and sorted, so a
    /// surface with three numbers gives one finding.
    #[must_use]
    pub fn broken_rules(&self) -> Vec<Rule> {
        self.text
            .references()
            .into_iter()
            .map(|reference| reference.rule(self.surface))
            .collect()
    }
}

/// Every text the audit reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Texts(Vec<TextItem>);

impl Texts {
    /// These texts.
    #[must_use]
    pub const fn new(items: Vec<TextItem>) -> Self {
        Self(items)
    }

    /// The texts in the order they were read.
    pub fn iter(&self) -> impl Iterator<Item = &TextItem> {
        self.0.iter()
    }
}
