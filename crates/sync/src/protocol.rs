/// Sync protocol state machine.
///
/// Exchanges snapshots between peers, diffs them, transfers missing blobs,
/// then delegates to merge_driver for the actual merge.
use std::path::Path;

use zenon_cas::{restore, Cas, Snapshot};
use zenon_core::error::{AgenticError, Result};
use zenon_core::types::ConflictPolicy;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::merge_driver::{merge_after_sync, write_conflict_files};
use crate::transport::{SyncConnection, SyncMessage};

/// Result of a completed sync operation.
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Number of files that were cleanly merged (no conflict).
    pub merged: usize,
    /// Number of files auto-resolved by the configured conflict policy.
    pub auto_resolved: usize,
    /// Number of files that need manual resolution.
    pub conflicts: usize,
    /// ID of the post-sync snapshot.
    pub snapshot_id: String,
}

/// Payload format for blob transfer during sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncPayload {
    /// Uncompressed blob data.
    FullBlob { hash: String, data: Vec<u8> },
    /// Zstd-compressed blob data.
    CompressedBlob {
        hash: String,
        compressed_data: Vec<u8>,
    },
}

impl SyncPayload {
    /// Create a payload from blob data, optionally compressing.
    pub fn from_blob(hash: String, data: Vec<u8>, compress: bool, level: i32) -> Result<Self> {
        if compress {
            let compressed_data = crate::compression::compress(&data, level)?;
            Ok(Self::CompressedBlob {
                hash,
                compressed_data,
            })
        } else {
            Ok(Self::FullBlob { hash, data })
        }
    }

    /// Extract the hash from the payload.
    pub fn hash(&self) -> &str {
        match self {
            Self::FullBlob { hash, .. } => hash,
            Self::CompressedBlob { hash, .. } => hash,
        }
    }

    /// Decompress and return the blob data.
    pub fn into_data(self) -> Result<(String, Vec<u8>)> {
        match self {
            Self::FullBlob { hash, data } => Ok((hash, data)),
            Self::CompressedBlob {
                hash,
                compressed_data,
            } => {
                let data = crate::compression::decompress(&compressed_data)?;
                Ok((hash, data))
            }
        }
    }
}

/// Run the full sync protocol as the initiator (Device A).
///
/// Steps:
/// 1. Create a pre-sync snapshot of the local vault.
/// 2. Send SyncRequest with local snapshot ID.
/// 3. Receive SyncResponse with remote snapshot ID.
/// 4. Send all local blobs that the remote is missing.
/// 5. Receive blobs that we are missing locally.
/// 6. Run three_way_merge with ancestor + local + remote.
/// 7. Create a post-sync snapshot.
/// 8. Exchange SyncComplete.
pub async fn run_sync_initiator(
    conn: &mut dyn SyncConnection,
    cas: &Cas,
    vault_path: &Path,
    policy: &ConflictPolicy,
) -> Result<SyncResult> {
    // Step 1: Create pre-sync snapshot
    let local_snap = Snapshot::create(vault_path, cas, Some("pre-sync".into()))
        .map_err(|e| AgenticError::Sync(format!("create pre-sync snapshot: {e}")))?;
    info!(snapshot_id = %local_snap.id, "created pre-sync snapshot");

    // Step 2: Send SyncRequest
    conn.send(&SyncMessage::SyncRequest {
        snapshot_id: local_snap.id.clone(),
        root_tree_id: local_snap.root_tree.clone(),
    })
    .await?;
    debug!("sent SyncRequest");

    // Step 3: Receive SyncResponse
    let (remote_snap_id, remote_root_tree_id) = match conn.recv().await? {
        SyncMessage::SyncResponse {
            snapshot_id,
            root_tree_id,
        } => (snapshot_id, root_tree_id),
        SyncMessage::Error { message } => {
            return Err(AgenticError::Sync(format!("peer error: {message}")));
        }
        other => {
            return Err(AgenticError::Sync(format!("unexpected message: {other:?}")));
        }
    };
    debug!(remote_snap_id = %remote_snap_id, "received SyncResponse");

    // Step 4: Find common ancestor (best-effort: look for matching snapshot ID in local store)
    let ancestor_id = find_common_ancestor(cas, &local_snap.id, &remote_snap_id)?;
    debug!(ancestor = %ancestor_id, "resolved common ancestor");

    // Step 5: Exchange missing blobs
    // Ask remote what blobs it needs
    let local_blob_ids = list_snapshot_blobs(cas, &local_snap.id)?;
    conn.send(&SyncMessage::BlobRequest {
        ids: local_blob_ids.clone(),
    })
    .await?;

    // Receive list of remote blob IDs that it has but we don't
    let remote_blob_ids = match conn.recv().await? {
        SyncMessage::BlobRequest { ids } => ids,
        SyncMessage::Error { message } => {
            return Err(AgenticError::Sync(format!(
                "peer blob req error: {message}"
            )));
        }
        other => {
            return Err(AgenticError::Sync(format!(
                "unexpected msg awaiting BlobRequest: {other:?}"
            )));
        }
    };

    // Send blobs the remote doesn't have
    let blobs_to_send: Vec<(String, Vec<u8>)> = remote_blob_ids
        .iter()
        .filter_map(|id| {
            // Only send blobs that WE have (they're asking for ones we advertised)
            cas.blob_store.load(id).ok().map(|data| (id.clone(), data))
        })
        .collect();

    conn.send(&SyncMessage::BlobBatch {
        blobs: blobs_to_send,
    })
    .await?;

    // Receive blobs from remote
    let received_batch = match conn.recv().await? {
        SyncMessage::BlobBatch { blobs } => blobs,
        SyncMessage::Error { message } => {
            return Err(AgenticError::Sync(format!(
                "peer blob batch error: {message}"
            )));
        }
        other => {
            return Err(AgenticError::Sync(format!(
                "unexpected msg awaiting BlobBatch: {other:?}"
            )));
        }
    };

    // Store received blobs locally
    for (id, data) in &received_batch {
        let stored_id = cas
            .blob_store
            .store(data)
            .map_err(|e| AgenticError::Sync(format!("store received blob: {e}")))?;
        if &stored_id != id {
            return Err(AgenticError::Sync(format!(
                "blob hash mismatch: expected {id}, got {stored_id}"
            )));
        }
    }
    debug!(count = received_batch.len(), "stored received blobs");

    // Step 6: Merge
    let outcome = merge_after_sync(
        cas,
        &ancestor_id,
        &local_snap.id,
        &remote_root_tree_id,
        policy,
    )?;

    maybe_restore_merged_vault(cas, vault_path, &outcome.merged_tree, &local_snap.root_tree)?;
    write_conflict_files(
        cas,
        vault_path,
        &outcome.conflict_paths,
        &local_snap.id,
        &remote_root_tree_id,
    )?;

    // Step 7: Create post-sync snapshot
    let post_snap = Snapshot::create(vault_path, cas, Some("post-sync".into()))
        .map_err(|e| AgenticError::Sync(format!("create post-sync snapshot: {e}")))?;
    info!(snapshot_id = %post_snap.id, "created post-sync snapshot");

    // Step 8: Exchange SyncComplete
    conn.send(&SyncMessage::SyncComplete {
        snapshot_id: post_snap.id.clone(),
    })
    .await?;

    match conn.recv().await? {
        SyncMessage::SyncComplete { snapshot_id } => {
            debug!(peer_snap = %snapshot_id, "peer confirmed sync complete");
        }
        SyncMessage::Error { message } => {
            return Err(AgenticError::Sync(format!(
                "peer sync complete error: {message}"
            )));
        }
        _ => {} // Non-fatal: peer may have sent something else
    }

    conn.close().await?;

    Ok(SyncResult {
        merged: outcome.merged,
        auto_resolved: outcome.auto_resolved,
        conflicts: outcome.conflicts,
        snapshot_id: post_snap.id,
    })
}

/// Run the full sync protocol as the responder (Device B).
pub async fn run_sync_responder(
    conn: &mut dyn SyncConnection,
    cas: &Cas,
    vault_path: &Path,
    policy: &ConflictPolicy,
) -> Result<SyncResult> {
    // Create pre-sync snapshot
    let local_snap = Snapshot::create(vault_path, cas, Some("pre-sync".into()))
        .map_err(|e| AgenticError::Sync(format!("create pre-sync snapshot (responder): {e}")))?;

    // Receive SyncRequest
    let (initiator_snap_id, initiator_root_tree_id) = match conn.recv().await? {
        SyncMessage::SyncRequest {
            snapshot_id,
            root_tree_id,
        } => (snapshot_id, root_tree_id),
        other => {
            let _ = conn
                .send(&SyncMessage::Error {
                    message: format!("expected SyncRequest, got {other:?}"),
                })
                .await;
            return Err(AgenticError::Sync("expected SyncRequest".into()));
        }
    };

    // Send SyncResponse
    conn.send(&SyncMessage::SyncResponse {
        snapshot_id: local_snap.id.clone(),
        root_tree_id: local_snap.root_tree.clone(),
    })
    .await?;

    // Find ancestor
    let ancestor_id = find_common_ancestor(cas, &local_snap.id, &initiator_snap_id)?;

    // Receive initiator's blob list
    let initiator_blob_ids = match conn.recv().await? {
        SyncMessage::BlobRequest { ids } => ids,
        other => {
            return Err(AgenticError::Sync(format!(
                "expected BlobRequest, got {other:?}"
            )));
        }
    };

    // Send our blob list (what we have that they may need)
    let local_blob_ids = list_snapshot_blobs(cas, &local_snap.id)?;
    // Determine which of their blobs we need
    let needed: Vec<String> = initiator_blob_ids
        .iter()
        .filter(|id| !cas.blob_store.exists(id))
        .cloned()
        .collect();
    conn.send(&SyncMessage::BlobRequest { ids: needed }).await?;

    // Receive blobs they're sending us
    let received_batch = match conn.recv().await? {
        SyncMessage::BlobBatch { blobs } => blobs,
        other => {
            return Err(AgenticError::Sync(format!(
                "expected BlobBatch, got {other:?}"
            )));
        }
    };

    for (id, data) in &received_batch {
        let stored_id = cas
            .blob_store
            .store(data)
            .map_err(|e| AgenticError::Sync(format!("store received blob: {e}")))?;
        if &stored_id != id {
            return Err(AgenticError::Sync(format!(
                "blob hash mismatch: expected {id}, got {stored_id}"
            )));
        }
    }

    // Send blobs they asked for (our local_blob_ids that they don't have)
    let blobs_to_send: Vec<(String, Vec<u8>)> = local_blob_ids
        .iter()
        .filter_map(|id| cas.blob_store.load(id).ok().map(|data| (id.clone(), data)))
        .collect();
    conn.send(&SyncMessage::BlobBatch {
        blobs: blobs_to_send,
    })
    .await?;

    // Merge
    let outcome = merge_after_sync(
        cas,
        &ancestor_id,
        &local_snap.id,
        &initiator_root_tree_id,
        policy,
    )?;

    maybe_restore_merged_vault(cas, vault_path, &outcome.merged_tree, &local_snap.root_tree)?;
    write_conflict_files(
        cas,
        vault_path,
        &outcome.conflict_paths,
        &local_snap.id,
        &initiator_root_tree_id,
    )?;

    // Create post-sync snapshot
    let post_snap = Snapshot::create(vault_path, cas, Some("post-sync".into()))
        .map_err(|e| AgenticError::Sync(format!("create post-sync snapshot (responder): {e}")))?;

    // Exchange SyncComplete
    conn.send(&SyncMessage::SyncComplete {
        snapshot_id: post_snap.id.clone(),
    })
    .await?;

    if let SyncMessage::SyncComplete { .. } = conn.recv().await? {}

    conn.close().await?;

    Ok(SyncResult {
        merged: outcome.merged,
        auto_resolved: outcome.auto_resolved,
        conflicts: outcome.conflicts,
        snapshot_id: post_snap.id,
    })
}

/// Find common ancestor snapshot ID.
/// Heuristic: if one snapshot ID matches a locally stored snapshot, that's the ancestor.
/// Falls back to empty tree ObjectId when no ancestor found.
fn find_common_ancestor(cas: &Cas, _local_id: &str, remote_id: &str) -> Result<String> {
    let remote_id_owned = remote_id.to_string();
    // If remote_id is known locally, use it directly as ancestor
    if Snapshot::load(cas, &remote_id_owned).is_ok() {
        return Ok(remote_id_owned);
    }
    // Fall back to local history. This is conservative but safer than assuming
    // the current local pre-sync snapshot is shared by the remote peer.
    let snapshots = Snapshot::list(cas).unwrap_or_default();
    if let Some(oldest) = snapshots.last() {
        return Ok(oldest.id.clone());
    }
    // No common ancestor — store an empty tree and use its ID.
    // merge will treat all files as new additions on both sides.
    let empty_tree = zenon_cas::tree::Tree { entries: vec![] };
    let json = serde_json::to_vec(&empty_tree)
        .map_err(|e| AgenticError::Sync(format!("serialize empty tree: {e}")))?;
    let empty_id = cas
        .blob_store
        .store(&json)
        .map_err(|e| AgenticError::Sync(format!("store empty tree: {e}")))?;
    Ok(empty_id)
}

/// Collect blob IDs referenced by a snapshot's tree recursively.
fn list_snapshot_blobs(cas: &Cas, snapshot_id: &str) -> Result<Vec<String>> {
    let snap = Snapshot::load(cas, &snapshot_id.to_string())
        .map_err(|e| AgenticError::Sync(format!("load snapshot for blob listing: {e}")))?;
    let mut blob_ids = Vec::new();
    collect_tree_blobs(cas, &snap.root_tree, &mut blob_ids)?;
    Ok(blob_ids)
}

fn collect_tree_blobs(cas: &Cas, tree_id: &str, blob_ids: &mut Vec<String>) -> Result<()> {
    use zenon_cas::tree::{EntryType, Tree};

    if !blob_ids.iter().any(|id| id == tree_id) {
        blob_ids.push(tree_id.to_string());
    }

    let tree = Tree::load(&cas.blob_store, &tree_id.to_string())
        .map_err(|e| AgenticError::Sync(format!("load tree {tree_id}: {e}")))?;

    for entry in tree.entries {
        if !blob_ids.iter().any(|id| id == &entry.hash) {
            blob_ids.push(entry.hash.clone());
        }
        if matches!(entry.entry_type, EntryType::Tree) {
            collect_tree_blobs(cas, &entry.hash, blob_ids)?;
        }
    }

    Ok(())
}

fn maybe_restore_merged_vault(
    cas: &Cas,
    vault_path: &Path,
    merged_tree_id: &Option<String>,
    local_tree_id: &str,
) -> Result<()> {
    let Some(merged_tree_id) = merged_tree_id else {
        return Ok(());
    };
    if merged_tree_id == local_tree_id {
        return Ok(());
    }

    let snapshot_id = crate::protocol::synthetic_snapshot_id(merged_tree_id);
    persist_snapshot_reference(cas, &snapshot_id, merged_tree_id)?;
    restore(vault_path, cas, &snapshot_id)
        .map_err(|e| AgenticError::Sync(format!("restore merged tree: {e}")))?;
    Ok(())
}

fn synthetic_snapshot_id(root_tree_id: &str) -> String {
    zenon_cas::hash::hash_bytes(format!("sync-merged-{root_tree_id}").as_bytes())
}

fn persist_snapshot_reference(cas: &Cas, snapshot_id: &str, root_tree_id: &str) -> Result<()> {
    let snap_path = cas.snapshots_dir.join(format!("{snapshot_id}.json"));
    if snap_path.exists() {
        return Ok(());
    }

    let snapshot = Snapshot {
        id: snapshot_id.to_string(),
        root_tree: root_tree_id.to_string(),
        timestamp: Utc::now(),
        device_id: "remote".into(),
        message: Some("sync-import".into()),
    };
    let json = serde_json::to_vec(&snapshot)
        .map_err(|e| AgenticError::Sync(format!("serialize sync snapshot: {e}")))?;
    std::fs::write(snap_path, json)
        .map_err(|e| AgenticError::Sync(format!("persist sync snapshot: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::SyncMessage;

    #[test]
    fn sync_result_fields() {
        let r = SyncResult {
            merged: 5,
            auto_resolved: 2,
            conflicts: 1,
            snapshot_id: "abc".to_string(),
        };
        assert_eq!(r.merged, 5);
        assert_eq!(r.auto_resolved, 2);
        assert_eq!(r.conflicts, 1);
        assert_eq!(r.snapshot_id, "abc");
    }

    // Mock transport for protocol tests
    struct MockConnection {
        send_buf: Vec<SyncMessage>,
        recv_queue: std::collections::VecDeque<SyncMessage>,
    }

    impl MockConnection {
        fn new(recv_queue: Vec<SyncMessage>) -> Self {
            Self {
                send_buf: Vec::new(),
                recv_queue: recv_queue.into_iter().collect(),
            }
        }
    }

    #[async_trait::async_trait]
    impl SyncConnection for MockConnection {
        async fn send(&mut self, msg: &SyncMessage) -> Result<()> {
            self.send_buf.push(msg.clone());
            Ok(())
        }

        async fn recv(&mut self) -> Result<SyncMessage> {
            self.recv_queue
                .pop_front()
                .ok_or_else(|| AgenticError::Sync("mock recv queue empty".into()))
        }

        async fn send_blob(&mut self, id: &str, data: &[u8]) -> Result<()> {
            self.send(&SyncMessage::BlobBatch {
                blobs: vec![(id.to_string(), data.to_vec())],
            })
            .await
        }

        async fn recv_blob(&mut self) -> Result<(String, Vec<u8>)> {
            match self.recv().await? {
                SyncMessage::BlobBatch { mut blobs } => Ok(blobs.remove(0)),
                _ => Err(AgenticError::Sync("expected BlobBatch".into())),
            }
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn initiator_sends_sync_request_first() -> Result<()> {
        let dir = tempfile::TempDir::new().map_err(AgenticError::Io)?;
        let vault = dir.path();
        std::fs::create_dir_all(vault.join(".zenon"))?;
        let cas = Cas::open(vault)?;

        // Create a real local snapshot first
        let local_snap = Snapshot::create(vault, &cas, Some("test-pre".into()))?;
        // Use the same snapshot as "remote" so merge trivially succeeds (same tree)
        let remote_snap_id = local_snap.id.clone();

        // Responder queue: SyncResponse, BlobRequest (empty), BlobBatch (empty), SyncComplete
        let mut mock = MockConnection::new(vec![
            SyncMessage::SyncResponse {
                snapshot_id: remote_snap_id.clone(),
                root_tree_id: local_snap.root_tree.clone(),
            },
            SyncMessage::BlobRequest { ids: vec![] },
            SyncMessage::BlobBatch { blobs: vec![] },
            SyncMessage::SyncComplete {
                snapshot_id: remote_snap_id.clone(),
            },
        ]);

        let result =
            run_sync_initiator(&mut mock, &cas, vault, &ConflictPolicy::NewestWins).await?;

        // First message sent by initiator should be SyncRequest
        assert!(matches!(mock.send_buf[0], SyncMessage::SyncRequest { .. }));
        assert!(!result.snapshot_id.is_empty());
        Ok(())
    }
}
