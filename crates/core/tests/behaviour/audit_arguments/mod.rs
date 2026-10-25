//! The command line of `issues-check`.

use tools_core::cli::audit::AuditArguments;

fn arguments(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn accepts_no_arguments() {
    assert!(AuditArguments::parse(arguments(&["issues-check"])).is_ok());
}

#[test]
fn rejects_an_unknown_argument_with_exit_code_two() {
    let failure = AuditArguments::parse(arguments(&["issues-check", "--invalid-flag"]))
        .expect_err("unknown flag fails");
    assert_eq!(failure.exit_code(), 2);
}

#[test]
fn shows_the_usage_and_exits_with_zero_for_help() {
    let help = AuditArguments::parse(arguments(&["issues-check", "--help"]))
        .expect_err("help is not a run");
    assert_eq!(help.exit_code(), 0);
    assert!(help.to_string().contains("issues-check"));
}
