//! The text of issues and pull requests: titles, bodies, comments, and reviews.
//!
//! The models are written on `std` only, as the rest of `domain` is.

use std::fmt;

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

/// A string a person reads on GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text(String);

impl Text {
    /// This text.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        Self(raw.to_owned())
    }

    /// The raw string content of this text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
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
    pub const fn number(&self) -> &IssueNumber {
        &self.number
    }

    /// Where in the item the text sits.
    #[must_use]
    pub const fn surface(&self) -> Surface {
        self.surface
    }

    /// The text content.
    #[must_use]
    pub const fn text(&self) -> &Text {
        &self.text
    }
}

/// Every text read from an issue or pull request.
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
