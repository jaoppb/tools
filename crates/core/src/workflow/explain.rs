//! The trace of one issue: every fact, every rule, which one fired.

use std::fmt;

use cel_interpreter::Value;

use crate::domain::IssueNumber;
use crate::workflow::engine::Violation;

/// What happened to one rule for one issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleResult {
    /// `when` was true: the rule fired.
    Fired,
    /// `when` was false.
    NotMatched,
    /// `when` failed to evaluate; the issue is frozen.
    Error(String),
    /// An earlier rule fired or failed.
    NotReached,
}

/// One rule as evaluated for one issue.
#[derive(Debug, Clone)]
pub(crate) struct RuleTrace {
    pub(crate) id: String,
    pub(crate) when: String,
    pub(crate) set: String,
    pub(crate) result: RuleResult,
}

/// The explanation of one issue's decision. `Display` is the `--explain` text.
#[derive(Debug, Clone)]
pub struct Explain {
    pub(crate) number: IssueNumber,
    pub(crate) current: String,
    pub(crate) destination: String,
    pub(crate) facts: Vec<(String, Value)>,
    pub(crate) rules: Vec<RuleTrace>,
    pub(crate) violation: Option<Violation>,
}

impl Explain {
    /// The result of the rule with this id, if the workflow has it.
    #[must_use]
    pub fn result_of(&self, id: &str) -> Option<&RuleResult> {
        self.rules
            .iter()
            .find(|rule| rule.id == id)
            .map(|rule| &rule.result)
    }
}

impl fmt::Display for Explain {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            out,
            "{}: {} -> {}",
            self.number, self.current, self.destination
        )?;
        if let Some(violation) = &self.violation {
            writeln!(out, "violation: {violation}")?;
        }
        writeln!(out, "facts:")?;
        for (name, value) in &self.facts {
            writeln!(out, "  {name} = {}", show(value))?;
        }
        writeln!(out, "rules:")?;
        for rule in &self.rules {
            let mark = match &rule.result {
                RuleResult::Fired => "FIRED      ".to_owned(),
                RuleResult::NotMatched => "no         ".to_owned(),
                RuleResult::Error(_) => "ERROR      ".to_owned(),
                RuleResult::NotReached => "not reached".to_owned(),
            };
            write!(out, "  [{mark}] {}: {} => {}", rule.id, rule.when, rule.set)?;
            if let RuleResult::Error(message) = &rule.result {
                write!(out, " ({message})")?;
            }
            writeln!(out)?;
        }
        Ok(())
    }
}

fn show(value: &Value) -> String {
    match value {
        Value::Bool(flag) => flag.to_string(),
        Value::Int(number) => number.to_string(),
        Value::String(text) => format!("{text:?}"),
        Value::Null => "null".to_owned(),
        Value::List(items) => {
            let shown: Vec<String> = items.iter().map(show).collect();
            format!("[{}]", shown.join(", "))
        }
        other => format!("{other:?}"),
    }
}
