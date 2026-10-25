//! The Project read and written through the `gh` command line.
//!
//! `gh` is the transport so the tool never reads, stores or prints a token: it
//! uses the login the owner already has.

pub mod error;
pub mod issues;
pub mod parse;
pub mod project;
pub mod query;
pub mod runner;
pub mod target;
pub mod texts;

pub use crate::adapters::github::error::GhError;
pub use crate::adapters::github::issues::{Gathered, GhIssues};
pub use crate::adapters::github::project::GhProject;
pub use crate::adapters::github::runner::GhRunner;
pub use crate::adapters::github::target::Target;
pub use crate::adapters::github::texts::{GatheredTexts, GhText};
