//! What a run prints and how it exits.

/// What to print and how to exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    /// The report, printed to standard error because printing macros are forbidden.
    pub text: String,
    /// The process exit code.
    pub code: u8,
}

impl Exit {
    /// A finished run with the given exit code.
    #[must_use]
    pub fn new(text: impl Into<String>, code: u8) -> Self {
        Self {
            text: text.into(),
            code,
        }
    }

    /// A run that could not complete: exit code 2.
    #[must_use]
    pub fn failed(reason: impl ToString) -> Self {
        Self {
            text: reason.to_string(),
            code: 2,
        }
    }
}
