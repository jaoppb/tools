//! `[workflow.params]`: their TOML shape against the preset, and the CEL value
//! every expression reads as `params`.
//!
//! A local param keeps the TOML type of the preset's value: a list stays a
//! list of the same element kind, a string stays a string, an integer stays an
//! integer. Params the preset does not ship are not typed.

use std::collections::{BTreeMap, HashMap};

use cel::Value;

/// The TOML kind of a value, as an error names it.
fn kind(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "list",
        toml::Value::Table(_) => "table",
    }
}

/// The kind every element of `items` shares; `None` when it is empty or mixed.
fn element_kind(items: &[toml::Value]) -> Option<&'static str> {
    let (first, rest) = items.split_first()?;
    let shared = kind(first);
    rest.iter()
        .all(|item| kind(item) == shared)
        .then_some(shared)
}

/// Whether `local` has the shape of `preset`: the same kind, and for a list
/// whose preset elements share a kind, elements of that kind only.
pub(crate) fn fits(preset: &toml::Value, local: &toml::Value) -> bool {
    match (preset, local) {
        (toml::Value::Array(expected), toml::Value::Array(items)) => element_kind(expected)
            .is_none_or(|shared| items.iter().all(|item| kind(item) == shared)),
        (expected, value) => kind(expected) == kind(value),
    }
}

/// Whether `local`, a scalar where the preset has a list, fits once wrapped in
/// a one-element list.
pub(crate) fn wraps(preset: &toml::Value, local: &toml::Value) -> bool {
    let wrapped = toml::Value::Array(vec![local.clone()]);
    matches!(preset, toml::Value::Array(_)) && !local.is_array() && fits(preset, &wrapped)
}

/// The shape of a value in words: `a string`, `a list of strings`.
pub(crate) fn describe(value: &toml::Value) -> String {
    match value {
        toml::Value::Array(items) if items.is_empty() => "an empty list".to_owned(),
        toml::Value::Array(items) => element_kind(items).map_or_else(
            || "a list of mixed kinds".to_owned(),
            |shared| format!("a list of {shared}s"),
        ),
        toml::Value::Integer(_) => "an integer".to_owned(),
        other => format!("a {}", kind(other)),
    }
}

/// The params as one CEL map; the name of the first param CEL cannot carry
/// (neither a scalar nor a list of scalars) otherwise.
pub(crate) fn cel_value<'a>(
    params: impl IntoIterator<Item = (&'a String, &'a toml::Value)>,
) -> Result<Value, &'a String> {
    params
        .into_iter()
        .map(|(name, param)| {
            scalar_or_list(param)
                .map(|value| (name.clone(), value))
                .ok_or(name)
        })
        .collect::<Result<HashMap<_, _>, _>>()
        .map(Value::from)
}

fn scalar_or_list(value: &toml::Value) -> Option<Value> {
    match value {
        toml::Value::Array(items) => items
            .iter()
            .map(scalar)
            .collect::<Option<Vec<_>>>()
            .map(Value::from),
        other => scalar(other),
    }
}

fn scalar(value: &toml::Value) -> Option<Value> {
    match value {
        toml::Value::String(text) => Some(Value::from(text.as_str())),
        toml::Value::Integer(number) => Some(Value::Int(*number)),
        toml::Value::Float(number) => Some(Value::Float(*number)),
        toml::Value::Boolean(flag) => Some(Value::Bool(*flag)),
        _ => None,
    }
}

/// The local params of a `[workflow]` table that are a scalar where its
/// preset has a list they fit once wrapped, for `tools config check|migrate`.
/// Empty without `extends` or for an unknown preset.
pub(crate) fn wrappable(workflow: &toml::Table) -> Vec<String> {
    let preset = workflow
        .get("extends")
        .and_then(toml::Value::as_str)
        .and_then(crate::workflow::config::preset_params);
    let (Some(preset), Some(local)) = (
        preset,
        workflow.get("params").and_then(toml::Value::as_table),
    ) else {
        return Vec::new();
    };
    local
        .iter()
        .filter(|(name, value)| preset.get(*name).is_some_and(|base| wraps(base, value)))
        .map(|(name, _)| name.clone())
        .collect()
}

/// The params a preset ships, by name.
pub(crate) type PresetParams = BTreeMap<String, toml::Value>;

#[cfg(test)]
mod tests {
    use super::{describe, fits, wraps};

    fn value(text: &str) -> toml::Value {
        let table: toml::Table = toml::from_str(&format!("v = {text}")).expect("a TOML value");
        table["v"].clone()
    }

    #[test]
    fn a_param_keeps_the_kind_of_the_preset() {
        assert!(fits(&value("\"type:\""), &value("\"kind/\"")));
        assert!(fits(&value("8"), &value("3")));
        assert!(
            !fits(&value("8"), &value("8.5")),
            "integer and float differ"
        );
        assert!(!fits(&value("8"), &value("\"8\"")));
        assert!(!fits(&value("[\"area:\"]"), &value("\"area:\"")));
        assert!(!fits(&value("\"type:\""), &value("[\"type:\"]")));
    }

    #[test]
    fn a_list_keeps_the_element_kind_of_the_preset() {
        assert!(fits(&value("[\"area:\"]"), &value("[\"a:\", \"b:\"]")));
        assert!(fits(&value("[\"area:\"]"), &value("[]")), "empty is fine");
        assert!(!fits(&value("[\"area:\"]"), &value("[1]")));
        assert!(!fits(&value("[\"area:\"]"), &value("[\"a:\", 1]")));
        assert!(
            fits(&value("[]"), &value("[1, \"a\"]")),
            "an empty preset list says nothing"
        );
    }

    #[test]
    fn only_a_scalar_of_the_element_kind_wraps() {
        assert!(wraps(&value("[\"area:\"]"), &value("\"area:\"")));
        assert!(!wraps(&value("[\"area:\"]"), &value("3")));
        assert!(!wraps(&value("[\"area:\"]"), &value("[\"area:\"]")));
        assert!(!wraps(&value("\"type:\""), &value("\"type:\"")));
    }

    #[test]
    fn a_shape_reads_as_words() {
        assert_eq!(describe(&value("\"x\"")), "a string");
        assert_eq!(describe(&value("8")), "an integer");
        assert_eq!(describe(&value("8.5")), "a float");
        assert_eq!(describe(&value("true")), "a boolean");
        assert_eq!(describe(&value("[\"a\"]")), "a list of strings");
        assert_eq!(describe(&value("[]")), "an empty list");
        assert_eq!(describe(&value("[1, \"a\"]")), "a list of mixed kinds");
    }
}
