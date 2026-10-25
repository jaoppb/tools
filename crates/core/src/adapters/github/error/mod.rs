//! How talking to `gh` can fail.

use crate::domain::IssueNumber;

/// A failure of the `gh` adapter. The port implementation maps it to the
/// application's error, so no `gh` detail leaks upward.
#[derive(Debug, thiserror::Error)]
pub enum GhError {
    /// The `gh` program is not installed.
    #[error("gh is not installed or is not on the PATH")]
    Missing,
    /// A Project number is needed for this operation but is not configured.
    #[error("project number is required but not configured")]
    MissingProject,
    /// `gh` ran and failed.
    #[error("gh failed: {reason}")]
    Failed {
        /// What `gh` said, or why it could not run.
        reason: String,
    },
    /// `gh` answered something this tool cannot read.
    #[error("cannot read the answer of gh: {reason}")]
    BadReply {
        /// What was wrong with the answer.
        reason: String,
    },
    /// `gh` answered only part of the data, which the audit never accepts.
    #[error("gh answered only part of the data: {what}")]
    Incomplete {
        /// What was cut off.
        what: String,
    },
    /// The Project has a Status this tool does not know.
    #[error("the Project has a Status named {name} that this tool does not know")]
    UnknownStatus {
        /// The unknown Status name.
        name: String,
    },
    /// The Project's Status field lacks an option the tool must set.
    #[error("the Project has no Status option named {name}")]
    NoStatusOption {
        /// The missing option name.
        name: String,
    },
    /// The issue is not an item of the Project.
    #[error("issue {number} is not an item of the Project")]
    NoItem {
        /// The issue that has no item.
        number: IssueNumber,
    },
}
