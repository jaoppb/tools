//! Shared builders: the preset, an in-memory remote and the destination checks.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use tools_core::domain::{IssueNumber, Repo, Snapshot};
use tools_core::workflow::{
    Decision, Engine, Evaluation, ParentResolver, ResolveError, Subject, Workflow,
};

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
        prs: Vec::new(),
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

/// The issue of the recorded fixtures, which live in `jaoppb/cosmo`.
pub fn cosmo_issue(raw: u64) -> IssueNumber {
    IssueNumber::new(Repo::new("jaoppb", "cosmo"), raw)
}

/// The parents of the recorded fixtures that are not cards of the Project.
pub fn fixture_remote() -> Remote {
    Remote::default()
        .with(outside(cosmo_issue(2), &["epic"]))
        .with(outside(cosmo_issue(91), &["type:feat"]))
        .with(outside(cosmo_issue(107), &["epic"]))
}

/// Under `builtin:cosmo` every card of `snapshot`, in order, gets exactly
/// these destinations. The expectations were recorded from the state machine
/// of `domain/target` under the strict rules before it was deleted (#57).
pub fn assert_destinations(snapshot: &Snapshot, remote: &Remote, expected: &[&str]) {
    let evaluation = evaluate(&cosmo(), snapshot, remote);
    let got: Vec<String> = evaluation
        .iter()
        .map(|decision| decision.destination().to_string())
        .collect();
    assert_eq!(got, expected, "{}", trace_of(&evaluation));
}

fn trace_of(evaluation: &Evaluation) -> String {
    evaluation
        .iter()
        .map(|decision| decision.explain().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
