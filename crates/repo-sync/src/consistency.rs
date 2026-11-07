//! Offline consistency check between `tools.toml` and the hand-written issue templates and docs.
//!
//! `tools.toml` declares the `type:*` and `area:*` labels. The issue templates under
//! `.github/ISSUE_TEMPLATE/` and the label lists in the docs restate them by hand, so this check
//! fails when they drift. It reads local files only and needs no network.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const TYPE_PREFIX: &str = "type:";
const AREA_PREFIX: &str = "area:";
const TEMPLATE_DIR: &str = ".github/ISSUE_TEMPLATE";
const TEMPLATE_CONFIG: &str = "config.yml";
const DOCS: [&str; 2] = ["AGENTS.md", "CONTRIBUTING.md"];
const BLOCK_START: &str = "<!-- labels:start -->";
const BLOCK_END: &str = "<!-- labels:end -->";

/// Checks the templates and docs under `root` against the declared label names.
///
/// Returns one message per problem; an empty list means everything agrees.
#[must_use]
pub fn check(root: &Path, declared: &[String]) -> Vec<String> {
    let declared: BTreeSet<&str> = declared
        .iter()
        .map(String::as_str)
        .filter(|name| name.starts_with(TYPE_PREFIX) || name.starts_with(AREA_PREFIX))
        .collect();
    let mut problems = check_templates(&root.join(TEMPLATE_DIR), &declared);
    for doc in DOCS {
        match fs::read_to_string(root.join(doc)) {
            Ok(text) => problems.extend(check_doc(doc, &text, &declared)),
            Err(error) => problems.push(format!("{doc}: cannot read: {error}")),
        }
    }
    problems
}

fn check_templates(dir: &Path, declared: &BTreeSet<&str>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut found = BTreeSet::new();
    match fs::read_dir(dir) {
        Ok(entries) => {
            let mut files: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
            files.sort();
            for path in files {
                let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if file == TEMPLATE_CONFIG || !file.ends_with(".yml") {
                    continue;
                }
                match fs::read_to_string(&path) {
                    Ok(text) => {
                        found.insert(file.to_owned());
                        problems.extend(check_template(file, &text, declared));
                    }
                    Err(error) => {
                        problems.push(format!("{TEMPLATE_DIR}/{file}: cannot read: {error}"))
                    }
                }
            }
        }
        Err(error) => problems.push(format!("{TEMPLATE_DIR}: cannot read: {error}")),
    }
    for label in declared.iter().filter(|name| name.starts_with(TYPE_PREFIX)) {
        let file = format!("{}.yml", &label[TYPE_PREFIX.len()..]);
        if !found.contains(&file) {
            problems.push(format!(
                "{label} is declared but {TEMPLATE_DIR}/{file} is missing"
            ));
        }
    }
    problems
}

fn check_template(file: &str, text: &str, declared: &BTreeSet<&str>) -> Vec<String> {
    let at = format!("{TEMPLATE_DIR}/{file}");
    let kind = file.trim_end_matches(".yml");
    let mut problems = Vec::new();
    match top_level(text, "labels").map(parse_list) {
        Some(labels) => {
            for label in &labels {
                if !declared.contains(label.as_str()) {
                    problems.push(format!("{at}: label {label} is not declared in tools.toml"));
                }
            }
            let expected = format!("{TYPE_PREFIX}{kind}");
            if labels != [expected.clone()] {
                problems.push(format!("{at}: labels must be exactly [{expected}]"));
            }
        }
        None => problems.push(format!("{at}: no top-level `labels:` line")),
    }
    let prefix = format!("{kind}: ");
    match top_level(text, "title").map(|value| value.trim().trim_matches('"')) {
        Some(title) if title.starts_with(&prefix) => {}
        _ => problems.push(format!("{at}: `title:` must start with \"{prefix}\"")),
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
    let listed: BTreeSet<&str> = block
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|token| token.starts_with(TYPE_PREFIX) || token.starts_with(AREA_PREFIX))
        .filter(|token| token.len() > TYPE_PREFIX.len())
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

/// The value of an unindented `key:` line.
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

    fn declared<'a>(names: &[&'a str]) -> BTreeSet<&'a str> {
        names.iter().copied().collect()
    }

    #[test]
    fn template_must_carry_exactly_its_type_label_and_title_prefix() {
        let set = declared(&["type:chore", "area:core"]);
        let ok = "title: \"chore: \"\nlabels: [\"type:chore\"]\n";
        assert!(check_template("chore.yml", ok, &set).is_empty());

        let wrong = "title: \"fix: \"\nlabels: [\"type:chore\", \"area:core\"]\n";
        let problems = check_template("chore.yml", wrong, &set);
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn template_with_undeclared_label_is_reported() {
        let set = declared(&["type:chore"]);
        let text = "title: \"chore: \"\nlabels: [\"type:typo\"]\n";
        let problems = check_template("chore.yml", text, &set);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("type:typo is not declared"))
        );
    }

    #[test]
    fn template_missing_labels_line_is_reported() {
        let set = declared(&["type:chore"]);
        let problems = check_template("chore.yml", "title: \"chore: \"\n", &set);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("no top-level `labels:`"))
        );
    }

    #[test]
    fn doc_block_is_compared_as_a_set() {
        let set = declared(&["type:feat", "area:core"]);
        let ok = format!("{BLOCK_START}\n- `type:feat` and `area:core`\n{BLOCK_END}");
        assert!(check_doc("A.md", &ok, &set).is_empty());

        let drift = format!("{BLOCK_START}\n`type:feat` `type:gone`\n{BLOCK_END}");
        let problems = check_doc("A.md", &drift, &set);
        assert!(problems.iter().any(|p| p.contains("missing area:core")));
        assert!(problems.iter().any(|p| p.contains("undeclared type:gone")));
    }

    #[test]
    fn doc_without_block_is_reported() {
        let problems = check_doc("A.md", "no block", &declared(&["type:feat"]));
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn declared_type_without_template_is_reported() {
        let dir =
            std::env::temp_dir().join(format!("repo-sync-consistency-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let problems = check_templates(&dir, &declared(&["type:feat"]));
        fs::remove_dir_all(&dir).unwrap();
        assert!(problems.iter().any(|p| p.contains("feat.yml is missing")));
    }
}
