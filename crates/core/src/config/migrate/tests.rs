use super::*;
use crate::config::{ConfigError, load_from_str};

const HEAD: &str = "[github.repository]\nowner = 'o'\nname = 'n'\n";

fn migrated(extra: &str) -> Migration {
    migrate(&format!("{HEAD}{extra}")).expect("migrates")
}

fn loads(text: &str) -> crate::config::Config {
    load_from_str(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

#[test]
fn the_default_toggles_become_cosmo_with_a_relaxed_gate() {
    let migration = migrated("[github.issues.rules]\ncycle = true\n");
    let config = loads(&migration.text);
    assert!(migration.text.contains("extends = \"builtin:cosmo\""));
    assert!(
        migration
            .text
            .contains("triaged = \"has_type && has_area && issue.milestone != null\"")
    );
    assert!(migration.text.contains("# Ready gate of the old toggles"));
    assert!(config.workflow().expect("compiles").is_some());
}

#[test]
fn priority_and_effort_on_keep_the_strict_cosmo_gate_untouched() {
    let migration = migrated("[github.issues.rules]\npriority = true\neffort = true\n");
    loads(&migration.text);
    assert!(!migration.text.contains("triaged"));
}

#[test]
fn a_single_enabled_toggle_keeps_only_its_term() {
    let migration = migrated("[github.issues.rules]\neffort = true\n");
    loads(&migration.text);
    assert!(migration.text.contains("has(issue.fields.Effort)"));
    assert!(!migration.text.contains("has(issue.fields.Priority)"));
}

#[test]
fn no_parent_and_no_membership_pick_the_standalone_preset() {
    let migration = migrated("[github.issues.rules]\nmembership = false\nrequire_parent = false\n");
    loads(&migration.text);
    assert!(migration.text.contains("extends = \"builtin:tools\""));
    assert!(!migration.text.contains("triaged"));
    let one_side = migrated("[github.issues.rules]\nmembership = false\n");
    assert!(one_side.text.contains("extends = \"builtin:cosmo\""));
}

#[test]
fn standalone_with_effort_gets_the_limit_the_old_gate_used() {
    let migration =
        migrated("[github.issues.rules]\nmembership = false\nparent = false\neffort = true\n");
    loads(&migration.text);
    assert!(migration.text.contains("effort_limit = 8"));
}

#[test]
fn area_prefixes_and_milestone_format_become_params_only_when_not_default() {
    let custom = migrated(
        "[github.issues.rules]\narea_prefixes = ['area:', 'domain:']\nmilestone_format = 'any'\n",
    );
    loads(&custom.text);
    assert!(
        custom
            .text
            .contains("area_prefix = [\"area:\", \"domain:\"]")
    );
    assert!(custom.text.contains("milestone_format = \"any\""));
    let default =
        migrated("[github.issues.rules]\narea_prefixes = ['area:']\nmilestone_format = 'semver'\n");
    assert!(!default.text.contains("[workflow.params]"));
}

#[test]
fn the_rules_table_stays_and_comments_survive() {
    let migration = migrated("# keep me\n[github.issues.rules]\nlabels = true # inline\n");
    assert!(migration.text.contains("# keep me"));
    assert!(migration.text.contains("labels = true # inline"));
    assert!(migration.text.contains("[github.issues.rules]"));
}

#[test]
fn a_second_run_changes_nothing() {
    let first = migrated(
        "[github.project]\nnumber = 1\n[github.project.status_mapping]\nBacklog = 'Backlog'\n[github.issues.rules]\neffort = true\n",
    );
    assert!(first.changed());
    let second = migrate(&first.text).expect("migrates");
    assert!(!second.changed());
    assert_eq!(second.text, first.text);
}

#[test]
fn aliases_that_repeat_the_status_name_are_dropped() {
    let migration = migrated(
        "[github.project]\nnumber = 1\n[github.project.status_mapping]\n'In progress' = 'In progress'\n",
    );
    assert!(!migration.text.contains("status_mapping"));
    assert!(migration.text.contains("number = 1"));
    loads(&migration.text);
}

#[test]
fn a_renaming_alias_refuses_and_names_the_follow_up() {
    let error = migrate(&format!(
        "{HEAD}[github.project.status_mapping]\n'To do' = 'Backlog'\nDone = 'Done'\n"
    ))
    .expect_err("a rename cannot be expressed");
    assert_eq!(
        error,
        MigrateError::Rename(vec![("To do".to_owned(), "Backlog".to_owned())])
    );
    let message = error.to_string();
    assert!(message.contains("\"To do\" = \"Backlog\""));
    assert!(message.contains("#88"));
}

#[test]
fn a_case_only_alias_is_a_rename_because_columns_match_exactly() {
    let error = migrate(&format!(
        "{HEAD}[github.project.status_mapping]\nbacklog = 'Backlog'\n"
    ))
    .expect_err("columns are case-sensitive now");
    assert!(matches!(error, MigrateError::Rename(_)));
}

#[test]
fn an_existing_workflow_is_left_alone() {
    let text = format!(
        "{HEAD}[workflow]\nextends = \"builtin:tools\"\n[github.issues.rules]\neffort = true\n"
    );
    let migration = migrate(&text).expect("migrates");
    assert!(!migration.changed());
    assert_eq!(migration.text, text);
}

#[test]
fn a_wrongly_typed_old_key_is_a_shape_error() {
    let error = migrate(&format!("{HEAD}[github.issues.rules]\neffort = 'yes'\n"))
        .expect_err("a string is not a boolean");
    assert!(matches!(error, MigrateError::Shape(_)));
    assert!(matches!(migrate("= bad"), Err(MigrateError::Syntax(_))));
}

#[test]
fn inspect_separates_pending_from_deprecated() {
    let old = inspect(&format!(
        "{HEAD}[github.project.status_mapping]\nDone = 'Done'\n[github.issues.rules]\ncycle = true\n"
    ))
    .expect("parses");
    assert_eq!(old.pending.len(), 2);
    let done = inspect(&format!(
        "{HEAD}[workflow]\nextends = \"builtin:cosmo\"\n[github.issues.rules]\ncycle = true\n"
    ))
    .expect("parses");
    assert!(done.pending.is_empty());
    assert_eq!(done.deprecated.len(), 1);
    assert_eq!(inspect(HEAD).expect("parses"), Report::default());
}

#[test]
fn loading_status_mapping_fails_naming_the_command() {
    let error = load_from_str(&format!(
        "{HEAD}[github.project.status_mapping]\nDone = 'Done'\n"
    ))
    .expect_err("removed key");
    assert!(matches!(error, ConfigError::RemovedStatusMapping { .. }));
    assert!(error.to_string().contains("tools config migrate"));
}

#[test]
fn loading_the_rules_table_warns_until_it_is_migrated() {
    let before = loads(&format!("{HEAD}[github.issues.rules]\ncycle = true\n"));
    assert_eq!(before.warnings.len(), 1);
    assert!(before.warnings[0].contains("tools config migrate"));
    let after = loads(&format!(
        "{HEAD}[workflow]\nextends = \"builtin:tools\"\n[github.issues.rules]\ncycle = true\n"
    ));
    assert_eq!(after.warnings.len(), 1);
    assert!(!after.warnings[0].contains("tools config migrate"));
    assert!(loads(HEAD).warnings.is_empty());
}
