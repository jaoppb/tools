//! Generic two-pass collection reconciliation against desired specification.

use std::collections::{HashMap, HashSet};

use crate::domain::repo::DeleteReason;

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

/// Reconciles a collection of current remote items against desired items and deprecated items.
///
/// Implements the two-pass algorithm:
/// 1. Walk desired items: if present in current and differing, emit `Update`. If missing, emit `Create`.
/// 2. Walk current items: if present in desired, skip (handled in pass 1). If in deprecated, emit `Delete(Deprecated)`.
///    If `prune_unmanaged`, emit `Delete(Unmanaged)`. Otherwise, emit `Unmanaged`.
pub fn reconcile_by_key<C, D, Diff, H>(
    current: &[C],
    desired: &[D],
    deprecated: &[String],
    prune_unmanaged: bool,
    handler: H,
) -> Vec<Diff>
where
    H: ReconcileHandler<C, D, Diff>,
{
    let mut diffs = Vec::new();
    let current_by_key: HashMap<String, &C> = current
        .iter()
        .map(|item| (handler.key_current(item).to_ascii_lowercase(), item))
        .collect();
    let desired_keys: HashSet<String> = desired
        .iter()
        .map(|item| handler.key_desired(item).to_ascii_lowercase())
        .collect();
    let deprecated_keys: HashSet<String> = deprecated
        .iter()
        .map(|key| key.to_ascii_lowercase())
        .collect();

    // 1. Reconcile desired items against current (O(M))
    for item in desired {
        let key = handler.key_desired(item).to_ascii_lowercase();
        if let Some(existing) = current_by_key.get(&key) {
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
        if desired_keys.contains(&key) {
            // Already reconciled in Step 1. Desired takes precedence.
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

    diffs
}
