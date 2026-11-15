//! `tools actions check`: parsing workflow references, the rules, and the `[actions]` table.

use std::cell::RefCell;

use tools_core::adapters::memory::MemoryActions;
use tools_core::application::{ActionsError, ActionsPort, audit_actions};
use tools_core::config::load_from_str;
use tools_core::domain::actions::{Major, PinFormat, RepoId, Rules, Versions, scan};

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

fn findings(rules: &Rules, workflow: &str, port: &impl ActionsPort) -> Vec<String> {
    audit_actions(port, rules, &scan("w.yml", workflow))
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

/// A port that fails for every repository and records which ones it was asked about.
#[derive(Default)]
struct Unreadable {
    asked: RefCell<Vec<String>>,
}

impl ActionsPort for Unreadable {
    fn versions(&self, repo: &RepoId) -> Result<Versions, ActionsError> {
        self.asked.borrow_mut().push(repo.to_string());
        Err(ActionsError::Read {
            repo: repo.to_string(),
            reason: "HTTP 404".to_owned(),
        })
    }
}

#[test]
fn an_unreadable_repository_is_a_finding_per_reference_and_the_audit_goes_on() {
    let port = Unreadable::default();
    let found = findings(
        &latest_only(),
        "uses: gone/x@v1\nuses: gone/x@v2\nuses: also/gone@v1\n",
        &port,
    );
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        found[0].starts_with("w.yml:1: [versions] cannot read the versions of gone/x: HTTP 404"),
        "{found:?}"
    );
    assert!(found[1].starts_with("w.yml:2: [versions]"), "{found:?}");
    assert!(found[2].contains("also/gone"), "{found:?}");
    assert_eq!(*port.asked.borrow(), ["gone/x", "also/gone"]);
}

#[test]
fn a_denied_or_disallowed_repository_is_not_looked_up() {
    let port = Unreadable::default();
    let deny = Rules {
        deny: vec!["evil/x".to_owned()],
        ..latest_only()
    };
    let denied = findings(&deny, "uses: evil/x@v1\n", &port);
    assert_eq!(denied.len(), 1, "{denied:?}");
    assert!(denied[0].contains("[deny]"), "{denied:?}");
    let allow = Rules {
        allow: Some(vec!["actions/*".to_owned()]),
        sha_matches_comment: true,
        ..Rules::default()
    };
    let disallowed = findings(&allow, &format!("uses: evil/x@{SHA} # v1\n"), &port);
    assert_eq!(disallowed.len(), 1, "{disallowed:?}");
    assert!(disallowed[0].contains("[allow]"), "{disallowed:?}");
    assert!(port.asked.borrow().is_empty());
}

#[test]
fn a_pin_format_finding_still_looks_the_repository_up() {
    let port = MemoryActions::default().with("a/b", versions(Some("v2"), &[]));
    let rules = Rules {
        pin_format: Some(PinFormat::ShaWithComment),
        ..latest_only()
    };
    let found = findings(&rules, "uses: a/b@v1\n", &port);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[1].contains("[latest-major]"), "{found:?}");
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

#[test]
fn a_bare_number_ref_is_a_branch_and_a_version_needs_the_v() {
    let references = scan("w.yml", "uses: a/b@2024\nuses: a/b@v2024\n");
    assert_eq!(references[0].claimed(), None);
    assert_eq!(references[1].claimed(), Some(Major(2024)));
}

#[test]
fn a_release_that_is_not_a_version_falls_back_to_the_highest_tag() {
    let port = MemoryActions::default().with("a/b", versions(Some("nightly"), &[("v4", SHA)]));
    assert!(findings(&latest_only(), "uses: a/b@v4\n", &port).is_empty());
}

#[test]
fn a_backport_marked_latest_does_not_lower_the_latest_major() {
    let port = MemoryActions::default().with(
        "a/b",
        versions(Some("v3.6.1"), &[("v4", SHA), ("v3", OTHER)]),
    );
    assert!(findings(&latest_only(), "uses: a/b@v4\n", &port).is_empty());
    assert_eq!(findings(&latest_only(), "uses: a/b@v3\n", &port).len(), 1);
}

#[test]
fn a_tag_names_a_major_only_with_the_v() {
    assert_eq!(Major::parse_tag("v7"), Some(Major(7)));
    assert_eq!(Major::parse_tag("v7.1.2"), Some(Major(7)));
    for text in [
        "7", "7.1", "2024.05", "20240101", "vv7", "v", "main", "v7-rc1",
    ] {
        assert_eq!(Major::parse_tag(text), None, "{text}");
    }
}

#[test]
fn bare_calver_tags_and_release_do_not_outrank_the_v_tags() {
    let port = MemoryActions::default()
        .with(
            "a/tags",
            versions(
                None,
                &[("v4", SHA), ("2024.05", OTHER), ("20240101", OTHER)],
            ),
        )
        .with("a/release", versions(Some("2024.05"), &[("v4", SHA)]));
    let rules = latest_only();
    assert!(findings(&rules, "uses: a/tags@v4\n", &port).is_empty());
    assert!(findings(&rules, "uses: a/release@v4\n", &port).is_empty());
}

#[test]
fn bare_tags_count_only_when_the_repository_has_no_v_version() {
    let port = MemoryActions::default().with(
        "a/b",
        versions(Some("1.2.3"), &[("1.2.3", SHA), ("0.9.0", OTHER)]),
    );
    let rules = Rules {
        latest_major: true,
        sha_matches_comment: true,
        ..Rules::default()
    };
    assert!(findings(&rules, &format!("uses: a/b@{SHA} # 1\n"), &port).is_empty());
    let stale = findings(&rules, &format!("uses: a/b@{OTHER} # v0\n"), &port);
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert!(stale[0].contains("latest major is v1"), "{stale:?}");
}

#[test]
fn sha_matches_comment_ignores_bare_tags_in_a_v_tagged_repository() {
    let port =
        MemoryActions::default().with("a/b", versions(None, &[("v4", SHA), ("2024.05", OTHER)]));
    let rules = Rules {
        sha_matches_comment: true,
        ..Rules::default()
    };
    let calver = findings(&rules, &format!("uses: a/b@{OTHER} # v2024\n"), &port);
    assert!(calver[0].contains("not the commit"), "{calver:?}");
}
