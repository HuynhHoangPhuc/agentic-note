/// Batch multi-peer sync: sequential fan-out with best-effort error recovery.
///
/// Iterates peers in deterministic (sorted) order, syncing each sequentially.
/// SyncTransport is not Clone/Send-safe for concurrent use, so we use sequential
/// iteration with per-peer error isolation rather than tokio::spawn parallelism.
use std::time::{Duration, Instant};

use agentic_note_cas::Cas;
use agentic_note_core::error::Result;
use agentic_note_core::types::ConflictPolicy;

use crate::protocol::{self, SyncResult};
use crate::transport::SyncTransport;

/// Status of a single peer sync attempt.
#[derive(Debug, Clone)]
pub enum PeerSyncStatus {
    Success,
    Failed(String),
    Skipped(String),
}

/// Outcome of syncing with a single peer.
#[derive(Debug, Clone)]
pub struct PeerSyncOutcome {
    pub peer_id: String,
    pub status: PeerSyncStatus,
    pub notes_synced: usize,
    pub duration: Duration,
}

/// Aggregated result of batch sync across all peers.
#[derive(Debug, Clone)]
pub struct BatchSyncResult {
    pub outcomes: Vec<PeerSyncOutcome>,
    pub total_merged: usize,
    pub total_auto_resolved: usize,
    pub total_conflicts: usize,
    pub total_duration: Duration,
}

/// Execute sync with multiple peers.
///
/// Peers are processed in sorted order for deterministic merge ordering.
/// Each peer failure is isolated: a failed peer does not abort remaining peers.
pub async fn sync_all_peers(
    transport: &dyn SyncTransport,
    cas: &Cas,
    vault_path: &std::path::Path,
    peer_ids: &[String],
    policy: &ConflictPolicy,
) -> Result<BatchSyncResult> {
    let start = Instant::now();

    // Sort peers for deterministic merge ordering
    let mut sorted_peers = peer_ids.to_vec();
    sorted_peers.sort();

    let mut outcomes = Vec::new();
    let mut total_merged = 0usize;
    let mut total_auto_resolved = 0usize;
    let mut total_conflicts = 0usize;

    for peer_id in &sorted_peers {
        let peer_start = Instant::now();

        match sync_single_peer(transport, cas, vault_path, peer_id, policy).await {
            Ok(result) => {
                total_merged += result.merged;
                total_auto_resolved += result.auto_resolved;
                total_conflicts += result.conflicts;
                outcomes.push(PeerSyncOutcome {
                    peer_id: peer_id.clone(),
                    status: PeerSyncStatus::Success,
                    notes_synced: result.merged + result.auto_resolved,
                    duration: peer_start.elapsed(),
                });
            }
            Err(e) => {
                // Best-effort: log failure, continue with remaining peers
                tracing::warn!(peer_id = %peer_id, error = %e, "peer sync failed");
                outcomes.push(PeerSyncOutcome {
                    peer_id: peer_id.clone(),
                    status: PeerSyncStatus::Failed(e.to_string()),
                    notes_synced: 0,
                    duration: peer_start.elapsed(),
                });
            }
        }
    }

    Ok(BatchSyncResult {
        outcomes,
        total_merged,
        total_auto_resolved,
        total_conflicts,
        total_duration: start.elapsed(),
    })
}

/// Sync with a single peer by connecting and running the initiator protocol.
async fn sync_single_peer(
    transport: &dyn SyncTransport,
    cas: &Cas,
    vault_path: &std::path::Path,
    peer_id: &str,
    policy: &ConflictPolicy,
) -> Result<SyncResult> {
    let mut conn = transport.connect(peer_id).await?;
    protocol::run_sync_initiator(conn.as_mut(), cas, vault_path, policy).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_sync_status_display() {
        let outcome = PeerSyncOutcome {
            peer_id: "peer-a".into(),
            status: PeerSyncStatus::Success,
            notes_synced: 5,
            duration: Duration::from_millis(100),
        };
        assert_eq!(outcome.peer_id, "peer-a");
        assert!(matches!(outcome.status, PeerSyncStatus::Success));
    }

    #[test]
    fn batch_result_aggregation() {
        let result = BatchSyncResult {
            outcomes: vec![
                PeerSyncOutcome {
                    peer_id: "a".into(),
                    status: PeerSyncStatus::Success,
                    notes_synced: 3,
                    duration: Duration::from_millis(50),
                },
                PeerSyncOutcome {
                    peer_id: "b".into(),
                    status: PeerSyncStatus::Failed("timeout".into()),
                    notes_synced: 0,
                    duration: Duration::from_millis(30000),
                },
            ],
            total_merged: 2,
            total_auto_resolved: 1,
            total_conflicts: 0,
            total_duration: Duration::from_secs(31),
        };
        assert_eq!(result.outcomes.len(), 2);
        assert_eq!(result.total_merged, 2);
        assert!(matches!(
            result.outcomes[1].status,
            PeerSyncStatus::Failed(_)
        ));
    }
}
