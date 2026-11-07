//! The config-driven workflow engine.
//!
//! Rules are written in CEL over a versioned fact schema, loaded from a
//! `[workflow]` block and evaluated first-match-wins, parents before children.
//! Nothing here knows the words "type", "area" or "epic": the vocabulary lives
//! in `[workflow.params]`, the facts and the rules of a preset.

mod config;
mod engine;
mod error;
mod explain;
mod expr;
mod schema;
mod subject;

pub use crate::workflow::config::{Workflow, WorkflowConfig};
pub use crate::workflow::engine::{Decision, Engine, Evaluation, Violation};
pub use crate::workflow::error::LoadError;
pub use crate::workflow::explain::{Explain, RuleResult};
pub use crate::workflow::subject::{FieldValue, ParentResolver, ResolveError, Subject};
