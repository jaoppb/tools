//! Generic two-pass collection reconciliation with rename support against desired specification.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::domain::repo::{DeleteReason, RepoPlanError};

/// Defines key extraction and diff variant construction for collection reconciliation.
pub trait ReconcileHandler<C, D, Diff> {
    /// Case-insensitive key for current remote item.
    fn key_current<'a>(&self, item: &'a C) -> &'a str;
    /// Case-insensitive key for desired config item.
    fn key_desired<'a>(&self, item: &'a D) -> &'a str;
    /// Compares current against desired and returns an update diff if fields differ.
    fn diff(&self, current: &C, desired: &D) -> Option<Diff>;
    /// Constructs a creation diff for a missing desired item.
    fn create(&self, desired: &D) -> Diff;
    /// Constructs a deletion diff for a deprecated or unmanaged item.
    fn delete(&self, current: &C, reason: DeleteReason) -> Diff;
    /// Constructs an unmanaged diff for an unmanaged item when pruning is disabled.
    fn unmanaged(&self, current: &C) -> Diff;
}

/// Validates static configuration of desired items, deprecated keys, and rename mappings.
///
/// # Errors
///
/// Returns [`RepoPlanError::Validation`] on any static configuration error.
pub fn validate_collection_spec<'a, I>(
    desired_keys: I,
    deprecated: &[String],
    renames: &BTreeMap<String, String>,
) -> Result<(), RepoPlanError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut desired_set: HashSet<String> = HashSet::new();
    for key in desired_keys {
        let lower = key.to_ascii_lowercase();
        if !desired_set.insert(lower) {
            return Err(RepoPlanError::Validation(format!(
                "duplicate item '{key}' in desired items"
            )));
        }
    }

    let deprecated_keys: HashSet<String> = deprecated
        .iter()
        .map(|key| key.to_ascii_lowercase())
        .collect();

    let mut rename_sources_lower: HashSet<String> = HashSet::new();
    for source in renames.keys() {
        let lower = source.to_ascii_lowercase();
        if !rename_sources_lower.insert(lower) {
            return Err(RepoPlanError::Validation(format!(
                "duplicate rename source '{source}'"
            )));
        }
    }

    for (source, target) in renames {
        let source_lower = source.to_ascii_lowercase();
        let target_lower = target.to_ascii_lowercase();

        if source_lower == target_lower {
            return Err(RepoPlanError::Validation(format!(
                "rename source '{source}' cannot be identical to target '{target}'"
            )));
        }

        if rename_sources_lower.contains(&target_lower) {
            return Err(RepoPlanError::Validation(format!(
                "rename cycle or chain detected: '{source}' -> '{target}'"
            )));
        }

        if desired_set.contains(&source_lower) {
            return Err(RepoPlanError::Validation(format!(
                "rename source '{source}' cannot also be declared in desired items"
            )));
        }

        if !desired_set.contains(&target_lower) {
            return Err(RepoPlanError::Validation(format!(
                "rename target '{target}' for source '{source}' is not declared in desired items"
            )));
        }

        if deprecated_keys.contains(&source_lower) {
            return Err(RepoPlanError::Validation(format!(
                "rename source '{source}' cannot also be declared in deprecated list"
            )));
        }

        if deprecated_keys.contains(&target_lower) {
            return Err(RepoPlanError::Validation(format!(
                "rename target '{target}' cannot also be declared in deprecated list"
            )));
        }
    }

    Ok(())
}

/// Reconciles a collection of current remote items against desired items, deprecated items, and renames.
///
/// Implements the two-pass algorithm with rename resolution and collision/cycle detection:
/// 1. Validate renames against cycles, chains, missing targets, duplicate sources, and deprecated entries.
/// 2. Walk desired items:
///    - If both the desired item and a rename source exist remotely, report a collision error.
///    - If multiple rename sources exist remotely for the same target, report a collision error.
///    - If present in current, match and diff.
///    - If missing in current but a rename source exists remotely, match and diff the rename.
///    - Otherwise, emit `Create`.
/// 3. Walk current items: if already matched in step 2, skip. If in deprecated, emit `Delete(Deprecated)`.
///    If `prune_unmanaged`, emit `Delete(Unmanaged)`. Otherwise, emit `Unmanaged`.
///
/// # Errors
///
/// Returns [`RepoPlanError::Validation`] on configuration errors or [`RepoPlanError::Collision`]
/// on remote state collisions.
pub fn reconcile_by_key<C, D, Diff, H>(
    current: &[C],
    desired: &[D],
    deprecated: &[String],
    renames: &BTreeMap<String, String>,
    prune_unmanaged: bool,
    handler: H,
) -> Result<Vec<Diff>, RepoPlanError>
where
    H: ReconcileHandler<C, D, Diff>,
{
    validate_collection_spec(
        desired.iter().map(|item| handler.key_desired(item)),
        deprecated,
        renames,
    )?;

    let mut current_by_key: HashMap<String, &C> = HashMap::new();
    for item in current {
        current_by_key.insert(handler.key_current(item).to_ascii_lowercase(), item);
    }

    let deprecated_keys: HashSet<String> = deprecated
        .iter()
        .map(|key| key.to_ascii_lowercase())
        .collect();

    let mut renames_by_target: HashMap<String, Vec<&str>> = HashMap::new();
    for (source, target) in renames {
        renames_by_target
            .entry(target.to_ascii_lowercase())
            .or_default()
            .push(source.as_str());
    }

    let mut diffs = Vec::new();
    let mut matched_current_keys: HashSet<String> = HashSet::new();

    // 1. Reconcile desired items against current (O(M))
    for item in desired {
        let target_key = handler.key_desired(item).to_ascii_lowercase();
        let direct = current_by_key.get(&target_key);

        let mut existing_sources: Vec<(&str, &C)> = Vec::new();
        if let Some(sources) = renames_by_target.get(&target_key) {
            for source in sources {
                if let Some(c) = current_by_key.get(&source.to_ascii_lowercase()) {
                    existing_sources.push((*source, c));
                }
            }
        }

        if direct.is_some() && !existing_sources.is_empty() {
            return Err(RepoPlanError::Collision(format!(
                "rename collision: both source '{}' and target '{}' exist on GitHub",
                existing_sources[0].0,
                handler.key_desired(item)
            )));
        }

        if existing_sources.len() > 1 {
            return Err(RepoPlanError::Collision(format!(
                "rename collision: multiple rename sources '{}' and '{}' for target '{}' exist on GitHub",
                existing_sources[0].0,
                existing_sources[1].0,
                handler.key_desired(item)
            )));
        }

        let matched = direct
            .copied()
            .map(|existing| (target_key, existing))
            .or_else(|| {
                existing_sources
                    .first()
                    .map(|(source_name, existing)| (source_name.to_ascii_lowercase(), *existing))
            });

        if let Some((matched_key, existing)) = matched {
            matched_current_keys.insert(matched_key);
            if let Some(diff) = handler.diff(existing, item) {
                diffs.push(diff);
            }
        } else {
            diffs.push(handler.create(item));
        }
    }

    // 2. Reconcile current items for deprecated or unmanaged (O(N))
    for item in current {
        let key = handler.key_current(item).to_ascii_lowercase();
        if matched_current_keys.contains(&key) {
            continue;
        }

        if deprecated_keys.contains(&key) {
            diffs.push(handler.delete(item, DeleteReason::Deprecated));
        } else if prune_unmanaged {
            diffs.push(handler.delete(item, DeleteReason::Unmanaged));
        } else {
            diffs.push(handler.unmanaged(item));
        }
    }

    Ok(diffs)
}
