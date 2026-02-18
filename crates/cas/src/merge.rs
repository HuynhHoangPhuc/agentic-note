use crate::blob::BlobStore;
use crate::conflict_policy::{resolve_conflict, ConflictResolution};
use crate::hash::ObjectId;
use crate::tree::{EntryType, Tree};
use agentic_note_core::types::ConflictPolicy;
use agentic_note_core::Result;
use std::collections::HashMap;

/// Information about a file that could not be auto-merged.
#[derive(Debug, Clone)]
pub struct ConflictInfo {
    pub path: String,
    pub version_a: ObjectId,
    pub version_b: ObjectId,
}

/// Information recorded for each automatically resolved conflict during a merge.
#[derive(Debug, Clone)]
pub struct AutoResolution {
    pub path: String,
    pub policy: ConflictPolicy,
    pub result_blob_id: ObjectId,
    pub description: String,
}

/// Result of a three-way merge operation.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// Paths of files successfully merged (took local or remote without conflict).
    pub applied: Vec<String>,
    /// Files where both sides changed but were auto-resolved by the active policy.
    pub auto_resolved: Vec<AutoResolution>,
    /// Files where both sides changed relative to the ancestor — needs manual resolution.
    pub conflicts: Vec<ConflictInfo>,
}

/// Three-way merge at the tree level.
/// - If only local changed → take local.
/// - If only remote changed → take remote.
/// - If both changed → apply `policy` (auto-resolve or conflict).
/// - If neither changed → keep ancestor.
///
/// # Errors
///
/// Returns an error if any tree or blob cannot be loaded from the CAS.
///
/// # Examples
///
/// ```no_run
/// use agentic_note_cas::{Cas, Snapshot, three_way_merge};
/// use agentic_note_core::types::ConflictPolicy;
/// # use std::path::Path;
/// # fn main() -> agentic_note_core::Result<()> {
/// let cas = Cas::open(Path::new("/path/to/vault"))?;
/// let snap = Snapshot::create(Path::new("/path/to/vault"), &cas, None)?;
/// let _merge = three_way_merge(
///     &cas.blob_store,
///     &snap.id,
///     &snap.id,
///     &snap.id,
///     &ConflictPolicy::Manual,
/// )?;
/// # Ok(()) }
/// ```
pub fn three_way_merge(
    store: &BlobStore,
    ancestor: &ObjectId,
    local: &ObjectId,
    remote: &ObjectId,
    policy: &ConflictPolicy,
) -> Result<MergeResult> {
    let mut result = MergeResult {
        applied: Vec::new(),
        auto_resolved: Vec::new(),
        conflicts: Vec::new(),
    };
    merge_trees_inner(store, ancestor, local, remote, "", policy, &mut result)?;
    Ok(result)
}

fn merge_trees_inner(
    store: &BlobStore,
    ancestor_id: &ObjectId,
    local_id: &ObjectId,
    remote_id: &ObjectId,
    prefix: &str,
    policy: &ConflictPolicy,
    result: &mut MergeResult,
) -> Result<()> {
    let ancestor_tree = Tree::load(store, ancestor_id)?;
    let local_tree = Tree::load(store, local_id)?;
    let remote_tree = Tree::load(store, remote_id)?;

    // Build lookup maps: name -> hash
    let anc_map: HashMap<&str, (&ObjectId, &EntryType)> = ancestor_tree
        .entries
        .iter()
        .map(|e| (e.name.as_str(), (&e.hash, &e.entry_type)))
        .collect();
    let loc_map: HashMap<&str, (&ObjectId, &EntryType)> = local_tree
        .entries
        .iter()
        .map(|e| (e.name.as_str(), (&e.hash, &e.entry_type)))
        .collect();
    let rem_map: HashMap<&str, (&ObjectId, &EntryType)> = remote_tree
        .entries
        .iter()
        .map(|e| (e.name.as_str(), (&e.hash, &e.entry_type)))
        .collect();

    // Collect all names from all three trees
    let mut all_names: Vec<&str> = anc_map
        .keys()
        .chain(loc_map.keys())
        .chain(rem_map.keys())
        .copied()
        .collect();
    all_names.sort_unstable();
    all_names.dedup();

    for name in all_names {
        let full_path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix, name)
        };

        let anc = anc_map.get(name).map(|(h, t)| (*h, *t));
        let loc = loc_map.get(name).map(|(h, t)| (*h, *t));
        let rem = rem_map.get(name).map(|(h, t)| (*h, *t));

        match (anc, loc, rem) {
            // Unchanged in both — nothing to do
            (Some((ah, _)), Some((lh, _)), Some((rh, _))) if lh == ah && rh == ah => {}

            // Only local changed
            (_, Some((lh, _)), Some((rh, _))) if lh != rh => {
                let anc_hash = anc.map(|(h, _)| h);
                let local_changed = anc_hash != Some(lh);
                let remote_changed = anc_hash != Some(rh);

                if (local_changed && !remote_changed) || (!local_changed && remote_changed) {
                    result.applied.push(full_path);
                } else {
                    // Both changed — recurse into subtrees or apply policy
                    match (loc, rem) {
                        (Some((lh, EntryType::Tree)), Some((rh, EntryType::Tree))) => {
                            if let Some((ah, EntryType::Tree)) = anc {
                                merge_trees_inner(store, ah, lh, rh, &full_path, policy, result)?;
                            } else {
                                apply_policy(
                                    store,
                                    full_path,
                                    lh.clone(),
                                    rh.clone(),
                                    policy,
                                    result,
                                )?;
                            }
                        }
                        (Some((lh, _)), Some((rh, _))) => {
                            apply_policy(store, full_path, lh.clone(), rh.clone(), policy, result)?;
                        }
                        _ => {}
                    }
                }
            }

            // Only in local (added locally or deleted remotely)
            (_, Some((lh, _)), None) => {
                let anc_hash = anc.map(|(h, _)| h);
                if anc_hash.is_none() {
                    // Added locally — take it
                    result.applied.push(full_path);
                } else {
                    // Deleted remotely, kept locally — conflict
                    apply_policy(store, full_path, lh.clone(), String::new(), policy, result)?;
                }
            }

            // Only in remote (added remotely or deleted locally)
            (_, None, Some((rh, _))) => {
                let anc_hash = anc.map(|(h, _)| h);
                if anc_hash.is_none() {
                    // Added remotely — take it
                    result.applied.push(full_path);
                } else {
                    // Deleted locally, kept remotely — conflict
                    apply_policy(store, full_path, String::new(), rh.clone(), policy, result)?;
                }
            }

            // Deleted from both — nothing to do
            (Some(_), None, None) => {}

            _ => {}
        }
    }

    Ok(())
}

/// Apply the conflict resolution policy, routing to auto_resolved or conflicts.
fn apply_policy(
    store: &BlobStore,
    path: String,
    version_a: ObjectId,
    version_b: ObjectId,
    policy: &ConflictPolicy,
    result: &mut MergeResult,
) -> Result<()> {
    let info = ConflictInfo {
        path: path.clone(),
        version_a,
        version_b,
    };

    // Skip policy resolution for delete-vs-modify cases (empty blob ids)
    if info.version_a.is_empty() || info.version_b.is_empty() {
        result.conflicts.push(info);
        return Ok(());
    }

    match resolve_conflict(store, &info, policy)? {
        ConflictResolution::Resolved {
            merged_blob_id,
            description,
        } => {
            result.auto_resolved.push(AutoResolution {
                path,
                policy: policy.clone(),
                result_blob_id: merged_blob_id,
                description,
            });
        }
        ConflictResolution::Unresolved(conflict_info) => {
            result.conflicts.push(conflict_info);
        }
    }

    Ok(())
}
