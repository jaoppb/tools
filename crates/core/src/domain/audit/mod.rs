//! The audit of the issue structure: the rules of `AGENTS.md` section 10 over a
//! survey of the repository's issues and the Project's items.
//!
//! Like the rest of `domain`, it imports only `std`: it never sees `gh`, JSON
//! or the network.

pub mod release;
pub mod rules;
pub mod survey;
pub mod text;
pub mod violation;

pub use crate::domain::audit::release::{Major, Minor, Release};
pub use crate::domain::audit::survey::issue::{BlockedBy, Placement, TreeIssue};
pub use crate::domain::audit::survey::{IssueTree, Members, Survey};
pub use crate::domain::audit::text::{Reference, Surface, Text, TextItem, Texts};
pub use crate::domain::audit::violation::{Cycle, Rule, Violation, Violations};
