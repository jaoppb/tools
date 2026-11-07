//! The `[workflow]` block: parsing, preset merge and load-time validation.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use cel_interpreter::Value;
use serde::Deserialize;
use toml::Spanned;

use crate::workflow::error::LoadError;
use crate::workflow::expr::Expr;
use crate::workflow::schema::{CURRENT, ROOTS, VERSION};

const COSMO: &str = include_str!("presets/cosmo.toml");
const TOOLS: &str = include_str!("presets/tools.toml");

fn preset(name: &str) -> Option<&'static str> {
    match name {
        "builtin:cosmo" => Some(COSMO),
        "builtin:tools" => Some(TOOLS),
        _ => None,
    }
}

#[derive(Debug, Default, Deserialize)]
struct File {
    #[serde(default)]
    workflow: WorkflowConfig,
}

/// The `[workflow]` block as written in `tools.toml`: typed and located, not
/// yet compiled. [`WorkflowConfig::compile`] validates it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConfig {
    schema: Option<Spanned<i64>>,
    extends: Option<Spanned<String>>,
    #[serde(default)]
    statuses: Vec<String>,
    default: Option<Spanned<String>>,
    #[serde(default)]
    params: BTreeMap<String, Spanned<toml::Value>>,
    #[serde(default)]
    facts: BTreeMap<String, Spanned<String>>,
    #[serde(default)]
    status: Vec<RawRule>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    id: Spanned<String>,
    when: Spanned<String>,
    set: Spanned<String>,
}

/// A value with the byte range it was read from; `None` for a builtin preset.
#[derive(Debug, Clone)]
struct At<T> {
    value: T,
    span: Option<Range<usize>>,
}

impl<T: Clone> At<T> {
    fn of(spanned: &Spanned<T>, located: bool) -> Self {
        Self {
            value: spanned.get_ref().clone(),
            span: located.then(|| spanned.span()),
        }
    }
}

/// A block after the preset is resolved: rules, facts and params merged.
struct Block {
    extends: Option<At<String>>,
    schema: Option<At<i64>>,
    statuses: Vec<String>,
    default: Option<At<String>>,
    params: BTreeMap<String, At<toml::Value>>,
    facts: BTreeMap<String, At<String>>,
    rules: Vec<RuleDef>,
}

struct RuleDef {
    id: At<String>,
    when: At<String>,
    set: At<String>,
}

impl Block {
    fn new(raw: &WorkflowConfig, located: bool) -> Self {
        Self {
            extends: raw.extends.as_ref().map(|at| At::of(at, located)),
            schema: raw.schema.as_ref().map(|at| At::of(at, located)),
            statuses: raw.statuses.clone(),
            default: raw.default.as_ref().map(|at| At::of(at, located)),
            params: raw
                .params
                .iter()
                .map(|(name, at)| (name.clone(), At::of(at, located)))
                .collect(),
            facts: raw
                .facts
                .iter()
                .map(|(name, at)| (name.clone(), At::of(at, located)))
                .collect(),
            rules: raw
                .status
                .iter()
                .map(|rule| RuleDef {
                    id: At::of(&rule.id, located),
                    when: At::of(&rule.when, located),
                    set: At::of(&rule.set, located),
                })
                .collect(),
        }
    }

    /// Local rules first, builtin rules appended unless shadowed by `id`.
    /// Facts and params merge local-over-preset.
    fn merge(self, preset: Self) -> Self {
        let shadowed: BTreeSet<String> = self.rules.iter().map(|r| r.id.value.clone()).collect();
        let mut rules = self.rules;
        rules.extend(
            preset
                .rules
                .into_iter()
                .filter(|rule| !shadowed.contains(&rule.id.value)),
        );
        Self {
            extends: None,
            schema: self.schema.or(preset.schema),
            statuses: if self.statuses.is_empty() {
                preset.statuses
            } else {
                self.statuses
            },
            default: self.default.or(preset.default),
            params: preset.params.into_iter().chain(self.params).collect(),
            facts: preset.facts.into_iter().chain(self.facts).collect(),
            rules,
        }
    }
}

/// Turns byte ranges of the source into line and column.
#[derive(Clone, Copy)]
struct Locator<'a>(Option<&'a str>);

impl Locator<'_> {
    fn locate(self, span: Option<&Range<usize>>, error: LoadError) -> LoadError {
        let (Some(source), Some(span)) = (self.0, span) else {
            return error;
        };
        let before = &source[..span.start.min(source.len())];
        let line = before.matches('\n').count() + 1;
        let column = before
            .rsplit('\n')
            .next()
            .map_or(0, |row| row.chars().count())
            + 1;
        LoadError::At {
            line,
            column,
            error: Box::new(error),
        }
    }
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

impl WorkflowConfig {
    /// Validates the block and compiles it, merging `extends` first.
    ///
    /// `source` is the TOML text the block was read from; with it, errors carry
    /// the line and column of the offending value.
    ///
    /// # Errors
    ///
    /// A [`LoadError`] naming the first problem found.
    pub fn compile(&self, source: Option<&str>) -> Result<Workflow, LoadError> {
        let at = Locator(source);
        let local = Block::new(self, true);
        check_schema(&local, at)?;
        let block = match &local.extends {
            None => local,
            Some(extends) => {
                let text = preset(&extends.value).ok_or_else(|| {
                    at.locate(
                        extends.span.as_ref(),
                        LoadError::Preset(extends.value.clone()),
                    )
                })?;
                let file: File = toml::from_str(text)?;
                let base = Block::new(&file.workflow, false);
                check_schema(&base, Locator(None))?;
                local.merge(base)
            }
        };
        build(block, at)
    }
}

impl Workflow {
    /// Loads the `[workflow]` block of a stand-alone `toml` string. Public so
    /// a block can be checked without a whole `tools.toml` around it.
    ///
    /// # Errors
    ///
    /// A [`LoadError`] naming the first problem found.
    pub fn from_toml(toml: &str) -> Result<Self, LoadError> {
        let file: File = ::toml::from_str(toml)?;
        file.workflow.compile(Some(toml))
    }

    /// The ids of the status rules, in evaluation order.
    #[must_use]
    pub fn rule_ids(&self) -> Vec<&str> {
        self.rules.iter().map(|rule| rule.id.as_str()).collect()
    }
}

fn check_schema(block: &Block, at: Locator<'_>) -> Result<(), LoadError> {
    match &block.schema {
        Some(schema) if schema.value != VERSION => {
            Err(at.locate(schema.span.as_ref(), LoadError::Schema(schema.value)))
        }
        _ => Ok(()),
    }
}

fn build(block: Block, at: Locator<'_>) -> Result<Workflow, LoadError> {
    if block.statuses.is_empty() {
        return Err(LoadError::NoStatuses);
    }
    let statuses: BTreeSet<&str> = block.statuses.iter().map(String::as_str).collect();
    let default = match &block.default {
        Some(default) => default.clone(),
        None => At {
            value: block.statuses.first().cloned().unwrap_or_default(),
            span: None,
        },
    };
    if !statuses.contains(default.value.as_str()) {
        return Err(at.locate(
            default.span.as_ref(),
            LoadError::UndeclaredDefault(default.value),
        ));
    }
    let mut seen = BTreeSet::new();
    for rule in &block.rules {
        if !seen.insert(rule.id.value.as_str()) {
            return Err(at.locate(
                rule.id.span.as_ref(),
                LoadError::DuplicateRule(rule.id.value.clone()),
            ));
        }
        if rule.set.value != CURRENT && !statuses.contains(rule.set.value.as_str()) {
            return Err(at.locate(
                rule.set.span.as_ref(),
                LoadError::UndeclaredStatus {
                    rule: rule.id.value.clone(),
                    status: rule.set.value.clone(),
                },
            ));
        }
    }
    if let Some((name, fact)) = block
        .facts
        .iter()
        .find(|(name, _)| ROOTS.contains(&name.as_str()))
    {
        return Err(at.locate(fact.span.as_ref(), LoadError::ReservedFact(name.clone())));
    }
    let known: BTreeSet<String> = ROOTS
        .iter()
        .map(|root| (*root).to_owned())
        .chain(block.facts.keys().cloned())
        .collect();
    let facts = order_facts(&block.facts, &known, at)?;
    let rules = block
        .rules
        .iter()
        .map(|rule| {
            let origin = format!("rule {:?}", rule.id.value);
            let when = Expr::compile(&origin, &rule.when.value, &known)
                .map_err(|error| at.locate(rule.when.span.as_ref(), error))?;
            Ok(Rule {
                id: rule.id.value.clone(),
                when,
                set: rule.set.value.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    Ok(Workflow {
        default: default.value,
        params: params_value(&block.params, at)?,
        facts,
        rules,
    })
}

/// Compiles the facts and sorts them so each comes after the facts it reads.
fn order_facts(
    sources: &BTreeMap<String, At<String>>,
    known: &BTreeSet<String>,
    at: Locator<'_>,
) -> Result<Vec<(String, Expr)>, LoadError> {
    let mut compiled = BTreeMap::new();
    for (name, source) in sources {
        let expr = Expr::compile(&format!("fact {name:?}"), &source.value, known)
            .map_err(|error| at.locate(source.span.as_ref(), error))?;
        compiled.insert(name.clone(), expr);
    }
    let mut ordered: Vec<String> = Vec::new();
    let mut done = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    for name in compiled.keys() {
        visit(name, &compiled, &mut visiting, &mut done, &mut ordered).map_err(
            |error| match error {
                LoadError::FactCycle(fact) => {
                    let span = sources.get(&fact).and_then(|source| source.span.clone());
                    at.locate(span.as_ref(), LoadError::FactCycle(fact))
                }
                other => other,
            },
        )?;
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
    done: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> Result<(), LoadError> {
    if done.contains(name) {
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
            visit(read, compiled, visiting, done, ordered)?;
        }
    }
    visiting.remove(name);
    done.insert(name.to_owned());
    ordered.push(name.to_owned());
    Ok(())
}

fn params_value(
    params: &BTreeMap<String, At<toml::Value>>,
    at: Locator<'_>,
) -> Result<Value, LoadError> {
    let entries = params
        .iter()
        .map(|(name, param)| {
            scalar_or_list(&param.value)
                .map(|value| (name.clone(), value))
                .ok_or_else(|| at.locate(param.span.as_ref(), LoadError::Param(name.clone())))
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
