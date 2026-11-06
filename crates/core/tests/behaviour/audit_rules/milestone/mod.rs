//! Every open issue has a milestone, and its title has the configured format.

use tools_core::config::{MilestoneFormat, RulesConfig, TitlePattern};
use tools_core::domain::{Lifecycle, TreeIssue};

use crate::support::audit::{blocked_by, closed, epic, in_milestone, labels, lines, story, survey};
use crate::support::number;

fn without_milestone(lifecycle: Lifecycle) -> TreeIssue {
    TreeIssue::new(number(10), lifecycle)
        .with_labels(labels(&["type:chore", "area:tooling"]))
        .with_parent(number(1))
}

#[test]
fn reports_an_open_issue_with_no_milestone() {
    let broken = survey(vec![epic(1), without_milestone(Lifecycle::Open)]);
    assert_eq!(lines(&broken), ["#10 no-milestone: no milestone"]);
    let repaired = survey(vec![epic(1), story(10, 1)]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn does_not_report_a_closed_issue_with_no_milestone() {
    let history = survey(vec![epic(1), without_milestone(Lifecycle::Closed)]);
    assert!(lines(&history).is_empty());
}

#[test]
fn reports_an_open_issue_with_a_milestone_that_is_not_a_release() {
    let broken = survey(vec![epic(1), in_milestone(story(10, 1), "backlog")]);
    assert_eq!(
        lines(&broken),
        ["#10 unreadable-milestone: milestone is not a release such as v0.1.0 or v0.2.0"]
    );
    let repaired = survey(vec![epic(1), in_milestone(story(10, 1), "v0.1.0")]);
    assert!(lines(&repaired).is_empty());
}

#[test]
fn does_not_report_a_closed_issue_with_a_milestone_that_is_not_a_release() {
    let history = in_milestone(closed(10, 1), "backlog");
    assert!(lines(&survey(vec![epic(1), history])).is_empty());
}

fn audit_with(format: MilestoneFormat, issues: Vec<TreeIssue>) -> Vec<String> {
    let rules = RulesConfig {
        milestone_format: format,
        ..RulesConfig::default()
    };
    survey(issues)
        .violations(&rules)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn accepts_any_milestone_title_when_the_format_is_any() {
    let phase = in_milestone(
        story(10, 1),
        "Auditoria · Fase 2 — servidor, sessão e progresso",
    );
    assert!(audit_with(MilestoneFormat::Any, vec![epic(1), phase]).is_empty());
}

#[test]
fn still_requires_a_milestone_when_the_format_is_any() {
    let found = audit_with(
        MilestoneFormat::Any,
        vec![epic(1), without_milestone(Lifecycle::Open)],
    );
    assert_eq!(found, ["#10 no-milestone: no milestone"]);
}

#[test]
fn checks_the_title_against_a_pattern() {
    let format = MilestoneFormat::Pattern(TitlePattern::new("^(v1|Fase \\d+)$").unwrap());
    let good = in_milestone(story(10, 1), "Fase 2");
    let bad = in_milestone(story(11, 1), "backlog");
    let found = audit_with(format, vec![epic(1), good, bad]);
    assert_eq!(
        found,
        ["#11 invalid-milestone-format: milestone does not match the configured milestone_format"]
    );
}

#[test]
fn does_not_report_a_closed_issue_whose_title_misses_the_pattern() {
    let format = MilestoneFormat::Pattern(TitlePattern::new("^(v1|Fase \\d+)$").unwrap());
    let history = in_milestone(closed(10, 1), "backlog");
    assert!(audit_with(format, vec![epic(1), history]).is_empty());
}

#[test]
fn skips_release_ordering_for_non_semver_milestones() {
    let phase = in_milestone(story(10, 1), "Fase 2");
    let blocker = in_milestone(epic(11), "v9");
    let tree = vec![epic(1), blocked_by(phase, &[11]), blocker];
    assert!(audit_with(MilestoneFormat::Any, tree).is_empty());
}

#[test]
fn accepts_phase_milestones_when_the_release_rule_is_off() {
    let rules = RulesConfig {
        milestone_format: MilestoneFormat::Any,
        release: false,
        ..RulesConfig::default()
    };
    let phase = in_milestone(story(10, 1), "Sprint 4");
    assert!(survey(vec![epic(1), phase]).violations(&rules).is_empty());
}
