use crate::blob::BlobStore;
use crate::hash::ObjectId;
use zenon_core::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    Blob,
    Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub entry_type: EntryType,
    pub hash: ObjectId,
}

/// A tree object representing a directory snapshot.
/// Entries are sorted by name for deterministic hashing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    /// Recursively walk `dir`, storing all file blobs, building tree objects,
    /// and returning the top-level `Tree` together with its stored `ObjectId`.
    /// `exclude` is a list of directory/file names to skip (e.g. `[".zenon"]`).
    pub fn from_dir(dir: &Path, store: &BlobStore, exclude: &[&str]) -> Result<(Tree, ObjectId)> {
        let mut entries: Vec<TreeEntry> = Vec::new();

        // Collect immediate children of dir
        let mut children: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                !exclude.contains(&name)
            })
            .collect();
        children.sort();

        for child in children {
            let name = child
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if child.is_dir() {
                let (_subtree, subtree_id) = Tree::from_dir(&child, store, exclude)?;
                entries.push(TreeEntry {
                    name,
                    entry_type: EntryType::Tree,
                    hash: subtree_id,
                });
            } else if child.is_file() {
                let data = std::fs::read(&child)?;
                let blob_id = store.store(&data)?;
                entries.push(TreeEntry {
                    name,
                    entry_type: EntryType::Blob,
                    hash: blob_id,
                });
            }
        }

        // Sort entries by name for deterministic hashing
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        let tree = Tree { entries };
        let json = serde_json::to_vec(&tree)
            .map_err(|e| zenon_core::AgenticError::Parse(e.to_string()))?;
        let tree_id = store.store(&json)?;
        Ok((tree, tree_id))
    }

    /// Load a tree object from the blob store.
    pub fn load(store: &BlobStore, id: &ObjectId) -> Result<Tree> {
        let data = store.load(id)?;
        serde_json::from_slice(&data)
            .map_err(|e| zenon_core::AgenticError::Parse(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn from_dir_builds_deterministic_tree() -> Result<()> {
        let base = env::temp_dir().join(format!("cas-tree-test-{}", std::process::id()));
        fs::create_dir_all(base.join("sub"))?;
        fs::write(base.join("a.md"), b"note a")?;
        fs::write(base.join("sub/b.md"), b"note b")?;

        let store_dir = base.join("objects");
        let store = BlobStore::new(store_dir);

        let (tree, id1) = Tree::from_dir(&base, &store, &["objects"])?;
        let (_, id2) = Tree::from_dir(&base, &store, &["objects"])?;

        assert_eq!(id1, id2, "tree id must be deterministic");
        assert_eq!(tree.entries.len(), 2); // a.md, sub
        Ok(())
    }

    #[test]
    fn tree_load_roundtrip() -> Result<()> {
        let base = env::temp_dir().join(format!("cas-tree-rt-{}", std::process::id()));
        fs::create_dir_all(&base)?;
        fs::write(base.join("note.md"), b"content")?;

        let store = BlobStore::new(base.join("objects"));
        let (original, id) = Tree::from_dir(&base, &store, &["objects"])?;
        let loaded = Tree::load(&store, &id)?;

        assert_eq!(original.entries.len(), loaded.entries.len());
        assert_eq!(original.entries[0].name, loaded.entries[0].name);
        Ok(())
    }
}
