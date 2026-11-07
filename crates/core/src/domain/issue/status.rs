//! The statuses of a workflow as validated values.
//!
//! A [`StatusName`] exists only if the [`Statuses`] it came from declares it, so
//! no code path writes a column the workflow does not know.

use std::fmt;

/// A status declared by a workflow: the name of a Project Status option.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StatusName(String);

impl StatusName {
    /// The name as the Project spells it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StatusName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl PartialEq<str> for StatusName {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for StatusName {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// Why a set of [`Statuses`] cannot be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StatusesError {
    /// No status is declared.
    #[error("no statuses are declared")]
    Empty,
    /// A declared name is blank.
    #[error("a status name is blank")]
    Blank,
    /// The default is not one of the declared names.
    #[error("default status {0:?} is not declared")]
    UndeclaredDefault(String),
}

/// A name that no declared status matches.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a declared status")]
pub struct UnknownStatus(pub String);

/// The declared statuses of a workflow and the one a card without a Status has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statuses {
    names: Vec<StatusName>,
    default: StatusName,
    /// Accepts any non-blank name: for readers that only need membership.
    open: bool,
}

impl Statuses {
    /// The declared `names`, in board order, with `default` among them.
    /// Names are trimmed and repeats are kept once.
    ///
    /// # Errors
    ///
    /// [`StatusesError`] when there are no names, a name is blank or `default`
    /// is not declared.
    pub fn new<S: AsRef<str>>(names: &[S], default: &str) -> Result<Self, StatusesError> {
        let mut declared: Vec<StatusName> = Vec::new();
        for name in names {
            let trimmed = name.as_ref().trim();
            if trimmed.is_empty() {
                return Err(StatusesError::Blank);
            }
            if !declared.iter().any(|seen| seen == trimmed) {
                declared.push(StatusName(trimmed.to_owned()));
            }
        }
        if declared.is_empty() {
            return Err(StatusesError::Empty);
        }
        let default = declared
            .iter()
            .find(|status| *status == default.trim())
            .cloned()
            .ok_or_else(|| StatusesError::UndeclaredDefault(default.to_owned()))?;
        Ok(Self {
            names: declared,
            default,
            open: false,
        })
    }

    /// No declared vocabulary: every non-blank name is accepted and a card
    /// without a Status is "No status". For readers that only need to know
    /// which issues are cards, such as the audit; never for writing.
    // TODO(#59): the audit stops reading Status and this mode goes with it.
    #[must_use]
    pub fn undeclared() -> Self {
        Self {
            names: Vec::new(),
            default: StatusName("No status".to_owned()),
            open: true,
        }
    }

    /// The declared status spelled `raw` (surrounding blanks ignored).
    ///
    /// # Errors
    ///
    /// [`UnknownStatus`] when no declared status has that name.
    pub fn parse(&self, raw: &str) -> Result<StatusName, UnknownStatus> {
        let trimmed = raw.trim();
        if self.open && !trimmed.is_empty() {
            return Ok(StatusName(trimmed.to_owned()));
        }
        self.names
            .iter()
            .find(|status| *status == trimmed)
            .cloned()
            .ok_or_else(|| UnknownStatus(raw.to_owned()))
    }

    /// The status of a card the board has no Status for.
    #[must_use]
    pub const fn default_status(&self) -> &StatusName {
        &self.default
    }

    /// The declared statuses in board order.
    pub fn iter(&self) -> impl Iterator<Item = &StatusName> {
        self.names.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> Statuses {
        Statuses::new(&["Todo", " Doing ", "Done", "Todo"], "Todo").expect("valid")
    }

    #[test]
    fn a_declared_name_parses_and_repeats_collapse() {
        let statuses = board();
        assert_eq!(statuses.iter().count(), 3);
        assert_eq!(statuses.parse(" Doing").expect("declared"), "Doing");
    }

    #[test]
    fn an_undeclared_name_is_refused() {
        assert_eq!(
            board().parse("doing"),
            Err(UnknownStatus("doing".to_owned()))
        );
    }

    #[test]
    fn the_default_must_be_declared() {
        assert_eq!(
            Statuses::new(&["A"], "B"),
            Err(StatusesError::UndeclaredDefault("B".to_owned()))
        );
        assert_eq!(Statuses::new::<&str>(&[], "A"), Err(StatusesError::Empty));
        assert_eq!(Statuses::new(&[" "], "A"), Err(StatusesError::Blank));
    }
}
