use crate::cas::Cas;
use crate::hash::ObjectId;
use crate::snapshot::Snapshot;
use crate::tree::{EntryType, Tree};
use zenon_core::Result;
use std::path::Path;
use tracing::info;

/// Restore a vault to the state recorded in `snapshot_id`.
/// A backup snapshot is created first so the current state is preserved.
pub fn restore(vault: &Path, cas: &Cas, snapshot_id: &ObjectId) -> Result<()> {
    // 1. Create a backup snapshot of current state before touching anything
    let _backup = Snapshot::create(vault, cas, Some("pre-restore backup".to_string()))?;
    info!("backup snapshot created before restore");

    // 2. Load the target snapshot
    let snap = Snapshot::load(cas, snapshot_id)?;

    // 3. Rebuild directory tree from the snapshot's root tree
    restore_tree(cas, &snap.root_tree, vault)?;

    info!("vault restored to snapshot {}", snapshot_id);
    Ok(())
}

/// Recursively restore files from a tree object into `dest_dir`.
/// Files present on disk but absent from the tree are removed.
fn restore_tree(cas: &Cas, tree_id: &ObjectId, dest_dir: &Path) -> Result<()> {
    let tree = Tree::load(&cas.blob_store, tree_id)?;

    // Collect names that should exist after restore
    let expected_names: std::collections::HashSet<&str> =
        tree.entries.iter().map(|e| e.name.as_str()).collect();

    // Remove entries in dest_dir not present in the tree (skip .zenon)
    if dest_dir.exists() {
        for entry in std::fs::read_dir(dest_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == ".zenon" {
                continue;
            }
            if !expected_names.contains(name_str.as_ref()) {
                let path = entry.path();
                if path.is_dir() {
                    std::fs::remove_dir_all(&path)?;
                } else {
                    std::fs::remove_file(&path)?;
                }
            }
        }
    } else {
        std::fs::create_dir_all(dest_dir)?;
    }

    // Write / recurse entries
    for entry in &tree.entries {
        let dest_path = dest_dir.join(&entry.name);
        match entry.entry_type {
            EntryType::Blob => {
                let data = cas.blob_store.load(&entry.hash)?;
                std::fs::write(&dest_path, &data)?;
            }
            EntryType::Tree => {
                std::fs::create_dir_all(&dest_path)?;
                restore_tree(cas, &entry.hash, &dest_path)?;
            }
        }
    }

    Ok(())
}
