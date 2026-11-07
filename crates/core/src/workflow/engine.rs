//! Dependency-ordered evaluation: parents first, cycles frozen, one pass.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use cel_interpreter::{Context, Value};

use crate::domain::{IssueNumber, Snapshot};
use crate::workflow::config::{Rule, Workflow};
use crate::workflow::explain::{Explain, RuleResult, RuleTrace};
use crate::workflow::schema::CURRENT;
use crate::workflow::subject::{FieldValue, ParentResolver, ResolveError, Subject};

/// Why an issue was frozen: no status change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// The issue is in a parent cycle.
    Cycle,
    /// A parent could not be fetched.
    ParentUnreadable {
        /// The parent that could not be read.
        parent: IssueNumber,
        /// Why.
        error: ResolveError,
    },
    /// A parent is frozen, so its derived facts do not exist: the child
    /// cannot be evaluated either.
    ParentFrozen {
        /// The frozen parent.
        parent: IssueNumber,
    },
    /// A fact or a `when` failed to evaluate.
    Eval {
        /// `fact "x"` or `rule "x"`.
        origin: String,
        /// The interpreter's message.
        message: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle => out.write_str("the issue is in a parent cycle"),
            Self::ParentUnreadable { parent, error } => {
                write!(out, "parent {parent} cannot be read: {error}")
            }
            Self::ParentFrozen { parent } => write!(out, "parent {parent} is frozen"),
            Self::Eval { origin, message } => write!(out, "{origin} failed: {message}"),
        }
    }
}

/// The decision for one Project card: the trace says everything.
#[derive(Debug, Clone)]
pub struct Decision {
    explain: Explain,
}

impl Decision {
    /// The issue.
    #[must_use]
    pub const fn number(&self) -> &IssueNumber {
        &self.explain.number
    }

    /// The status the card should have; its current one when frozen.
    #[must_use]
    pub fn destination(&self) -> &str {
        &self.explain.destination
    }

    /// The id of the rule that fired, if one did.
    #[must_use]
    pub fn fired(&self) -> Option<&str> {
        self.explain
            .rules
            .iter()
            .find(|rule| rule.result == RuleResult::Fired)
            .map(|rule| rule.id.as_str())
    }

    /// Why the card is frozen, if it is.
    #[must_use]
    pub const fn violation(&self) -> Option<&Violation> {
        self.explain.violation.as_ref()
    }

    /// The full trace.
    #[must_use]
    pub const fn explain(&self) -> &Explain {
        &self.explain
    }
}

/// The decisions of a run, in Project order.
#[derive(Debug, Clone, Default)]
pub struct Evaluation(Vec<Decision>);

impl Evaluation {
    /// The decisions in Project order.
    pub fn iter(&self) -> impl Iterator<Item = &Decision> {
        self.0.iter()
    }

    /// The decision for `number`, if it is a Project card.
    #[must_use]
    pub fn decision(&self, number: &IssueNumber) -> Option<&Decision> {
        self.0.iter().find(|decision| decision.number() == number)
    }
}

/// Evaluates a [`Workflow`] over a snapshot.
pub struct Engine<'a> {
    workflow: &'a Workflow,
    resolver: &'a dyn ParentResolver,
}

impl<'a> Engine<'a> {
    /// An engine for `workflow` that fetches missing parents with `resolver`.
    #[must_use]
    pub const fn new(workflow: &'a Workflow, resolver: &'a dyn ParentResolver) -> Self {
        Self { workflow, resolver }
    }

    /// The decision for every card of `snapshot`.
    #[must_use]
    pub fn evaluate(&self, snapshot: &Snapshot) -> Evaluation {
        let mut run = Run::new(self.workflow, self.resolver);
        let cards: Vec<Subject> = snapshot
            .cards()
            .map(|card| Subject::from_card(card, snapshot))
            .collect();
        for subject in &cards {
            run.subjects
                .insert(subject.number.clone(), Ok(subject.clone()));
        }
        for subject in &cards {
            run.walk_chain(&subject.number);
        }
        Evaluation(
            cards
                .iter()
                .map(|subject| run.node(subject).decision.clone())
                .collect(),
        )
    }
}

/// One issue once evaluated.
struct Node {
    /// The `issue` value with derived facts and `next`, as a child sees it.
    value: Value,
    decision: Decision,
}

impl Node {
    fn frozen(&self) -> bool {
        self.decision.violation().is_some()
    }
}

/// What was evaluated before an issue froze.
#[derive(Default)]
struct Partial {
    facts: Vec<(String, Value)>,
    rules: Vec<RuleTrace>,
}

struct Run<'a> {
    workflow: &'a Workflow,
    resolver: &'a dyn ParentResolver,
    subjects: HashMap<IssueNumber, Result<Subject, ResolveError>>,
    cyclic: HashSet<IssueNumber>,
    walked: HashSet<IssueNumber>,
    nodes: HashMap<IssueNumber, Rc<Node>>,
    /// The functions, registered once; each issue gets a child scope.
    root: Context<'static>,
}

impl<'a> Run<'a> {
    fn new(workflow: &'a Workflow, resolver: &'a dyn ParentResolver) -> Self {
        Self {
            workflow,
            resolver,
            subjects: HashMap::new(),
            cyclic: HashSet::new(),
            walked: HashSet::new(),
            nodes: HashMap::new(),
            root: root_context(),
        }
    }

    /// Loads the parent chain of `start`, fetching what the Project lacks, and
    /// records the issues that sit on a parent cycle.
    fn walk_chain(&mut self, start: &IssueNumber) {
        let mut path: Vec<IssueNumber> = Vec::new();
        let mut current = start.clone();
        loop {
            if let Some(at) = path.iter().position(|seen| *seen == current) {
                self.cyclic.extend(path[at..].iter().cloned());
                break;
            }
            if self.walked.contains(&current) {
                break;
            }
            let subject = self
                .subjects
                .entry(current.clone())
                .or_insert_with(|| self.resolver.fetch(&current));
            let parent = subject
                .as_ref()
                .ok()
                .and_then(|subject| subject.parent.clone());
            path.push(current);
            match parent {
                Some(parent) => current = parent,
                None => break,
            }
        }
        self.walked.extend(path);
    }

    fn node(&mut self, subject: &Subject) -> Rc<Node> {
        if let Some(node) = self.nodes.get(&subject.number) {
            return Rc::clone(node);
        }
        let node = Rc::new(self.compute(subject));
        self.nodes.insert(subject.number.clone(), Rc::clone(&node));
        node
    }

    fn compute(&mut self, subject: &Subject) -> Node {
        let current = subject
            .status
            .clone()
            .unwrap_or_else(|| self.workflow.default.clone());
        if self.cyclic.contains(&subject.number) {
            return self.frozen(subject, &current, Violation::Cycle, Partial::default());
        }
        let parent = match self.parent_value(subject) {
            Ok(parent) => parent,
            Err(violation) => {
                return self.frozen(subject, &current, violation, Partial::default());
            }
        };
        self.evaluate(subject, &current, parent)
    }

    /// The parent as a child sees it: `null` without a parent, otherwise the
    /// parent node's value; a parent that cannot be read or is frozen freezes
    /// the child.
    fn parent_value(&mut self, subject: &Subject) -> Result<Value, Violation> {
        let Some(parent) = &subject.parent else {
            return Ok(Value::Null);
        };
        let loaded = match self.subjects.get(parent) {
            Some(Ok(loaded)) => loaded.clone(),
            Some(Err(error)) => return Err(unreadable(parent, error.clone())),
            None => {
                return Err(unreadable(
                    parent,
                    ResolveError::Failed("not loaded".to_owned()),
                ));
            }
        };
        let node = self.node(&loaded);
        if node.frozen() {
            return Err(Violation::ParentFrozen {
                parent: parent.clone(),
            });
        }
        Ok(node.value.clone())
    }

    fn evaluate(&self, subject: &Subject, current: &str, parent: Value) -> Node {
        let mut map = issue_map(subject, current);
        map.insert("parent".to_owned(), parent);
        let mut context = self.root.new_inner_scope();
        context.add_variable_from_value("issue", map.clone());
        context.add_variable_from_value("params", self.workflow.params.clone());
        context.add_variable_from_value("repo", subject.number.repo().name());
        let (facts, failure) = self.eval_facts(&mut context);
        if let Some(violation) = failure {
            let partial = Partial {
                facts,
                rules: Vec::new(),
            };
            return self.frozen(subject, current, violation, partial);
        }
        let (rules, outcome) = self.eval_rules(&context);
        let set = match outcome {
            Ok(rule) => rule.map(|rule| rule.set.as_str()),
            Err(violation) => {
                return self.frozen(subject, current, violation, Partial { facts, rules });
            }
        };
        let next = match set {
            Some(set) if set != CURRENT => set.to_owned(),
            _ => current.to_owned(),
        };
        for (name, value) in &facts {
            map.insert(name.clone(), value.clone());
        }
        map.insert("next".to_owned(), Value::from(next.as_str()));
        Node {
            value: map.into(),
            decision: decision(subject, current, &next, Partial { facts, rules }, None),
        }
    }

    /// Evaluates the derived facts in dependency order, adding each to
    /// `context`. Returns the facts done and the violation that stopped them.
    fn eval_facts(&self, context: &mut Context<'_>) -> (Vec<(String, Value)>, Option<Violation>) {
        let mut facts: Vec<(String, Value)> = Vec::new();
        for (name, expr) in &self.workflow.facts {
            match expr.program().execute(context) {
                Ok(value) => {
                    context.add_variable_from_value(name.as_str(), value.clone());
                    facts.push((name.clone(), value));
                }
                Err(error) => {
                    let violation = Violation::Eval {
                        origin: format!("fact {name:?}"),
                        message: error.to_string(),
                    };
                    return (facts, Some(violation));
                }
            }
        }
        (facts, None)
    }

    /// Evaluates rules in order until one fires or fails. Returns every
    /// rule's trace and the rule that fired, if any.
    fn eval_rules(
        &self,
        context: &Context<'_>,
    ) -> (Vec<RuleTrace>, Result<Option<&Rule>, Violation>) {
        let mut traces: Vec<RuleTrace> = self
            .workflow
            .rules
            .iter()
            .map(|rule| RuleTrace {
                id: rule.id.clone(),
                when: rule.when.source().to_owned(),
                set: rule.set.clone(),
                result: RuleResult::NotReached,
            })
            .collect();
        for (rule, trace) in self.workflow.rules.iter().zip(&mut traces) {
            let failure = match rule.when.program().execute(context) {
                Ok(Value::Bool(true)) => {
                    trace.result = RuleResult::Fired;
                    return (traces, Ok(Some(rule)));
                }
                Ok(Value::Bool(false)) => {
                    trace.result = RuleResult::NotMatched;
                    continue;
                }
                Ok(other) => format!("expected bool, got {other:?}"),
                Err(error) => error.to_string(),
            };
            trace.result = RuleResult::Error(failure.clone());
            let violation = Violation::Eval {
                origin: format!("rule {:?}", rule.id),
                message: failure,
            };
            return (traces, Err(violation));
        }
        (traces, Ok(None))
    }

    /// A node that changes nothing: `next` is the current status.
    fn frozen(
        &self,
        subject: &Subject,
        current: &str,
        violation: Violation,
        partial: Partial,
    ) -> Node {
        let mut map = issue_map(subject, current);
        map.insert("parent".to_owned(), Value::Null);
        map.insert("next".to_owned(), Value::from(current));
        Node {
            value: map.into(),
            decision: decision(subject, current, current, partial, Some(violation)),
        }
    }
}

fn decision(
    subject: &Subject,
    current: &str,
    destination: &str,
    partial: Partial,
    violation: Option<Violation>,
) -> Decision {
    Decision {
        explain: Explain {
            number: subject.number.clone(),
            current: current.to_owned(),
            destination: destination.to_owned(),
            facts: partial.facts,
            rules: partial.rules,
            violation,
        },
    }
}

fn unreadable(parent: &IssueNumber, error: ResolveError) -> Violation {
    Violation::ParentUnreadable {
        parent: parent.clone(),
        error,
    }
}

fn issue_map(subject: &Subject, status: &str) -> HashMap<String, Value> {
    let fields: HashMap<String, Value> = subject
        .fields
        .iter()
        .map(|(name, value)| (name.clone(), field_value(value)))
        .collect();
    let blockers: Vec<Value> = subject
        .blockers
        .iter()
        .map(|(number, closed)| {
            let blocker: HashMap<String, Value> = HashMap::from([
                ("number".to_owned(), Value::from(number.to_string())),
                ("state".to_owned(), state(*closed)),
            ]);
            Value::from(blocker)
        })
        .collect();
    HashMap::from([
        ("number".to_owned(), number_value(&subject.number)),
        ("state".to_owned(), state(subject.closed)),
        ("status".to_owned(), Value::from(status)),
        ("labels".to_owned(), Value::from(subject.labels.clone())),
        (
            "milestone".to_owned(),
            Value::from(subject.milestone.clone()),
        ),
        ("fields".to_owned(), Value::from(fields)),
        ("blockers".to_owned(), Value::from(blockers)),
        (
            "closed_by_open_pr".to_owned(),
            Value::Bool(subject.closed_by_open_pr),
        ),
    ])
}

fn number_value(number: &IssueNumber) -> Value {
    Value::Int(i64::try_from(number.value()).unwrap_or(i64::MAX))
}

fn state(closed: bool) -> Value {
    Value::from(if closed { "closed" } else { "open" })
}

fn field_value(value: &FieldValue) -> Value {
    match value {
        FieldValue::Text(text) => Value::from(text.as_str()),
        FieldValue::Number(number) => Value::Int(*number),
    }
}

fn root_context() -> Context<'static> {
    let mut context = Context::default();
    context.add_function("fold", fold);
    context
}

/// `fold(s)`: lowercase and ascii-fold, for accent-insensitive comparison.
fn fold(text: Arc<String>) -> String {
    deunicode::deunicode(&text).to_ascii_lowercase()
}
