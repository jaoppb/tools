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
    assert!(matches!(error.root(), LoadError::Preset(name) if name == "builtin:nope"));
}

#[test]
fn schema_two_is_rejected_and_schema_one_is_accepted() {
    let error = load(&format!("{BLANK}schema = 2\n")).expect_err("only schema 1 exists");
    assert!(matches!(error.root(), LoadError::Schema(2)));
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
    assert!(
        matches!(error.root(), LoadError::UndeclaredStatus { status, .. } if status == "Doing")
    );
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
    assert!(matches!(error.root(), LoadError::Unknown { name, .. } if name == "isue"));
}

#[test]
fn an_unknown_function_is_rejected_at_load() {
    let error = load(&format!(
        "{BLANK}\n[workflow.facts]\nx = 'shout(issue.state)'\n"
    ))
    .expect_err("shout is not a helper");
    assert!(matches!(
        error.root(),
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
    assert!(matches!(error.root(), LoadError::FactCycle(_)));
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
    assert!(matches!(error.root(), LoadError::ReservedFact(name) if name == "issue"));
}

fn at(error: &LoadError) -> (usize, usize) {
    error.location().expect("the error carries a location")
}

#[test]
fn semantic_errors_name_the_line_and_column_of_the_value() {
    let undeclared = load(&format!(
        "{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = 'true'\nset = \"Doing\"\n"
    ))
    .expect_err("Doing is not declared");
    assert_eq!(at(&undeclared), (8, 7));

    let schema = load(&format!("{BLANK}schema = 2\n")).expect_err("only schema 1");
    assert_eq!(at(&schema), (4, 10));

    let preset = load("[workflow]\nextends = \"builtin:nope\"\n").expect_err("no preset");
    assert_eq!(at(&preset), (2, 11));

    let cycle =
        load(&format!("{BLANK}\n[workflow.facts]\na = 'b'\nb = 'a'\n")).expect_err("a cycle");
    assert_eq!(at(&cycle).0, 6);

    let unknown = load(&format!(
        "{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = 'isue.state == \"open\"'\nset = \"Done\"\n"
    ))
    .expect_err("typo");
    assert_eq!(at(&unknown), (7, 8));
}

#[test]
fn an_error_inside_a_preset_has_no_location_to_claim() {
    let error =
        load("[workflow]\nextends = \"builtin:tools\"\nstatuses = [\"Backlog\", \"Done\"]\n")
            .expect_err("the preset sets statuses this block does not declare");
    assert!(matches!(error.root(), LoadError::UndeclaredStatus { .. }));
    assert_eq!(error.location(), None);
}

#[test]
fn a_param_that_is_not_a_scalar_or_a_list_is_rejected_where_it_is_written() {
    let error =
        load(&format!("{BLANK}\n[workflow.params]\nx = {{ a = 1 }}\n")).expect_err("a table");
    assert!(matches!(error.root(), LoadError::Param(name) if name == "x"));
    assert_eq!(at(&error), (6, 5));
}

#[test]
fn both_presets_load_and_a_local_param_overrides_the_preset_one() {
    for name in ["builtin:cosmo", "builtin:tools"] {
        load(&format!("[workflow]\nextends = \"{name}\"\n")).expect("the preset loads");
    }
    load("[workflow]\nextends = \"builtin:tools\"\n[workflow.params]\ntype_prefix = \"kind:\"\n")
        .expect("a param override needs no shadow");
}

// A name bound by a macro is not free; the same spelling outside the macro is.
#[test]
fn a_macro_bound_name_is_not_a_free_identifier() {
    let blank = |expr: &str| {
        load(&format!(
            "{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = '{expr}'\nset = \"Done\"\n"
        ))
    };
    blank("issue.labels.exists(l, l == \"a\")").expect("l is bound by exists");
    blank("issue.labels.filter(l, l.startsWith(\"a\")).size() == 1").expect("filter");
    blank("issue.labels.map(l, l).all(m, m != \"\")").expect("nested macros");
    blank("issue.labels.exists(l, issue.labels.exists(m, m == l))").expect("two scopes");

    // `l` is bound inside the first macro only: the second use is free.
    let error = blank("issue.labels.exists(l, l == \"a\") && l == \"b\"")
        .expect_err("l is free outside the macro");
    assert!(matches!(error.root(), LoadError::Unknown { name, .. } if name == "l"));
}

#[test]
fn operators_and_macros_are_not_unknown_functions() {
    let blank = |expr: &str| {
        load(&format!(
            "{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = '{expr}'\nset = \"Done\"\n"
        ))
    };
    blank("!(issue.number > 1 || 2 in [1, 2]) ? has(issue.fields.X) : issue.state == \"open\"")
        .expect("operators, in, ternary and has are built in");
    let error = blank("shout(issue.state) == \"x\"").expect_err("not a helper");
    assert!(
        matches!(error.root(), LoadError::Unknown { kind: "function", name, .. } if name == "shout")
    );
}

mod config {
    use tools_core::config::{ConfigError, load_from_str};
    use tools_core::workflow::LoadError;

    const HEAD: &str = "[github.repository]\nowner = \"o\"\nname = \"n\"\n\n";

    #[test]
    fn a_tools_toml_without_workflow_has_none() {
        let config = load_from_str(HEAD).expect("loads");
        assert!(config.workflow().expect("nothing to compile").is_none());
    }

    #[test]
    fn a_workflow_block_loads_next_to_the_github_keys_and_compiles() {
        let config =
            load_from_str(&format!("{HEAD}[workflow]\nextends = \"builtin:tools\"\n")).expect("ok");
        let workflow = config.workflow().expect("compiles").expect("present");
        assert!(workflow.rule_ids().contains(&"gate-holds"));
    }

    #[test]
    fn an_invalid_workflow_fails_the_load_with_the_line_in_the_whole_file() {
        let text = format!("{HEAD}[workflow]\nextends = \"builtin:nope\"\n");
        let error = load_from_str(&text).expect_err("no such preset");
        let ConfigError::Workflow { source, .. } = &error else {
            panic!("expected a workflow error, got {error}");
        };
        assert!(matches!(source.root(), LoadError::Preset(_)));
        // The file has five lines before the value: the line is the file's.
        assert_eq!(source.location(), Some((6, 11)));
        assert!(error.to_string().contains("line 6, column 11"), "{error}");
    }

    #[test]
    fn an_unknown_key_in_workflow_is_a_parse_error_with_a_location() {
        let error =
            load_from_str(&format!("{HEAD}[workflow]\nstatuss = []\n")).expect_err("a typo");
        assert!(matches!(error, ConfigError::Parse { .. }));
        assert!(error.to_string().contains("statuss"), "{error}");
    }
}

#[test]
fn an_underscore_function_is_not_mistaken_for_an_operator() {
    let error = load(&format!(
        "{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = '_nope(issue.number) == 1'\nset = \"Done\"\n"
    ))
    .expect_err("not a helper");
    assert!(
        matches!(error.root(), LoadError::Unknown { kind: "function", name, .. } if name == "_nope")
    );
}

#[test]
fn every_built_in_function_the_loader_allows_also_runs_in_the_interpreter() {
    // A drift between the allow-list and the interpreter would turn a load-time
    // pass into a run-time freeze: evaluate each and demand no violation.
    use crate::support::{number, open_card, snapshot};
    use crate::workflow::support::{Remote, decision, evaluate};
    use tools_core::domain::Status;

    for expr in [
        "size(issue.labels) >= 0",
        "\"ab\".startsWith(\"a\") && \"ab\".endsWith(\"b\") && \"ab\".contains(\"a\")",
        "max(1, 2) == 2 && min(1, 2) == 1",
        "int(\"3\") == 3 && double(1) == 1.0 && string(1) == \"1\"",
    ] {
        let toml =
            format!("{BLANK}\n[[workflow.status]]\nid = \"r\"\nwhen = '{expr}'\nset = \"Done\"\n");
        let workflow = load(&toml).unwrap_or_else(|error| panic!("{expr}: {error}"));
        let board = snapshot(vec![open_card(1, Status::Backlog)], &[]);
        let evaluation = evaluate(&workflow, &board, &Remote::default());
        let result = decision(&evaluation, &number(1));
        assert_eq!(result.violation(), None, "{expr}");
        assert_eq!(result.fired(), Some("r"), "{expr}");
    }
}

#[test]
fn a_workflow_without_statuses_or_a_preset_says_so() {
    let error = load("[workflow]\n").expect_err("nothing declared");
    assert!(matches!(error.root(), LoadError::NoStatuses));
}
