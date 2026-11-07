//! What the engine does that the state machine cannot, and its trace.

use std::time::Instant;

use tools_core::workflow::{ResolveError, RuleId, RuleResult, Violation, Workflow};

use crate::support::{epic, number, open_card, snapshot, task_of};
use crate::workflow::support::{Remote, cosmo, decision, evaluate, outside};

#[test]
fn a_task_whose_story_is_outside_the_project_now_waits_for_it() {
    // The old state machine let it through because the story is not a card;
    // the engine fetches the story and applies the rule.
    let cards = vec![task_of(open_card(11, "Backlog"), 10)];
    let board = snapshot(cards, &[]);
    let remote = Remote::default().with(outside(number(10), &["type:feat"]));
    let evaluation = evaluate(&cosmo(), &board, &remote);
    assert_eq!(decision(&evaluation, &number(11)).destination(), "Backlog");
    assert_eq!(
        decision(&evaluation, &number(11))
            .fired()
            .map(RuleId::as_str),
        Some("gate-closed")
    );
}

#[test]
fn a_parent_the_token_cannot_read_freezes_only_the_child() {
    let cards = vec![
        task_of(open_card(11, "Backlog"), 10),
        open_card(20, "Backlog"),
    ];
    let evaluation = evaluate(&cosmo(), &snapshot(cards, &[]), &Remote::default());
    let child = decision(&evaluation, &number(11));
    assert_eq!(child.destination(), "Backlog");
    assert!(matches!(
        child.violation(),
        Some(Violation::ParentUnreadable { .. })
    ));
    assert_eq!(decision(&evaluation, &number(20)).destination(), "Ready");
}

#[test]
fn issues_in_a_parent_cycle_and_their_children_are_frozen() {
    let cards = vec![
        task_of(open_card(10, "Ready"), 11),
        task_of(open_card(11, "Ready"), 10),
        task_of(open_card(12, "Backlog"), 10),
        open_card(20, "Backlog"),
    ];
    let evaluation = evaluate(&cosmo(), &snapshot(cards, &[]), &Remote::default());
    for raw in [10, 11] {
        let frozen = decision(&evaluation, &number(raw));
        assert_eq!(frozen.destination(), "Ready");
        assert_eq!(frozen.violation(), Some(&Violation::Cycle));
        assert_eq!(frozen.fired().map(RuleId::as_str), None);
    }
    let below = decision(&evaluation, &number(12));
    assert!(matches!(
        below.violation(),
        Some(Violation::ParentFrozen { .. })
    ));
    assert_eq!(below.destination(), "Backlog");
    assert_eq!(decision(&evaluation, &number(20)).destination(), "Ready");
}

#[test]
fn a_parent_chain_is_evaluated_parents_first() {
    let cards = vec![
        task_of(open_card(12, "Backlog"), 11),
        task_of(open_card(11, "Backlog"), 10),
        open_card(10, "In progress"),
    ];
    let evaluation = evaluate(&cosmo(), &snapshot(cards, &[]), &Remote::default());
    assert_eq!(decision(&evaluation, &number(11)).destination(), "Ready");
    // 11 is Ready in this run, which is not started: 12 keeps waiting.
    assert_eq!(decision(&evaluation, &number(12)).destination(), "Backlog");
}

#[test]
fn a_runtime_error_in_a_rule_freezes_the_issue_without_falling_through() {
    let workflow = Workflow::from_toml(
        "[workflow]\nextends = \"builtin:cosmo\"\n\n[[workflow.status]]\nid = \"unguarded\"\nwhen = 'issue.fields.Nope > 1'\nset = \"Done\"\n",
    )
    .expect("the unguarded rule compiles");
    let cards = vec![open_card(20, "Backlog")];
    let evaluation = evaluate(&workflow, &snapshot(cards, &[]), &Remote::default());
    let frozen = decision(&evaluation, &number(20));
    assert_eq!(frozen.destination(), "Backlog");
    assert!(
        matches!(frozen.violation(), Some(Violation::Eval { origin, .. }) if origin == "rule \"unguarded\"")
    );
    assert!(matches!(
        frozen.explain().result_of("unguarded"),
        Some(RuleResult::Error(_))
    ));
    assert_eq!(
        frozen.explain().result_of("gate-holds"),
        Some(&RuleResult::NotReached)
    );
}

#[test]
fn a_param_override_changes_the_gate_without_shadowing_a_rule() {
    let workflow = Workflow::from_toml(
        "[workflow]\nextends = \"builtin:cosmo\"\n\n[workflow.params]\neffort_limit = 2\n",
    )
    .expect("a param override loads");
    let cards = vec![open_card(20, "Backlog")];
    let evaluation = evaluate(&workflow, &snapshot(cards, &[]), &Remote::default());
    assert_eq!(decision(&evaluation, &number(20)).destination(), "Backlog");
}

#[test]
fn a_param_can_rename_the_epic_label_and_the_engine_does_not_care() {
    let workflow = Workflow::from_toml(
        "[workflow]\nextends = \"builtin:cosmo\"\n\n[workflow.params]\nepic_label = \"initiative\"\n",
    )
    .expect("a param override loads");
    let board = snapshot(vec![epic(5, "Ready")], &[]);
    let by_default = evaluate(&cosmo(), &board, &Remote::default());
    assert_eq!(decision(&by_default, &number(5)).destination(), "Ready");
    // `epic` is an ordinary label now, and this card has no milestone.
    let renamed = evaluate(&workflow, &board, &Remote::default());
    assert_eq!(decision(&renamed, &number(5)).destination(), "Backlog");
}

#[test]
fn the_explain_trace_lists_every_fact_and_rule_and_marks_the_one_that_fired() {
    let blocked = crate::support::blocked_by(
        open_card(20, "Backlog"),
        21,
        tools_core::domain::Lifecycle::Open,
    );
    let evaluation = evaluate(&cosmo(), &snapshot(vec![blocked], &[]), &Remote::default());
    let text = decision(&evaluation, &number(20)).explain().to_string();
    assert!(text.starts_with("#20: Backlog -> Backlog"), "{text}");
    assert!(text.contains("unblocked = false"), "{text}");
    assert!(text.contains("[no         ] epic-is-untouched"), "{text}");
    assert!(text.contains("[FIRED      ] gate-closed"), "{text}");
    assert!(text.contains("[not reached] gate-holds"), "{text}");
}

#[test]
fn a_thousand_issues_evaluate_in_well_under_a_second() {
    let mut cards = vec![epic(1, "Backlog")];
    for raw in 2..=1000_u64 {
        let card = open_card(raw, "Backlog");
        cards.push(if raw % 5 == 0 {
            task_of(card, raw - 1)
        } else {
            card
        });
    }
    let board = snapshot(cards, &[]);
    let workflow = cosmo();
    let remote = Remote::default();
    let started = Instant::now();
    let evaluation = evaluate(&workflow, &board, &remote);
    let elapsed = started.elapsed();
    eprintln!("1000 issues in {elapsed:?} (unoptimised test build)");
    assert_eq!(evaluation.iter().count(), 1000);
    // Debug builds are ~10x slower than release; the bound only catches a
    // complexity blow-up, the real number is in the spec.
    assert!(elapsed.as_secs() < 10, "{elapsed:?}");
}

#[test]
fn each_resolve_error_freezes_the_child_and_reaches_the_violation() {
    for error in [
        ResolveError::NotFound,
        ResolveError::Forbidden,
        ResolveError::Failed("502 bad gateway".to_owned()),
    ] {
        let cards = vec![task_of(open_card(11, "Backlog"), 10)];
        let remote = Remote::default().failing(number(10), error.clone());
        let evaluation = evaluate(&cosmo(), &snapshot(cards, &[]), &remote);
        let child = decision(&evaluation, &number(11));
        assert_eq!(child.destination(), "Backlog");
        assert_eq!(
            child.violation(),
            Some(&Violation::ParentUnreadable {
                parent: number(10),
                error
            })
        );
    }
}

#[test]
fn the_resolver_is_asked_once_per_issue_even_when_many_children_share_the_parent() {
    let cards = vec![
        task_of(open_card(11, "Backlog"), 10),
        task_of(open_card(12, "Backlog"), 10),
        task_of(open_card(13, "Backlog"), 10),
    ];
    let remote = Remote::default().failing(number(10), ResolveError::Forbidden);
    let evaluation = evaluate(&cosmo(), &snapshot(cards, &[]), &remote);
    assert_eq!(remote.asked(), vec![number(10)]);
    for raw in [11, 12, 13] {
        assert!(decision(&evaluation, &number(raw)).violation().is_some());
    }
}

#[test]
fn a_child_of_a_frozen_parent_is_frozen_with_the_parent_named() {
    // #12 -> #11 -> #10 (unreadable): #11 is frozen by the read, #12 by #11.
    let cards = vec![
        task_of(open_card(11, "Backlog"), 10),
        task_of(open_card(12, "Backlog"), 11),
    ];
    let remote = Remote::default().failing(number(10), ResolveError::Forbidden);
    let evaluation = evaluate(&cosmo(), &snapshot(cards, &[]), &remote);
    assert_eq!(
        decision(&evaluation, &number(12)).violation(),
        Some(&Violation::ParentFrozen { parent: number(11) })
    );
}

#[test]
fn the_pull_requests_that_close_an_issue_are_a_list_of_numbers() {
    use tools_core::domain::{Cards, Closes, Closing, Snapshot};
    let toml = "[workflow]\nstatuses = [\"Backlog\", \"Done\"]\n\n[[workflow.status]]\nid = \"by-two\"\nwhen = 'issue.prs.size() == 2 && issue.prs[0].number == 1001 && issue.prs[1].repo == \"\"'\nset = \"Done\"\n";
    let workflow = Workflow::from_toml(toml).expect("the block loads");
    let closing = Closing::new(vec![
        Closes::new(number(1001), number(1)),
        Closes::new(number(1002), number(1)),
        Closes::new(number(1003), number(2)),
    ]);
    let board = Snapshot::new(
        Cards::new(vec![open_card(1, "Backlog"), open_card(2, "Backlog")]),
        closing,
    );
    let evaluation = evaluate(&workflow, &board, &Remote::default());
    assert_eq!(decision(&evaluation, &number(1)).destination(), "Done");
    assert_eq!(decision(&evaluation, &number(2)).destination(), "Backlog");
    assert_eq!(decision(&evaluation, &number(2)).violation(), None);
}
