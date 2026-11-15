//! `tools actions check`: parsing workflow references, the rules, and the `[actions]` table.

use tools_core::adapters::memory::MemoryActions;
use tools_core::application::audit_actions;
use tools_core::config::load_from_str;
use tools_core::domain::actions::{Major, PinFormat, Rules, Versions, scan};

const SHA: &str = "3d3c42e5aac5ba805825da76410c181273ba90b1";
const OTHER: &str = "949feb2413d6458794dcd2491c4babbbce0c15c1";

fn versions(latest: Option<&str>, tags: &[(&str, &str)]) -> Versions {
    Versions {
        latest_release: latest.map(str::to_owned),
        tags: tags
            .iter()
            .map(|(tag, sha)| ((*tag).to_owned(), (*sha).to_owned()))
            .collect(),
    }
}

fn findings(rules: &Rules, workflow: &str, port: &MemoryActions) -> Vec<String> {
    audit_actions(port, rules, &scan("w.yml", workflow))
        .expect("reads")
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn latest_only() -> Rules {
    Rules {
        latest_major: true,
        ..Rules::default()
    }
}

#[test]
fn majors_are_read_from_versions_and_nothing_else() {
    assert_eq!(Major::parse("v7"), Some(Major(7)));
    assert_eq!(Major::parse("7.1"), Some(Major(7)));
    assert_eq!(Major::parse("v7.1.2"), Some(Major(7)));
    for text in ["main", "v7-rc1", "", "v", "v1.2.3.4", "release"] {
        assert_eq!(Major::parse(text), None, "{text}");
    }
}

#[test]
fn scan_reads_tags_shas_and_skips_local_and_docker_uses() {
    let text = format!(
        "steps:\n  - uses: actions/checkout@v7\n  - name: x\n    uses: \"actions/cache@{SHA}\" # v6 pinned\n  - uses: ./local\n  - uses: docker://alpine:3\n  - uses: github/codeql-action/init@main\n"
    );
    let references = scan("w.yml", &text);
    assert_eq!(references.len(), 3);
    assert_eq!(references[0].claimed(), Some(Major(7)));
    assert_eq!(references[0].location(), "w.yml:2");
    assert_eq!(references[1].claimed(), Some(Major(6)));
    assert_eq!(references[2].repo.to_string(), "github/codeql-action");
    assert_eq!(references[2].claimed(), None);
}

#[test]
fn no_rule_enabled_means_no_finding_and_no_request() {
    let port = MemoryActions::default();
    assert!(findings(&Rules::default(), "uses: a/b@v1\n", &port).is_empty());
    assert!(!Rules::default().any());
}

#[test]
fn latest_major_compares_tag_and_sha_comment_with_the_latest_release() {
    let port = MemoryActions::default().with("actions/checkout", versions(Some("v7.0.1"), &[]));
    let rules = latest_only();
    assert!(findings(&rules, "uses: actions/checkout@v7\n", &port).is_empty());
    assert!(
        findings(
            &rules,
            &format!("uses: actions/checkout@{SHA} # v7\n"),
            &port
        )
        .is_empty()
    );
    let stale = findings(&rules, "uses: actions/checkout@v6\n", &port);
    assert_eq!(stale.len(), 1);
    assert!(
        stale[0].contains("v6") && stale[0].contains("v7"),
        "{stale:?}"
    );
}

#[test]
fn latest_major_falls_back_to_tags_and_fails_when_nothing_gives_a_major() {
    let tagged = MemoryActions::default()
        .with(
            "a/tags",
            versions(None, &[("v2", SHA), ("v10.1.0", OTHER), ("nightly", SHA)]),
        )
        .with("a/bare", versions(None, &[("nightly", SHA)]));
    let rules = latest_only();
    assert!(findings(&rules, "uses: a/tags@v10\n", &tagged).is_empty());
    assert_eq!(findings(&rules, "uses: a/tags@v2\n", &tagged).len(), 1);
    let bare = findings(&rules, "uses: a/bare@v1\n", &tagged);
    assert!(bare[0].contains("cannot determine"), "{bare:?}");
}

#[test]
fn latest_major_never_passes_a_ref_without_a_major() {
    let port = MemoryActions::default().with("a/b", versions(Some("v3"), &[]));
    let rules = latest_only();
    assert_eq!(findings(&rules, "uses: a/b@main\n", &port).len(), 1);
    assert_eq!(
        findings(&rules, &format!("uses: a/b@{SHA}\n"), &port).len(),
        1
    );
}

#[test]
fn a_github_failure_stops_the_audit() {
    let result = audit_actions(
        &MemoryActions::default(),
        &latest_only(),
        &scan("w.yml", "uses: a/b@v1\n"),
    );
    assert!(result.is_err());
}

#[test]
fn pin_format_demands_one_ref_style() {
    let port = MemoryActions::default();
    let sha_rule = Rules {
        pin_format: Some(PinFormat::ShaWithComment),
        ..Rules::default()
    };
    assert!(findings(&sha_rule, &format!("uses: a/b@{SHA} # v1\n"), &port).is_empty());
    for text in [
        format!("uses: a/b@{SHA}\n"),
        "uses: a/b@v1\n".to_owned(),
        "uses: a/b@main\n".to_owned(),
    ] {
        assert_eq!(findings(&sha_rule, &text, &port).len(), 1, "{text}");
    }
    let tag_rule = Rules {
        pin_format: Some(PinFormat::Tag),
        ..Rules::default()
    };
    assert!(findings(&tag_rule, "uses: a/b@v1\n", &port).is_empty());
    assert_eq!(
        findings(&tag_rule, &format!("uses: a/b@{SHA} # v1\n"), &port).len(),
        1
    );
}

#[test]
fn sha_matches_comment_checks_the_tag_of_the_commit() {
    let port = MemoryActions::default().with(
        "a/b",
        versions(Some("v8"), &[("v7", SHA), ("v7.1.0", SHA), ("v8", OTHER)]),
    );
    let rules = Rules {
        sha_matches_comment: true,
        ..Rules::default()
    };
    assert!(findings(&rules, &format!("uses: a/b@{SHA} # v7\n"), &port).is_empty());
    let wrong = findings(&rules, &format!("uses: a/b@{SHA} # v8\n"), &port);
    assert!(wrong[0].contains("comment says v8"), "{wrong:?}");
    let unknown = findings(
        &rules,
        &format!("uses: a/b@{} # v7\n", "0".repeat(40)),
        &port,
    );
    assert!(unknown[0].contains("not the commit"), "{unknown:?}");
    assert_eq!(
        findings(&rules, &format!("uses: a/b@{SHA}\n"), &port).len(),
        1
    );
    assert!(findings(&rules, "uses: a/b@v7\n", &port).is_empty());
}

#[test]
fn allow_deny_and_ignore_match_owner_and_repo_patterns() {
    let port = MemoryActions::default();
    let rules = Rules {
        allow: Some(vec!["actions/*".to_owned(), "pnpm/action-setup".to_owned()]),
        deny: vec!["actions/cache".to_owned()],
        ..Rules::default()
    };
    let text = "uses: Actions/checkout@v1\nuses: actions/cache@v1\nuses: pnpm/action-setup@v1\nuses: evil/x@v1\n";
    let found = findings(&rules, text, &port);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].contains("[deny]") && found[0].contains(":2:"),
        "{found:?}"
    );
    assert!(
        found[1].contains("[allow]") && found[1].contains(":4:"),
        "{found:?}"
    );
    let ignoring = Rules {
        ignore: vec!["evil/*".to_owned()],
        ..rules
    };
    assert_eq!(findings(&ignoring, text, &port).len(), 1);
}

#[test]
fn an_ignored_repository_is_not_even_looked_up() {
    let rules = Rules {
        ignore: vec!["a/b".to_owned()],
        ..latest_only()
    };
    assert!(findings(&rules, "uses: a/b@v1\n", &MemoryActions::default()).is_empty());
}

const HEAD: &str = "[github.repository]\nowner = \"o\"\nname = \"n\"\n";

#[test]
fn the_actions_table_loads_strictly_and_defaults_to_off() {
    assert_eq!(load_from_str(HEAD).expect("loads").actions(), None);
    let config = load_from_str(&format!(
        "{HEAD}[actions]\nlatest_major = true\npin_format = \"sha-with-comment\"\nsha_matches_comment = true\nallow = [\"actions/*\"]\ndeny = [\"x/y\"]\nignore = [\"a/b\"]\n"
    ))
    .expect("loads");
    let rules = config.actions().expect("set");
    assert!(rules.latest_major && rules.sha_matches_comment);
    assert_eq!(rules.pin_format, Some(PinFormat::ShaWithComment));
    assert_eq!(rules.allow, Some(vec!["actions/*".to_owned()]));
    assert!(load_from_str(&format!("{HEAD}[actions]\nlatest-major = true\n")).is_err());
    assert!(load_from_str(&format!("{HEAD}[actions]\npin_format = \"branch\"\n")).is_err());
    assert!(
        !load_from_str(&format!("{HEAD}[actions]\n"))
            .expect("loads")
            .actions()
            .expect("set")
            .any()
    );
}
