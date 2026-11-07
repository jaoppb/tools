//! Shared builders: the preset, an in-memory remote and the old oracle.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use tools_core::domain::{IssueNumber, Snapshot};
use tools_core::workflow::{
    Decision, Engine, Evaluation, ParentResolver, ResolveError, Subject, Workflow,
};

use crate::support::strict_rules;

pub fn cosmo() -> Workflow {
    Workflow::from_toml("[workflow]\nextends = \"builtin:cosmo\"\n").expect("the preset loads")
}

/// A remote that knows the issues it was given and fails for the rest.
#[derive(Default)]
pub struct Remote {
    known: HashMap<IssueNumber, Subject>,
    failing: HashMap<IssueNumber, ResolveError>,
    asked: RefCell<Vec<IssueNumber>>,
}

impl Remote {
    /// This issue cannot be read, for this reason.
    pub fn failing(mut self, number: IssueNumber, error: ResolveError) -> Self {
        self.failing.insert(number, error);
        self
    }

    /// Every number the engine asked for, in order.
    pub fn asked(&self) -> Vec<IssueNumber> {
        self.asked.borrow().clone()
    }

    pub fn with(mut self, subject: Subject) -> Self {
        self.known.insert(subject.number.clone(), subject);
        self
    }
}

impl ParentResolver for Remote {
    fn fetch(&self, number: &IssueNumber) -> Result<Subject, ResolveError> {
        self.asked.borrow_mut().push(number.clone());
        if let Some(error) = self.failing.get(number) {
            return Err(error.clone());
        }
        self.known
            .get(number)
            .cloned()
            .ok_or(ResolveError::NotFound)
    }
}

/// An open issue outside the Project with these labels.
pub fn outside(number: IssueNumber, labels: &[&str]) -> Subject {
    Subject {
        number,
        closed: false,
        status: None,
        labels: labels.iter().map(|label| (*label).to_owned()).collect(),
        milestone: None,
        fields: BTreeMap::new(),
        blockers: Vec::new(),
        parent: None,
        closed_by_open_pr: false,
    }
}

pub fn evaluate(workflow: &Workflow, snapshot: &Snapshot, remote: &Remote) -> Evaluation {
    Engine::new(workflow, remote).evaluate(snapshot)
}

pub fn decision<'a>(evaluation: &'a Evaluation, number: &IssueNumber) -> &'a Decision {
    evaluation
        .decision(number)
        .expect("the card is in the snapshot")
}

/// The status each card gets from the current state machine under the strict
/// rules, which is what `builtin:cosmo` reproduces.
pub fn old_destinations(snapshot: &Snapshot) -> Vec<(IssueNumber, String)> {
    let plan = snapshot.plan_with(&strict_rules());
    snapshot
        .cards()
        .map(|card| {
            let moved = plan
                .moves()
                .iter()
                .find(|planned| planned.number() == card.number())
                .map(|planned| planned.destination().name().to_owned());
            (
                card.number().clone(),
                moved.unwrap_or_else(|| card.status().name().to_owned()),
            )
        })
        .collect()
}

/// The engine and the state machine agree on every card.
pub fn assert_parity(snapshot: &Snapshot, remote: &Remote) {
    let evaluation = evaluate(&cosmo(), snapshot, remote);
    for (number, expected) in old_destinations(snapshot) {
        let decision = decision(&evaluation, &number);
        assert_eq!(
            decision.destination(),
            expected,
            "{number}: fired {:?}, violation {:?}\n{}",
            decision.fired(),
            decision.violation(),
            decision.explain()
        );
    }
}
