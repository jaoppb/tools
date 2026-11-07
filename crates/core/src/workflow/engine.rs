//! Dependency-ordered evaluation: parents first, cycles frozen, one pass.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use cel_interpreter::{Context, Value};

use crate::domain::{IssueNumber, Snapshot};
use crate::workflow::config::Workflow;
use crate::workflow::explain::{Explain, RuleResult, RuleTrace};
use crate::workflow::schema::CURRENT;
use crate::workflow::subject::{FieldValue, ParentResolver, Subject};

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
        reason: String,
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
            Self::ParentUnreadable { parent, reason } => {
                write!(out, "parent {parent} cannot be read: {reason}")
            }
            Self::ParentFrozen { parent } => write!(out, "parent {parent} is frozen"),
            Self::Eval { origin, message } => write!(out, "{origin} failed: {message}"),
        }
    }
}

/// The decision for one Project card.
#[derive(Debug, Clone)]
pub struct Decision {
    number: IssueNumber,
    destination: String,
    fired: Option<String>,
    explain: Explain,
}

impl Decision {
    /// The issue.
    #[must_use]
    pub const fn number(&self) -> &IssueNumber {
        &self.number
    }

    /// The status the card should have; its current one when frozen.
    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// The id of the rule that fired, if one did.
    #[must_use]
    pub fn fired(&self) -> Option<&str> {
        self.fired.as_deref()
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
        self.0.iter().find(|decision| decision.number == *number)
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
        let order: Vec<IssueNumber> = snapshot
            .cards()
            .map(|card| {
                let subject = Subject::from_card(card, snapshot);
                run.subjects.insert(subject.number.clone(), Ok(subject));
                card.number().clone()
            })
            .collect();
        for number in &order {
            run.walk_chain(number);
        }
        Evaluation(
            order
                .iter()
                .map(|number| {
                    let node = run.node(number);
                    Decision {
                        number: number.clone(),
                        destination: node.next.clone(),
                        fired: node.fired.clone(),
                        explain: node.explain.clone(),
                    }
                })
                .collect(),
        )
    }
}

/// One issue once evaluated.
struct Node {
    next: String,
    fired: Option<String>,
    /// The `issue` value with derived facts and `next`, as a child sees it.
    value: Value,
    explain: Explain,
}

struct Run<'a> {
    workflow: &'a Workflow,
    resolver: &'a dyn ParentResolver,
    subjects: HashMap<IssueNumber, Result<Subject, String>>,
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

    fn node(&mut self, number: &IssueNumber) -> Rc<Node> {
        if let Some(node) = self.nodes.get(number) {
            return Rc::clone(node);
        }
        let node = Rc::new(self.compute(number));
        self.nodes.insert(number.clone(), Rc::clone(&node));
        node
    }

    fn compute(&mut self, number: &IssueNumber) -> Node {
        let subject = match self.subjects.get(number) {
            Some(Ok(subject)) => subject.clone(),
            Some(Err(reason)) => return self.unreadable(number, reason.clone()),
            None => return self.unreadable(number, "not loaded".to_owned()),
        };
        let current = subject
            .status
            .clone()
            .unwrap_or_else(|| self.workflow.default.clone());
        if self.cyclic.contains(number) {
            return self.frozen(&subject, &current, Violation::Cycle, Vec::new(), Vec::new());
        }
        let parent = match &subject.parent {
            None => Value::Null,
            Some(parent) => match self.subjects.get(parent) {
                Some(Err(reason)) => {
                    let violation = Violation::ParentUnreadable {
                        parent: parent.clone(),
                        reason: reason.clone(),
                    };
                    return self.frozen(&subject, &current, violation, Vec::new(), Vec::new());
                }
                _ => {
                    let node = self.node(parent);
                    if node.explain.violation.is_some() {
                        let violation = Violation::ParentFrozen {
                            parent: parent.clone(),
                        };
                        return self.frozen(&subject, &current, violation, Vec::new(), Vec::new());
                    }
                    node.value.clone()
                }
            },
        };
        self.evaluate(&subject, &current, parent)
    }

    fn unreadable(&self, number: &IssueNumber, reason: String) -> Node {
        let subject = Subject {
            number: number.clone(),
            closed: false,
            status: None,
            labels: Vec::new(),
            milestone: None,
            fields: Default::default(),
            blockers: Vec::new(),
            parent: None,
            closed_by_open_pr: false,
        };
        let violation = Violation::ParentUnreadable {
            parent: number.clone(),
            reason,
        };
        let current = self.workflow.default.clone();
        self.frozen(&subject, &current, violation, Vec::new(), Vec::new())
    }

    fn evaluate(&self, subject: &Subject, current: &str, parent: Value) -> Node {
        let mut map = issue_map(subject, current);
        map.insert("parent".to_owned(), parent);
        let mut context = self.root.new_inner_scope();
        context.add_variable_from_value("issue", map.clone());
        context.add_variable_from_value("params", self.workflow.params.clone());
        context.add_variable_from_value("repo", subject.number.repo().name());
        let mut facts: Vec<(String, Value)> = Vec::new();
        for (name, expr) in &self.workflow.facts {
            match expr.program().execute(&context) {
                Ok(value) => {
                    context.add_variable_from_value(name.as_str(), value.clone());
                    facts.push((name.clone(), value));
                }
                Err(error) => {
                    let violation = Violation::Eval {
                        origin: format!("fact {name:?}"),
                        message: error.to_string(),
                    };
                    return self.frozen(subject, current, violation, facts, Vec::new());
                }
            }
        }
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
        let mut outcome = None;
        for (rule, trace) in self.workflow.rules.iter().zip(&mut traces) {
            match rule.when.program().execute(&context) {
                Ok(Value::Bool(true)) => {
                    trace.result = RuleResult::Fired;
                    outcome = Some((rule.id.clone(), rule.set.clone()));
                    break;
                }
                Ok(Value::Bool(false)) => trace.result = RuleResult::NotMatched,
                Ok(other) => {
                    trace.result = RuleResult::Error(format!("expected bool, got {other:?}"));
                    break;
                }
                Err(error) => {
                    trace.result = RuleResult::Error(error.to_string());
                    break;
                }
            }
        }
        let failed = traces.iter().find_map(|trace| match &trace.result {
            RuleResult::Error(message) => Some(Violation::Eval {
                origin: format!("rule {:?}", trace.id),
                message: message.clone(),
            }),
            _ => None,
        });
        if let Some(violation) = failed {
            return self.frozen(subject, current, violation, facts, traces);
        }
        let (fired, set) = match outcome {
            Some((id, set)) => (Some(id), set),
            None => (None, CURRENT.to_owned()),
        };
        let next = if set == CURRENT {
            current.to_owned()
        } else {
            set
        };
        for (name, value) in &facts {
            map.insert(name.clone(), value.clone());
        }
        map.insert("next".to_owned(), Value::from(next.as_str()));
        Node {
            next: next.clone(),
            fired,
            value: map.into(),
            explain: Explain {
                number: subject.number.clone(),
                current: current.to_owned(),
                destination: next,
                facts,
                rules: traces,
                violation: None,
            },
        }
    }

    /// A node that changes nothing: `next` is the current status.
    fn frozen(
        &self,
        subject: &Subject,
        current: &str,
        violation: Violation,
        facts: Vec<(String, Value)>,
        rules: Vec<RuleTrace>,
    ) -> Node {
        let mut map = issue_map(subject, current);
        map.insert("parent".to_owned(), Value::Null);
        map.insert("next".to_owned(), Value::from(current));
        Node {
            next: current.to_owned(),
            fired: None,
            value: map.into(),
            explain: Explain {
                number: subject.number.clone(),
                current: current.to_owned(),
                destination: current.to_owned(),
                facts,
                rules,
                violation: Some(violation),
            },
        }
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
