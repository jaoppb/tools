//! The config-driven workflow engine.
//!
//! Rules are written in CEL over a versioned fact schema, loaded from a
//! `[workflow]` block and evaluated first-match-wins, parents before children.
//! Nothing here knows the words "type", "area" or "epic": the vocabulary lives
//! in `[workflow.params]`, the facts and the rules of a preset.

mod config;
mod contain;
mod engine;
mod error;
mod explain;
mod expr;
mod params;
mod schema;
mod subject;

pub use crate::workflow::config::{RuleId, Workflow, WorkflowConfig};
pub use crate::workflow::engine::{Decision, Engine, Evaluation, Outcome, Violation};
pub use crate::workflow::error::LoadError;
pub use crate::workflow::explain::{Explain, RuleResult};
pub use crate::workflow::subject::{FieldValue, ParentResolver, ResolveError, Subject};

pub(crate) use crate::workflow::params::wrappable as wrappable_params;
