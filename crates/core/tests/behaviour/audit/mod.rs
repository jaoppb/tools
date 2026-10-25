//! The audit use case over the in-memory repository and Project.

use tools_core::adapters::memory::{MemoryIssues, MemoryProject, MemoryText};
use tools_core::application::{Audit, ProjectState, SyncError};
use tools_core::config::RulesConfig;
use tools_core::domain::{
    IssueNumber, IssueTree, Snapshot, Status, Surface, TextItem, Texts, TreeIssue,
};

use crate::support::audit::{epic, labelled, story, text};
use crate::support::{number, open_card, snapshot};

const ANSWERS: &str = "the in-memory sources answer";

fn project_of(issues: &[TreeIssue]) -> MemoryProject {
    let cards = issues
        .iter()
        .map(|issue| open_card(issue.number().value(), Status::Backlog))
        .collect();
    MemoryProject::new(snapshot(cards, &[]))
}

type MemoryAudit = Audit<MemoryIssues, MemoryProject, MemoryText>;

fn no_texts() -> MemoryText {
    MemoryText::new(Texts::default())
}

fn audit_of(issues: Vec<TreeIssue>) -> MemoryAudit {
    let project = project_of(&issues);
    Audit::new(
        MemoryIssues::new(IssueTree::new(issues)),
        Some(project),
        no_texts(),
        RulesConfig::default(),
    )
}

fn clean() -> Vec<TreeIssue> {
    vec![epic(1), story(10, 1)]
}

fn one_violation() -> Vec<TreeIssue> {
    vec![epic(1), labelled(story(10, 1), &["type:feat"])]
}

#[test]
fn a_clean_repository_exits_with_zero() {
    let report = audit_of(clean()).run().expect(ANSWERS);
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn says_so_when_nothing_is_found() {
    let report = audit_of(clean()).run().expect(ANSWERS);
    assert_eq!(report.to_string(), "no violations found\n");
}

#[test]
fn a_violation_makes_the_report_exit_with_one() {
    let report = audit_of(one_violation()).run().expect(ANSWERS);
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn lists_each_violation_on_its_own_line_and_counts_them() {
    let two = vec![
        epic(1),
        labelled(story(10, 1), &["type:feat"]),
        labelled(story(11, 1), &["area:tooling"]),
    ];
    let report = audit_of(two).run().expect(ANSWERS);
    assert_eq!(
        report.to_string(),
        "#10 no-area: no area label\n#11 not-one-type: not exactly one type label\n2 violations\n"
    );
    let single = audit_of(one_violation()).run().expect(ANSWERS);
    assert!(single.to_string().ends_with("\n1 violation\n"));
}

#[test]
fn prints_the_same_lines_on_every_run() {
    let audit = audit_of(one_violation());
    let first = audit.run().expect(ANSWERS);
    let second = audit.run().expect(ANSWERS);
    assert_eq!(first, second);
}

#[test]
fn reports_an_issue_missing_from_the_project() {
    let issues = clean();
    let project = MemoryProject::new(snapshot(vec![open_card(1, Status::Backlog)], &[]));
    let audit = Audit::new(
        MemoryIssues::new(IssueTree::new(issues)),
        Some(project),
        no_texts(),
        RulesConfig::default(),
    );
    let report = audit.run().expect(ANSWERS);
    assert_eq!(
        report.to_string(),
        "#10 not-in-project: not an item of the Project\n1 violation\n"
    );
}

#[test]
fn stops_with_an_error_when_the_issues_cannot_be_read() {
    let audit = Audit::new(
        MemoryIssues::failing("gh failed"),
        Some(project_of(&clean())),
        no_texts(),
        RulesConfig::default(),
    );
    let failure = audit.run().expect_err("the issues are unreadable");
    assert_eq!(
        failure.to_string(),
        "cannot read the issues of the repository: gh failed"
    );
}

/// A Project that cannot be read, and refuses every write.
struct BrokenProject;

impl ProjectState for BrokenProject {
    fn snapshot(&self) -> Result<Snapshot, SyncError> {
        Err(SyncError::Read {
            reason: "no login".to_owned(),
        })
    }

    fn set_status(&mut self, number: IssueNumber, _status: Status) -> Result<(), SyncError> {
        Err(SyncError::Write {
            number,
            reason: "read only".to_owned(),
        })
    }
}

#[test]
fn stops_with_an_error_when_the_project_cannot_be_read() {
    let audit = Audit::new(
        MemoryIssues::new(IssueTree::new(clean())),
        Some(BrokenProject),
        no_texts(),
        RulesConfig::default(),
    );
    let failure = audit.run().expect_err("the Project is unreadable");
    assert_eq!(failure.to_string(), "cannot read the Project: no login");
}

#[test]
fn a_run_leaves_the_project_as_it_found_it() {
    let project = project_of(&clean());
    let before = project.snapshot().expect(ANSWERS);
    let audit = Audit::new(
        MemoryIssues::new(IssueTree::new(clean())),
        Some(project.clone()),
        no_texts(),
        RulesConfig::default(),
    );
    audit.run().expect(ANSWERS);
    assert_eq!(project.snapshot().expect(ANSWERS), before);
    assert_eq!(number(10).value(), 10);
}

fn audit_with_texts(items: Vec<TextItem>) -> MemoryAudit {
    Audit::new(
        MemoryIssues::new(IssueTree::new(clean())),
        Some(project_of(&clean())),
        MemoryText::new(Texts::new(items)),
        RulesConfig::default(),
    )
}

#[test]
fn a_text_without_references_leaves_the_report_clean() {
    let audit = audit_with_texts(vec![text(10, Surface::Body, "Links the ADR by its title.")]);
    let report = audit.run().expect(ANSWERS);
    assert_eq!(report.to_string(), "no violations found\n");
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn an_old_adr_number_makes_the_report_exit_with_one() {
    let audit = audit_with_texts(vec![text(10, Surface::Body, "See ADR-0013.")]);
    let report = audit.run().expect(ANSWERS);
    assert_eq!(
        report.to_string(),
        "#10 old-adr-number: old ADR number in the body; link the ADR by its title (only formerly ADR-NNNN may keep one)\n1 violation\n"
    );
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn stops_with_an_error_when_the_text_cannot_be_read() {
    let audit = Audit::new(
        MemoryIssues::new(IssueTree::new(clean())),
        Some(project_of(&clean())),
        MemoryText::failing("gh failed"),
        RulesConfig::default(),
    );
    let failure = audit.run().expect_err("the text is unreadable");
    assert_eq!(
        failure.to_string(),
        "cannot read the text of the issues and pull requests: gh failed"
    );
}
