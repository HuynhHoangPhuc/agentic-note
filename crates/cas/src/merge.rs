use std::collections::HashMap;
use agentic_note_core::Result;
use crate::blob::BlobStore;
use crate::hash::ObjectId;
use crate::tree::{EntryType, Tree};

/// Information about a file that could not be auto-merged.
#[derive(Debug, Clone)]
pub struct ConflictInfo {
    pub path: String,
    pub version_a: ObjectId,
    pub version_b: ObjectId,
}

/// Result of a three-way merge operation.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// Paths of files successfully merged (took local or remote without conflict).
    pub applied: Vec<String>,
    /// Files where both sides changed relative to the ancestor — needs manual resolution.
    pub conflicts: Vec<ConflictInfo>,
}

/// Three-way merge at the tree level.
/// - If only local changed → take local.
/// - If only remote changed → take remote.
/// - If both changed → conflict.
/// - If neither changed → keep ancestor.
pub fn three_way_merge(
    store: &BlobStore,
    ancestor: &ObjectId,
    local: &ObjectId,
    remote: &ObjectId,
) -> Result<MergeResult> {
    let mut result = MergeResult {
        applied: Vec::new(),
        conflicts: Vec::new(),
    };
    merge_trees_inner(store, ancestor, local, remote, "", &mut result)?;
    Ok(result)
}

fn merge_trees_inner(
    store: &BlobStore,
    ancestor_id: &ObjectId,
    local_id: &ObjectId,
    remote_id: &ObjectId,
    prefix: &str,
    result: &mut MergeResult,
) -> Result<()> {
    let ancestor_tree = Tree::load(store, ancestor_id)?;
    let local_tree = Tree::load(store, local_id)?;
    let remote_tree = Tree::load(store, remote_id)?;

    // Build lookup maps: name -> hash
    let anc_map: HashMap<&str, (&ObjectId, &EntryType)> = ancestor_tree
        .entries.iter().map(|e| (e.name.as_str(), (&e.hash, &e.entry_type))).collect();
    let loc_map: HashMap<&str, (&ObjectId, &EntryType)> = local_tree
        .entries.iter().map(|e| (e.name.as_str(), (&e.hash, &e.entry_type))).collect();
    let rem_map: HashMap<&str, (&ObjectId, &EntryType)> = remote_tree
        .entries.iter().map(|e| (e.name.as_str(), (&e.hash, &e.entry_type))).collect();

    // Collect all names from all three trees
    let mut all_names: Vec<&str> = anc_map.keys()
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
                let local_changed = anc_hash.map_or(true, |ah| lh != ah);
                let remote_changed = anc_hash.map_or(true, |ah| rh != ah);

                if local_changed && !remote_changed {
                    result.applied.push(full_path);
                } else if !local_changed && remote_changed {
                    result.applied.push(full_path);
                } else {
                    // Both changed — recurse into subtrees or mark conflict
                    match (loc, rem) {
                        (Some((lh, EntryType::Tree)), Some((rh, EntryType::Tree))) => {
                            if let Some((ah, EntryType::Tree)) = anc {
                                merge_trees_inner(store, ah, lh, rh, &full_path, result)?;
                            } else {
                                result.conflicts.push(ConflictInfo {
                                    path: full_path,
                                    version_a: lh.clone(),
                                    version_b: rh.clone(),
                                });
                            }
                        }
                        (Some((lh, _)), Some((rh, _))) => {
                            result.conflicts.push(ConflictInfo {
                                path: full_path,
                                version_a: lh.clone(),
                                version_b: rh.clone(),
                            });
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
                    result.conflicts.push(ConflictInfo {
                        path: full_path,
                        version_a: lh.clone(),
                        version_b: String::new(),
                    });
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
                    result.conflicts.push(ConflictInfo {
                        path: full_path,
                        version_a: String::new(),
                        version_b: rh.clone(),
                    });
                }
            }

            // Deleted from both — nothing to do
            (Some(_), None, None) => {}

            _ => {}
        }
    }

    Ok(())
}
