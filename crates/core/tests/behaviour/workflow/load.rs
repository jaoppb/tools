//! Load-time validation: every mistake is caught before an issue is seen.

use tools_core::workflow::{LoadError, Workflow};

fn load(toml: &str) -> Result<Workflow, LoadError> {
    Workflow::from_toml(toml)
}

const BLANK: &str = r#"
[workflow]
statuses = ["Todo", "Done"]
"#;

#[test]
fn the_cosmo_preset_loads_by_itself() {
    load("[workflow]\nextends = \"builtin:cosmo\"\n").expect("the preset loads");
}

#[test]
fn a_blank_slate_workflow_loads_without_the_vocabulary_of_any_preset() {
    load(&format!(
        "{BLANK}\n[[workflow.status]]\nid = \"all\"\nwhen = 'true'\nset = \"Todo\"\n"
    ))
    .expect("a workflow needs no preset");
}

#[test]
fn an_unknown_preset_is_rejected() {
    let error = load("[workflow]\nextends = \"builtin:nope\"\n").expect_err("no such preset");
    assert!(matches!(error, LoadError::Preset(name) if name == "builtin:nope"));
}

#[test]
fn schema_two_is_rejected_and_schema_one_is_accepted() {
    let error = load(&format!("{BLANK}schema = 2\n")).expect_err("only schema 1 exists");
    assert!(matches!(error, LoadError::Schema(2)));
    load(&format!("{BLANK}schema = 1\n")).expect("schema 1 is the current one");
}

#[test]
fn an_unknown_key_is_a_toml_error_with_a_location() {
    let error = load("[workflow]\nstatuss = []\n").expect_err("a typo");
    let text = error.to_string();
    assert!(text.contains("statuss"), "{text}");
    assert!(text.contains("line 2"), "{text}");
}

#[test]
fn a_rule_that_sets_an_undeclared_status_is_rejected() {
    let error = load(&format!(
        "{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = 'true'\nset = \"Doing\"\n"
    ))
    .expect_err("Doing is not declared");
    assert!(matches!(error, LoadError::UndeclaredStatus { status, .. } if status == "Doing"));
}

#[test]
fn a_syntax_error_names_the_rule_and_the_position() {
    let error = load(&format!(
        "{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = 'issue.labels.exists(l, l == \"a\"'\nset = \"Done\"\n"
    ))
    .expect_err("a missing parenthesis");
    let text = error.to_string();
    assert!(text.contains("rule \"r\""), "{text}");
    assert!(text.contains("<input>:1:"), "{text}");
}

#[test]
fn an_unknown_identifier_is_rejected_at_load() {
    let error = load(&format!(
        "{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = 'isue.state == \"open\"'\nset = \"Done\"\n"
    ))
    .expect_err("isue is a typo");
    assert!(matches!(error, LoadError::Unknown { name, .. } if name == "isue"));
}

#[test]
fn an_unknown_function_is_rejected_at_load() {
    let error = load(&format!(
        "{BLANK}\n[workflow.facts]\nx = 'shout(issue.state)'\n"
    ))
    .expect_err("shout is not a helper");
    assert!(matches!(
        error,
        LoadError::Unknown {
            kind: "function",
            ..
        }
    ));
}

#[test]
fn facts_that_read_each_other_in_a_circle_are_rejected() {
    let error =
        load(&format!("{BLANK}\n[workflow.facts]\na = 'b'\nb = 'a'\n")).expect_err("a cycle");
    assert!(matches!(error, LoadError::FactCycle(_)));
}

#[test]
fn a_local_rule_with_a_builtin_id_shadows_the_builtin() {
    let workflow = load(
        "[workflow]\nextends = \"builtin:cosmo\"\n\n[[workflow.status]]\nid = \"closed-is-done\"\nwhen = 'issue.state == \"closed\"'\nset = \"Backlog\"\n",
    )
    .expect("a shadow loads");
    let ids = workflow.rule_ids();
    assert_eq!(ids.iter().filter(|id| **id == "closed-is-done").count(), 1);
    assert_eq!(ids[0], "closed-is-done");
}

#[test]
fn a_fact_named_like_a_schema_root_is_rejected() {
    let error =
        load(&format!("{BLANK}\n[workflow.facts]\nissue = 'true'\n")).expect_err("hides issue");
    assert!(matches!(error, LoadError::ReservedFact(name) if name == "issue"));
}
