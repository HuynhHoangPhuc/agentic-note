use crate::cas::Cas;
use crate::hash::ObjectId;
use crate::tree::Tree;
use agentic_note_core::{AgenticError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: ObjectId,
    pub root_tree: ObjectId,
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub message: Option<String>,
}

impl Snapshot {
    /// Build a tree from the vault directory, persist metadata, and return the snapshot.
    pub fn create(vault: &Path, cas: &Cas, message: Option<String>) -> Result<Snapshot> {
        let exclude = [".agentic"];
        let (_tree, root_tree) = Tree::from_dir(vault, &cas.blob_store, &exclude)?;

        let timestamp = Utc::now();
        // Use timestamp + tree id as snapshot id for determinism within a second
        let raw = format!(
            "{}{}{}",
            root_tree,
            timestamp.timestamp_nanos_opt().unwrap_or(0),
            cas.device_id
        );
        let id = crate::hash::hash_bytes(raw.as_bytes());

        let snap = Snapshot {
            id: id.clone(),
            root_tree,
            timestamp,
            device_id: cas.device_id.clone(),
            message,
        };

        let json = serde_json::to_vec(&snap).map_err(|e| AgenticError::Parse(e.to_string()))?;
        let snap_path = cas.snapshots_dir.join(format!("{}.json", id));
        std::fs::write(&snap_path, &json)?;

        tracing::info!("created snapshot: {}", id);
        Ok(snap)
    }

    /// Load all snapshots, sorted by timestamp descending (newest first).
    pub fn list(cas: &Cas) -> Result<Vec<Snapshot>> {
        let mut snaps: Vec<Snapshot> = Vec::new();
        for entry in std::fs::read_dir(&cas.snapshots_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let data = std::fs::read(&path)?;
                let snap: Snapshot = serde_json::from_slice(&data)
                    .map_err(|e| AgenticError::Parse(e.to_string()))?;
                snaps.push(snap);
            }
        }
        snaps.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(snaps)
    }

    /// Load a single snapshot by its ID.
    pub fn load(cas: &Cas, id: &ObjectId) -> Result<Snapshot> {
        let path = cas.snapshots_dir.join(format!("{}.json", id));
        if !path.exists() {
            return Err(AgenticError::NotFound(format!(
                "snapshot not found: {}",
                id
            )));
        }
        let data = std::fs::read(&path)?;
        serde_json::from_slice(&data).map_err(|e| AgenticError::Parse(e.to_string()))
    }
}
