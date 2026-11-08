//! The one-shot rewrite of the removed `tools.toml` keys into `[workflow]`.
//!
//! Works on the text with `toml_edit`, so comments and layout survive. It never loads the file as
//! a [`Config`](super::Config): a file with removed keys does not load.

use toml_edit::{Array, DocumentMut, Item, Table, value};

/// The column-alias follow-up: a rename of a board column cannot be written as `[workflow]` yet.
const RENAME_ISSUE: &str = "#88";
/// The default `[github.issues.rules] area_prefixes`, which the presets already ship.
const DEFAULT_AREA_PREFIXES: [&str; 1] = ["area:"];
/// The Effort limit the old Ready gate used, for the `builtin:tools` preset that ships none.
const EFFORT_LIMIT: i64 = 8;
const BASE_GATE: &str = "has_type && has_area && issue.milestone != null";
const PRIORITY_TERM: &str = "has(issue.fields.Priority)";
const EFFORT_TERM: &str = "has(issue.fields.Effort) && issue.fields.Effort <= params.effort_limit";

/// Why a file cannot be migrated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MigrateError {
    /// The text is not TOML.
    #[error("tools.toml is not valid TOML: {0}")]
    Syntax(String),
    /// A key has a type the old schema never allowed.
    #[error("{0}")]
    Shape(String),
    /// A column alias renames a board column.
    #[error(
        "[github.project.status_mapping] renames board columns ({}); the board columns must now equal [workflow].statuses and the engine has no column aliases yet (see {RENAME_ISSUE}). Rename the columns on the board to the canonical names, then drop the key; nothing was written",
        .0.iter().map(|(column, status)| format!("\"{column}\" = \"{status}\"")).collect::<Vec<_>>().join(", ")
    )]
    Rename(Vec<(String, String)>),
}

/// What a file still needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Keys `tools config migrate` would rewrite or remove.
    pub pending: Vec<String>,
    /// Keys that stay until #59 and need no action.
    pub deprecated: Vec<String>,
}

/// A rewritten file and what changed in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    /// The new text; equal to the input when nothing changed.
    pub text: String,
    /// One line per change, and the behaviour changes to know about.
    pub notes: Vec<String>,
}

impl Migration {
    /// Whether the rewrite changed the file.
    #[must_use]
    pub const fn changed(&self) -> bool {
        !self.notes.is_empty()
    }
}

/// Which old keys `contents` still carries.
///
/// # Errors
///
/// [`MigrateError::Syntax`] when the text is not TOML.
pub fn inspect(contents: &str) -> Result<Report, MigrateError> {
    let doc = parse(contents)?;
    let mut report = Report::default();
    if status_mapping(&doc).is_some() {
        report.pending.push(
            "[github.project.status_mapping] is removed: board columns must equal [workflow].statuses"
                .to_owned(),
        );
    }
    if rules(&doc).is_some() {
        if doc.contains_key("workflow") {
            report.deprecated.push(
                "[github.issues.rules] is deprecated and stays until #59 (the issue audit still reads it)"
                    .to_owned(),
            );
        } else {
            report
                .pending
                .push("[github.issues.rules] has no [workflow] equivalent yet".to_owned());
        }
    }
    Ok(report)
}

/// Rewrites `contents`: drops `status_mapping`, and derives `[workflow]` from
/// `[github.issues.rules]` when the file has no `[workflow]` block yet. The rules table stays
/// until #59. A second run changes nothing.
///
/// # Errors
///
/// [`MigrateError::Rename`] when an alias renames a board column, [`MigrateError::Syntax`] or
/// [`MigrateError::Shape`] for text the old schema never accepted. Nothing is returned to write.
pub fn migrate(contents: &str) -> Result<Migration, MigrateError> {
    let mut doc = parse(contents)?;
    let mut notes = Vec::new();
    drop_status_mapping(&mut doc, &mut notes)?;
    if !doc.contains_key("workflow")
        && let Some(old) = rules(&doc).map(OldRules::read).transpose()?
    {
        doc.insert("workflow", Item::Table(old.workflow(&mut notes)));
    }
    let text = if notes.is_empty() {
        contents.to_owned()
    } else {
        doc.to_string()
    };
    Ok(Migration { text, notes })
}

fn parse(contents: &str) -> Result<DocumentMut, MigrateError> {
    contents
        .parse()
        .map_err(|error: toml_edit::TomlError| MigrateError::Syntax(error.to_string()))
}

fn status_mapping(doc: &DocumentMut) -> Option<&Item> {
    doc.get("github")?.get("project")?.get("status_mapping")
}

fn rules(doc: &DocumentMut) -> Option<&Item> {
    doc.get("github")?.get("issues")?.get("rules")
}

fn drop_status_mapping(doc: &mut DocumentMut, notes: &mut Vec<String>) -> Result<(), MigrateError> {
    let Some(mapping) = status_mapping(doc) else {
        return Ok(());
    };
    let pairs = mapping.as_table_like().ok_or_else(|| {
        MigrateError::Shape("[github.project.status_mapping] must be a table".to_owned())
    })?;
    let mut renames = Vec::new();
    for (column, status) in pairs.iter() {
        let status = status.as_str().ok_or_else(|| {
            MigrateError::Shape(format!(
                "[github.project.status_mapping] \"{column}\" must be a string"
            ))
        })?;
        if column.trim().is_empty() || column.trim() == status.trim() {
            continue; // an empty column was ignored; an exact repeat of the name says nothing
        }
        renames.push((column.to_owned(), status.to_owned()));
    }
    if !renames.is_empty() {
        return Err(MigrateError::Rename(renames));
    }
    if let Some(project) = doc
        .get_mut("github")
        .and_then(|github| github.get_mut("project"))
        .and_then(Item::as_table_like_mut)
    {
        project.remove("status_mapping");
    }
    notes.push(
        "removed [github.project.status_mapping] (every alias repeated its status name)".to_owned(),
    );
    Ok(())
}

/// The keys of `[github.issues.rules]` the migration reads.
struct OldRules {
    parent: bool,
    membership: bool,
    priority: bool,
    effort: bool,
    area_prefixes: Vec<String>,
    milestone_format: Option<String>,
}

impl OldRules {
    fn read(item: &Item) -> Result<Self, MigrateError> {
        let rules = item.as_table_like().ok_or_else(|| {
            MigrateError::Shape("[github.issues.rules] must be a table".to_owned())
        })?;
        let flag = |keys: &[&str], default: bool| -> Result<bool, MigrateError> {
            keys.iter()
                .find_map(|key| rules.get(key).map(|found| (key, found)))
                .map_or(Ok(default), |(key, found)| {
                    found.as_bool().ok_or_else(|| {
                        MigrateError::Shape(format!(
                            "[github.issues.rules] {key} must be a boolean"
                        ))
                    })
                })
        };
        let area_prefixes = match rules.get("area_prefixes") {
            None => DEFAULT_AREA_PREFIXES.map(str::to_owned).to_vec(),
            Some(found) => found
                .as_array()
                .and_then(|array| {
                    array
                        .iter()
                        .map(|entry| entry.as_str().map(str::to_owned))
                        .collect()
                })
                .ok_or_else(|| {
                    MigrateError::Shape(
                        "[github.issues.rules] area_prefixes must be a list of strings".to_owned(),
                    )
                })?,
        };
        let milestone_format = match rules.get("milestone_format") {
            None => None,
            Some(found) => Some(found.as_str().map(str::to_owned).ok_or_else(|| {
                MigrateError::Shape(
                    "[github.issues.rules] milestone_format must be a string".to_owned(),
                )
            })?),
        };
        Ok(Self {
            parent: flag(&["parent", "require_parent"], true)?,
            membership: flag(&["membership"], true)?,
            priority: flag(&["priority"], false)?,
            effort: flag(&["effort"], false)?,
            area_prefixes,
            milestone_format: milestone_format.filter(|format| format != "semver"),
        })
    }

    fn workflow(&self, notes: &mut Vec<String>) -> Table {
        let standalone = !self.parent && !self.membership;
        let preset = if standalone { "tools" } else { "cosmo" };
        let mut workflow = Table::new();
        workflow.insert("extends", value(format!("builtin:{preset}")));
        notes.push(format!(
            "added [workflow] extends = \"builtin:{preset}\" (parent = {}, membership = {})",
            self.parent, self.membership
        ));

        let mut params = Table::new();
        if self.area_prefixes != DEFAULT_AREA_PREFIXES {
            let list: Array = self.area_prefixes.iter().collect();
            params.insert("area_prefix", value(list));
            notes.push("area_prefixes -> [workflow.params] area_prefix".to_owned());
        }
        if let Some(format) = &self.milestone_format {
            params.insert("milestone_format", value(format));
            notes.push(
                "milestone_format -> [workflow.params] milestone_format (read by the audit rules of #59)"
                    .to_owned(),
            );
        }

        let mut facts = Table::new();
        let strict_cosmo = self.priority && self.effort;
        let relaxed = if standalone {
            self.priority || self.effort
        } else {
            !strict_cosmo
        };
        if relaxed {
            let mut terms = vec![BASE_GATE];
            if self.priority {
                terms.push(PRIORITY_TERM);
            }
            if self.effort {
                terms.push(EFFORT_TERM);
                if standalone {
                    params.insert("effort_limit", value(EFFORT_LIMIT));
                }
            }
            facts.insert("triaged", value(terms.join(" && ")));
            if let Some(mut key) = facts.key_mut("triaged") {
                key.leaf_decor_mut().set_prefix(format!(
                    "# Ready gate of the old toggles (priority = {}, effort = {}); \
                     builtin:cosmo alone requires both Priority and Effort.\n",
                    self.priority, self.effort
                ));
            }
            notes.push(format!(
                "priority = {}, effort = {} -> [workflow.facts] triaged shadows the preset gate",
                self.priority, self.effort
            ));
        }
        if !params.is_empty() {
            workflow.insert("params", Item::Table(params));
        }
        if !facts.is_empty() {
            workflow.insert("facts", Item::Table(facts));
        }
        workflow
    }
}

#[cfg(test)]
mod tests;
