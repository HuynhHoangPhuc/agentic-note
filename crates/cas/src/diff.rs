use std::collections::HashMap;
use agentic_note_core::Result;
use crate::blob::BlobStore;
use crate::hash::ObjectId;
use crate::tree::{EntryType, Tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub path: String,
    pub status: DiffStatus,
}

/// Recursively compare two trees, returning per-file diff entries.
/// `prefix` is the current path prefix used for building full relative paths.
pub fn diff_trees(
    store: &BlobStore,
    tree_a: &ObjectId,
    tree_b: &ObjectId,
) -> Result<Vec<DiffEntry>> {
    diff_trees_inner(store, tree_a, tree_b, "")
}

fn diff_trees_inner(
    store: &BlobStore,
    tree_a_id: &ObjectId,
    tree_b_id: &ObjectId,
    prefix: &str,
) -> Result<Vec<DiffEntry>> {
    let tree_a = Tree::load(store, tree_a_id)?;
    let tree_b = Tree::load(store, tree_b_id)?;

    // Build lookup maps: name -> (hash, type)
    let map_a: HashMap<&str, (&ObjectId, &EntryType)> = tree_a
        .entries
        .iter()
        .map(|e| (e.name.as_str(), (&e.hash, &e.entry_type)))
        .collect();

    let map_b: HashMap<&str, (&ObjectId, &EntryType)> = tree_b
        .entries
        .iter()
        .map(|e| (e.name.as_str(), (&e.hash, &e.entry_type)))
        .collect();

    let mut results = Vec::new();

    // Entries in A
    for (name, (hash_a, etype_a)) in &map_a {
        let full_path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix, name)
        };

        match map_b.get(name) {
            None => {
                // Deleted — emit all blobs under this entry
                collect_deleted(store, hash_a, etype_a, &full_path, &mut results)?;
            }
            Some((hash_b, etype_b)) => {
                if hash_a != hash_b {
                    match (etype_a, etype_b) {
                        (EntryType::Blob, EntryType::Blob) => {
                            results.push(DiffEntry { path: full_path, status: DiffStatus::Modified });
                        }
                        (EntryType::Tree, EntryType::Tree) => {
                            let sub = diff_trees_inner(store, hash_a, hash_b, &full_path)?;
                            results.extend(sub);
                        }
                        _ => {
                            // Type changed — treat old as deleted, new as added
                            collect_deleted(store, hash_a, etype_a, &full_path, &mut results)?;
                            collect_added(store, hash_b, etype_b, &full_path, &mut results)?;
                        }
                    }
                }
            }
        }
    }

    // Entries only in B (added)
    for (name, (hash_b, etype_b)) in &map_b {
        if !map_a.contains_key(name) {
            let full_path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", prefix, name)
            };
            collect_added(store, hash_b, etype_b, &full_path, &mut results)?;
        }
    }

    Ok(results)
}

fn collect_deleted(
    store: &BlobStore,
    hash: &ObjectId,
    etype: &EntryType,
    path: &str,
    out: &mut Vec<DiffEntry>,
) -> Result<()> {
    match etype {
        EntryType::Blob => out.push(DiffEntry { path: path.to_string(), status: DiffStatus::Deleted }),
        EntryType::Tree => {
            let tree = Tree::load(store, hash)?;
            for entry in &tree.entries {
                let child_path = format!("{}/{}", path, entry.name);
                collect_deleted(store, &entry.hash, &entry.entry_type, &child_path, out)?;
            }
        }
    }
    Ok(())
}

fn collect_added(
    store: &BlobStore,
    hash: &ObjectId,
    etype: &EntryType,
    path: &str,
    out: &mut Vec<DiffEntry>,
) -> Result<()> {
    match etype {
        EntryType::Blob => out.push(DiffEntry { path: path.to_string(), status: DiffStatus::Added }),
        EntryType::Tree => {
            let tree = Tree::load(store, hash)?;
            for entry in &tree.entries {
                let child_path = format!("{}/{}", path, entry.name);
                collect_added(store, &entry.hash, &entry.entry_type, &child_path, out)?;
            }
        }
    }
    Ok(())
}
