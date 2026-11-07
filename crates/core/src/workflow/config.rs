//! The `[workflow]` block: parsing, preset merge and load-time validation.

use std::collections::{BTreeMap, BTreeSet};

use cel_interpreter::Value;
use serde::Deserialize;

use crate::workflow::error::LoadError;
use crate::workflow::expr::Expr;
use crate::workflow::schema::{CURRENT, ROOTS, VERSION};

const COSMO: &str = include_str!("presets/cosmo.toml");

#[derive(Debug, Default, Deserialize)]
struct File {
    #[serde(default)]
    workflow: RawWorkflow,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkflow {
    schema: Option<i64>,
    extends: Option<String>,
    #[serde(default)]
    statuses: Vec<String>,
    default: Option<String>,
    #[serde(default)]
    params: BTreeMap<String, toml::Value>,
    #[serde(default)]
    facts: BTreeMap<String, String>,
    #[serde(default)]
    status: Vec<RawRule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    id: String,
    when: String,
    set: String,
}

/// One compiled status rule.
#[derive(Debug)]
pub(crate) struct Rule {
    pub(crate) id: String,
    pub(crate) when: Expr,
    pub(crate) set: String,
}

/// A validated workflow: every expression compiled, every status declared.
#[derive(Debug)]
pub struct Workflow {
    pub(crate) default: String,
    pub(crate) params: Value,
    /// Derived facts in dependency order.
    pub(crate) facts: Vec<(String, Expr)>,
    pub(crate) rules: Vec<Rule>,
}

impl Workflow {
    /// Loads the `[workflow]` block of `toml`, merging `extends` first.
    ///
    /// Local rules run first and builtin rules are appended; a local rule
    /// shadows a builtin one with the same `id`. Facts and params merge
    /// local-over-preset.
    ///
    /// # Errors
    ///
    /// A [`LoadError`] naming the first problem found.
    pub fn from_toml(toml: &str) -> Result<Self, LoadError> {
        let local = parse(toml)?;
        let merged = match local.extends.as_deref() {
            None => local,
            Some("builtin:cosmo") => merge(local, parse(COSMO)?),
            Some(other) => return Err(LoadError::Preset(other.to_owned())),
        };
        build(merged)
    }

    /// The ids of the status rules, in evaluation order.
    #[must_use]
    pub fn rule_ids(&self) -> Vec<&str> {
        self.rules.iter().map(|rule| rule.id.as_str()).collect()
    }
}

fn parse(toml: &str) -> Result<RawWorkflow, LoadError> {
    let file: File = ::toml::from_str(toml)?;
    match file.workflow.schema {
        None | Some(VERSION) => Ok(file.workflow),
        Some(other) => Err(LoadError::Schema(other)),
    }
}

fn merge(local: RawWorkflow, preset: RawWorkflow) -> RawWorkflow {
    let shadowed: BTreeSet<&str> = local.status.iter().map(|rule| rule.id.as_str()).collect();
    let appended: Vec<RawRule> = preset
        .status
        .into_iter()
        .filter(|rule| !shadowed.contains(rule.id.as_str()))
        .collect();
    let mut rules = local.status;
    rules.extend(appended);
    RawWorkflow {
        schema: local.schema.or(preset.schema),
        extends: None,
        statuses: if local.statuses.is_empty() {
            preset.statuses
        } else {
            local.statuses
        },
        default: local.default.or(preset.default),
        params: preset.params.into_iter().chain(local.params).collect(),
        facts: preset.facts.into_iter().chain(local.facts).collect(),
        status: rules,
    }
}

fn build(raw: RawWorkflow) -> Result<Workflow, LoadError> {
    let statuses: BTreeSet<&str> = raw.statuses.iter().map(String::as_str).collect();
    let default = raw
        .default
        .unwrap_or_else(|| raw.statuses.first().cloned().unwrap_or_default());
    if !statuses.contains(default.as_str()) {
        return Err(LoadError::UndeclaredDefault(default));
    }
    let mut seen = BTreeSet::new();
    for rule in &raw.status {
        if !seen.insert(rule.id.as_str()) {
            return Err(LoadError::DuplicateRule(rule.id.clone()));
        }
        if rule.set != CURRENT && !statuses.contains(rule.set.as_str()) {
            return Err(LoadError::UndeclaredStatus {
                rule: rule.id.clone(),
                status: rule.set.clone(),
            });
        }
    }
    if let Some(name) = raw.facts.keys().find(|name| ROOTS.contains(&name.as_str())) {
        return Err(LoadError::ReservedFact(name.clone()));
    }
    let known: BTreeSet<String> = ROOTS
        .iter()
        .map(|root| (*root).to_owned())
        .chain(raw.facts.keys().cloned())
        .collect();
    let facts = order_facts(&raw.facts, &known)?;
    let rules = raw
        .status
        .iter()
        .map(|rule| {
            let origin = format!("rule {:?}", rule.id);
            Ok(Rule {
                id: rule.id.clone(),
                when: Expr::compile(&origin, &rule.when, &known)?,
                set: rule.set.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    Ok(Workflow {
        default,
        params: params_value(&raw.params)?,
        facts,
        rules,
    })
}

/// Compiles the facts and sorts them so each comes after the facts it reads.
fn order_facts(
    sources: &BTreeMap<String, String>,
    known: &BTreeSet<String>,
) -> Result<Vec<(String, Expr)>, LoadError> {
    let mut compiled = BTreeMap::new();
    for (name, source) in sources {
        compiled.insert(
            name.clone(),
            Expr::compile(&format!("fact {name:?}"), source, known)?,
        );
    }
    let mut ordered: Vec<String> = Vec::new();
    let mut visiting = BTreeSet::new();
    for name in compiled.keys() {
        visit(name, &compiled, &mut visiting, &mut ordered)?;
    }
    Ok(ordered
        .into_iter()
        .filter_map(|name| compiled.remove(&name).map(|expr| (name, expr)))
        .collect())
}

fn visit(
    name: &str,
    compiled: &BTreeMap<String, Expr>,
    visiting: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> Result<(), LoadError> {
    if ordered.iter().any(|done| done == name) {
        return Ok(());
    }
    if !visiting.insert(name.to_owned()) {
        return Err(LoadError::FactCycle(name.to_owned()));
    }
    if let Some(expr) = compiled.get(name) {
        for read in expr
            .reads()
            .iter()
            .filter(|read| compiled.contains_key(*read))
        {
            visit(read, compiled, visiting, ordered)?;
        }
    }
    visiting.remove(name);
    ordered.push(name.to_owned());
    Ok(())
}

fn params_value(params: &BTreeMap<String, toml::Value>) -> Result<Value, LoadError> {
    let entries = params
        .iter()
        .map(|(name, value)| {
            scalar_or_list(value)
                .map(|value| (name.clone(), value))
                .ok_or_else(|| LoadError::Param(name.clone()))
        })
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    Ok(entries.into())
}

fn scalar_or_list(value: &toml::Value) -> Option<Value> {
    match value {
        toml::Value::Array(items) => items
            .iter()
            .map(scalar)
            .collect::<Option<Vec<_>>>()
            .map(Value::from),
        other => scalar(other),
    }
}

fn scalar(value: &toml::Value) -> Option<Value> {
    match value {
        toml::Value::String(text) => Some(Value::from(text.as_str())),
        toml::Value::Integer(number) => Some(Value::Int(*number)),
        toml::Value::Float(number) => Some(Value::Float(*number)),
        toml::Value::Boolean(flag) => Some(Value::Bool(*flag)),
        _ => None,
    }
}
