//! Offline consistency check between `tools.toml` and the hand-written issue templates and docs.
//!
//! `tools.toml` declares the labels and which template applies which label. The templates under
//! `.github/ISSUE_TEMPLATE/` and the label lists in the docs restate them by hand, so this check
//! fails when they drift. It reads local files only and needs no network, and it knows no label
//! vocabulary: names, prefixes and title conventions all come from `tools.toml`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use tools_core::config::IssueTemplatesConfig;

const TEMPLATE_DIR: &str = ".github/ISSUE_TEMPLATE";
const TEMPLATE_CONFIG: &str = "config.yml";
const DOCS: [&str; 2] = ["AGENTS.md", "CONTRIBUTING.md"];
const BLOCK_START: &str = "<!-- labels:start -->";
const BLOCK_END: &str = "<!-- labels:end -->";

/// Checks the templates and docs under `root` against the declared labels and templates.
///
/// Returns one message per problem; an empty list means everything agrees. Without a
/// `[github.issue_templates]` table the templates are not checked.
#[must_use]
pub fn check(
    root: &Path,
    declared: &[String],
    templates: Option<&IssueTemplatesConfig>,
) -> Vec<String> {
    let declared: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
    let mut problems = templates.map_or_else(Vec::new, |templates| {
        check_templates(&root.join(TEMPLATE_DIR), templates, &declared)
    });
    for doc in DOCS {
        match fs::read_to_string(root.join(doc)) {
            Ok(text) => problems.extend(check_doc(doc, &text, &declared)),
            Err(error) => problems.push(format!("{doc}: cannot read: {error}")),
        }
    }
    problems
}

fn check_templates(
    dir: &Path,
    templates: &IssueTemplatesConfig,
    declared: &BTreeSet<&str>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let files = match template_files(dir) {
        Ok(files) => files,
        Err(problem) => return vec![problem],
    };
    for item in &templates.items {
        let at = format!("{TEMPLATE_DIR}/{}", item.file);
        if !declared.contains(item.label.as_str()) {
            problems.push(format!(
                "{at}: label {} is not declared in tools.toml",
                item.label
            ));
        }
        match files.iter().find(|(file, _)| *file == item.file) {
            Some((_, text)) => problems.extend(check_template(&at, text, &item.label, &item.title)),
            None => problems.push(format!(
                "{at}: declared in [github.issue_templates] but missing"
            )),
        }
    }
    if templates.require_label {
        for (file, _) in &files {
            if !templates.items.iter().any(|item| item.file == *file) {
                problems.push(format!(
                    "{TEMPLATE_DIR}/{file}: has no label linked in [github.issue_templates]"
                ));
            }
        }
    }
    problems
}

/// The `(file name, contents)` of every issue template, `config.yml` excluded, sorted by name.
fn template_files(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let unreadable =
        |path: &Path, error: std::io::Error| format!("{}: cannot read: {error}", path.display());
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| unreadable(dir, error))? {
        let path = entry.map_err(|error| unreadable(dir, error))?.path();
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file == TEMPLATE_CONFIG || !file.ends_with(".yml") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|error| unreadable(&path, error))?;
        files.push((file.to_owned(), text));
    }
    files.sort();
    Ok(files)
}

fn check_template(at: &str, text: &str, label: &str, title_prefix: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if top_level(text, "labels").map(parse_list) != Some(vec![label.to_owned()]) {
        problems.push(format!("{at}: `labels:` must be exactly [{label}]"));
    }
    match top_level(text, "title").map(|value| value.trim().trim_matches('"')) {
        Some(title) if title.starts_with(title_prefix) => {}
        _ => problems.push(format!("{at}: `title:` must start with \"{title_prefix}\"")),
    }
    problems
}

fn check_doc(doc: &str, text: &str, declared: &BTreeSet<&str>) -> Vec<String> {
    let Some(block) = text
        .split_once(BLOCK_START)
        .and_then(|(_, rest)| rest.split_once(BLOCK_END))
        .map(|(block, _)| block)
    else {
        return vec![format!(
            "{doc}: missing {BLOCK_START} ... {BLOCK_END} block"
        )];
    };
    // A label is a bullet that starts with a backticked name; other text is prose.
    let listed: BTreeSet<&str> = block
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("- `")?.split('`').next())
        .collect();
    let mut problems = Vec::new();
    for label in declared.difference(&listed) {
        problems.push(format!("{doc}: label block is missing {label}"));
    }
    for label in listed.difference(declared) {
        problems.push(format!("{doc}: label block lists undeclared {label}"));
    }
    problems
}

/// The value of an unindented `key:` line. Only inline values are understood, so a block-style
/// `labels:` list reads as empty.
fn top_level<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
}

/// Parses an inline YAML list such as `["a", "b"]`.
fn parse_list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools_core::config::IssueTemplate;

    fn declared<'a>(names: &[&'a str]) -> BTreeSet<&'a str> {
        names.iter().copied().collect()
    }

    fn templates(require_label: bool, items: &[(&str, &str, &str)]) -> IssueTemplatesConfig {
        IssueTemplatesConfig {
            require_label,
            items: items
                .iter()
                .map(|(file, label, title)| IssueTemplate {
                    file: (*file).into(),
                    label: (*label).into(),
                    title: (*title).into(),
                })
                .collect(),
        }
    }

    fn dir_with(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("repo-sync-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (file, text) in files {
            fs::write(dir.join(file), text).unwrap();
        }
        dir
    }

    #[test]
    fn template_must_carry_exactly_its_label_and_title_prefix() {
        let ok = "title: \"chore: \"\nlabels: [\"x:chore\"]\n";
        assert!(check_template("a", ok, "x:chore", "chore: ").is_empty());

        let wrong = "title: \"fix: \"\nlabels: [\"x:chore\", \"y:core\"]\n";
        assert_eq!(check_template("a", wrong, "x:chore", "chore: ").len(), 2);
        assert_eq!(
            check_template("a", "title: \"chore: \"\n", "x:chore", "chore: ").len(),
            1
        );
    }

    #[test]
    fn entries_are_checked_against_declared_labels_and_files() {
        let dir = dir_with(
            "entries",
            &[
                ("a.yml", "title: \"a: \"\nlabels: [\"k:a\"]\n"),
                (TEMPLATE_CONFIG, ""),
            ],
        );
        let set = declared(&["k:a"]);
        let ok = templates(true, &[("a.yml", "k:a", "a: ")]);
        assert!(check_templates(&dir, &ok, &set).is_empty());

        let bad = templates(true, &[("a.yml", "k:zz", "a: "), ("b.yml", "k:a", "b: ")]);
        let problems = check_templates(&dir, &bad, &set);
        let _ = fs::remove_dir_all(&dir);
        assert!(problems.iter().any(|p| p.contains("k:zz is not declared")));
        assert!(
            problems
                .iter()
                .any(|p| p.contains("b.yml") && p.contains("missing"))
        );
    }

    #[test]
    fn require_label_controls_whether_unlinked_templates_fail() {
        let dir = dir_with(
            "require",
            &[("a.yml", "title: \"a: \"\nlabels: [\"k:a\"]\n")],
        );
        let set = declared(&["k:a"]);
        let strict = check_templates(&dir, &templates(true, &[]), &set);
        let lax = check_templates(&dir, &templates(false, &[]), &set);
        let _ = fs::remove_dir_all(&dir);
        assert!(
            strict
                .iter()
                .any(|p| p.contains("a.yml: has no label linked"))
        );
        assert!(lax.is_empty());
    }

    #[test]
    fn doc_block_compares_label_bullets_as_a_set_and_ignores_prose() {
        let set = declared(&["a:x", "b:y"]);
        let ok = format!(
            "{BLOCK_START}\n- **Rule `a:`**\n  - `a:x` \u{2014} see `crates/core`\n- `b:y`\n{BLOCK_END}"
        );
        assert!(check_doc("A.md", &ok, &set).is_empty());

        let drift = format!("{BLOCK_START}\n- `a:x`\n- `a:gone`\n{BLOCK_END}");
        let problems = check_doc("A.md", &drift, &set);
        assert!(problems.iter().any(|p| p.contains("missing b:y")));
        assert!(problems.iter().any(|p| p.contains("undeclared a:gone")));
    }

    #[test]
    fn doc_without_block_is_reported() {
        assert_eq!(check_doc("A.md", "no block", &declared(&["a:x"])).len(), 1);
    }
}
