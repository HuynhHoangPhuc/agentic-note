/// Post-sync merge orchestration.
///
/// Bridges the sync crate to the CAS three_way_merge function.
/// Delegates all conflict resolution to ConflictPolicy from Phase 5.
use std::path::Path;

use agentic_note_cas::merge::three_way_merge;
use agentic_note_cas::{Cas, Snapshot};
use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::ConflictPolicy;
use tracing::{info, warn};

/// Summary of a completed merge operation.
#[derive(Debug, Clone)]
pub struct MergeOutcome {
    /// Root tree ID of the materialized merge result.
    pub merged_tree: Option<String>,
    /// Files cleanly merged (no conflict, took local or remote).
    pub merged: usize,
    /// Files auto-resolved by the configured policy.
    pub auto_resolved: usize,
    /// Files that still need manual resolution.
    pub conflicts: usize,
    /// Paths of files requiring manual intervention.
    pub conflict_paths: Vec<String>,
}

/// Merge after sync: runs three_way_merge and writes conflict files if needed.
///
/// # Parameters
/// - `cas` — CAS instance for the local vault.
/// - `ancestor_id` — snapshot/tree ID of the common ancestor.
/// - `local_id` — local pre-sync snapshot/tree ID.
/// - `remote_id` — remote snapshot/tree ID.
/// - `policy` — how to resolve conflicts.
///
/// Returns a MergeOutcome summarising what happened.
pub fn merge_after_sync(
    cas: &Cas,
    ancestor_id: &str,
    local_id: &str,
    remote_id: &str,
    policy: &ConflictPolicy,
) -> Result<MergeOutcome> {
    // Resolve snapshot IDs to root tree IDs
    let ancestor_tree = resolve_to_tree(cas, ancestor_id)?;
    let local_tree = resolve_to_tree(cas, local_id)?;
    let remote_tree = resolve_to_tree(cas, remote_id)?;

    let merge_result = three_way_merge(
        &cas.blob_store,
        &ancestor_tree,
        &local_tree,
        &remote_tree,
        policy,
    )
    .map_err(|e| AgenticError::Sync(format!("three_way_merge failed: {e}")))?;

    info!(
        applied = merge_result.applied.len(),
        auto_resolved = merge_result.auto_resolved.len(),
        conflicts = merge_result.conflicts.len(),
        "merge complete"
    );

    if !merge_result.conflicts.is_empty() {
        warn!(
            count = merge_result.conflicts.len(),
            "merge conflicts require manual resolution"
        );
    }

    Ok(MergeOutcome {
        merged_tree: merge_result.merged_tree,
        merged: merge_result.applied.len(),
        auto_resolved: merge_result.auto_resolved.len(),
        conflicts: merge_result.conflicts.len(),
        conflict_paths: merge_result
            .conflicts
            .iter()
            .map(|c| c.path.clone())
            .collect(),
    })
}

/// Write conflict marker files to `.agentic/conflicts/` for manual conflicts.
///
/// Creates one file per conflict: `{conflict_dir}/{path}.conflict`
/// containing both versions with markers.
pub fn write_conflict_files(
    cas: &Cas,
    vault_path: &Path,
    conflict_paths: &[String],
    local_id: &str,
    remote_id: &str,
) -> Result<()> {
    let conflict_dir = vault_path.join(".agentic").join("conflicts");
    if conflict_dir.exists() {
        for entry in std::fs::read_dir(&conflict_dir)
            .map_err(|e| AgenticError::Sync(format!("read conflicts dir: {e}")))? {
            let entry = entry.map_err(|e| AgenticError::Sync(format!("read conflict entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("conflict") {
                std::fs::remove_file(&path)
                    .map_err(|e| AgenticError::Sync(format!("remove conflict file: {e}")))?;
            }
        }
    }
    if conflict_paths.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(&conflict_dir)
        .map_err(|e| AgenticError::Sync(format!("create conflicts dir: {e}")))?;

    let local_tree = resolve_to_tree(cas, local_id)?;
    let remote_tree = resolve_to_tree(cas, remote_id)?;

    for path in conflict_paths {
        // Load both versions from the trees
        let local_bytes = load_blob_by_path(cas, &local_tree, path).unwrap_or_default();
        let remote_bytes = load_blob_by_path(cas, &remote_tree, path).unwrap_or_default();

        let local_content = String::from_utf8_lossy(&local_bytes);
        let remote_content = String::from_utf8_lossy(&remote_bytes);

        let conflict_content = render_conflict_markers(&local_content, &remote_content);

        // Use a safe filename: replace '/' with '_'
        let safe_name = path.replace('/', "_");
        let conflict_file = conflict_dir.join(format!("{safe_name}.conflict"));
        std::fs::write(&conflict_file, conflict_content.as_bytes())
            .map_err(|e| AgenticError::Sync(format!("write conflict file {path}: {e}")))?;

        info!(path = %path, file = ?conflict_file, "wrote conflict file");
    }

    Ok(())
}

fn render_conflict_markers(local_content: &str, remote_content: &str) -> String {
    format!(
        "<<<< LOCAL\n{}\n====\n{}\n>>>> REMOTE\n",
        local_content, remote_content
    )
}

/// Resolve a snapshot ID or tree ID to a tree ObjectId.
/// If the ID is a snapshot, returns its root_tree. Otherwise returns it as-is.
fn resolve_to_tree(cas: &Cas, id: &str) -> Result<String> {
    let id_owned = id.to_string();
    match Snapshot::load(cas, &id_owned) {
        Ok(snap) => Ok(snap.root_tree),
        Err(_) => {
            // Assume it's already a tree ID
            Ok(id_owned)
        }
    }
}

/// Load the blob bytes for a given file path from a tree ID.
/// Returns None if the path is not found in the tree.
fn load_blob_by_path(cas: &Cas, tree_id: &str, file_path: &str) -> Option<Vec<u8>> {
    use agentic_note_cas::tree::{EntryType, Tree};

    let parts: Vec<&str> = file_path.split('/').collect();
    let mut current_tree_id = tree_id.to_string();

    for (i, part) in parts.iter().enumerate() {
        let tree = Tree::load(&cas.blob_store, &current_tree_id).ok()?;
        let entry = tree.entries.iter().find(|e| e.name == *part)?;

        if i == parts.len() - 1 {
            // Last segment — should be a blob
            if matches!(entry.entry_type, EntryType::Blob) {
                return cas.blob_store.load(&entry.hash).ok();
            }
            return None;
        } else {
            // Intermediate segment — must be a tree
            if matches!(entry.entry_type, EntryType::Tree) {
                current_tree_id = entry.hash.clone();
            } else {
                return None;
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_cas(dir: &Path) -> Cas {
        Cas::open(dir).expect("open cas")
    }

    #[test]
    fn merge_empty_vaults_produces_no_conflicts() {
        let dir = TempDir::new().expect("temp dir");
        let vault = dir.path();
        let cas = temp_cas(vault);

        // Create a snapshot of the empty vault
        let snap = agentic_note_cas::Snapshot::create(vault, &cas, Some("test".into()))
            .expect("create snapshot");

        let outcome = merge_after_sync(
            &cas,
            &snap.id,
            &snap.id,
            &snap.id,
            &ConflictPolicy::NewestWins,
        )
        .expect("merge after sync");

        assert_eq!(outcome.conflicts, 0);
        assert_eq!(outcome.conflict_paths, Vec::<String>::new());
    }

    #[test]
    fn merge_identical_snapshots_no_conflict() {
        let dir = TempDir::new().expect("temp dir");
        let vault = dir.path();
        std::fs::write(vault.join("note.md"), b"# Hello").expect("write note");
        let cas = temp_cas(vault);

        let snap = agentic_note_cas::Snapshot::create(vault, &cas, None).expect("create snapshot");

        let outcome = merge_after_sync(&cas, &snap.id, &snap.id, &snap.id, &ConflictPolicy::Manual)
            .expect("merge after sync");

        assert_eq!(outcome.conflicts, 0);
    }

    #[test]
    fn write_conflict_files_creates_directory() {
        let dir = TempDir::new().expect("temp dir");
        let vault = dir.path();
        let cas = temp_cas(vault);

        let snap = agentic_note_cas::Snapshot::create(vault, &cas, None).expect("create snapshot");
        let conflict_paths = vec!["some/note.md".to_string()];

        write_conflict_files(&cas, vault, &conflict_paths, &snap.id, &snap.id)
            .expect("write conflict files");

        let conflicts_dir = vault.join(".agentic").join("conflicts");
        assert!(conflicts_dir.exists());
    }

    #[test]
    fn write_conflict_files_removes_stale_entries() {
        let dir = TempDir::new().expect("temp dir");
        let vault = dir.path();
        let cas = temp_cas(vault);
        let conflicts_dir = vault.join(".agentic").join("conflicts");
        std::fs::create_dir_all(&conflicts_dir).expect("create conflicts dir");
        std::fs::write(conflicts_dir.join("stale.conflict"), b"old").expect("write stale");

        let snap = agentic_note_cas::Snapshot::create(vault, &cas, None).expect("create snapshot");
        write_conflict_files(&cas, vault, &[], &snap.id, &snap.id).expect("clear conflict files");

        let entries = std::fs::read_dir(&conflicts_dir)
            .expect("read conflicts dir")
            .count();
        assert_eq!(entries, 0);
    }
}
