---
phase: 4
title: "Batch Multi-Peer Sync"
status: complete
effort: 2h
depends_on: [3]
---

## Context Links

- [Sync lib.rs](../../crates/sync/src/lib.rs)
- [Sync device_registry.rs](../../crates/sync/src/device_registry.rs)
- [Sync protocol.rs](../../crates/sync/src/protocol.rs)
- [Research: Multi-Peer Coordination](research/researcher-sync-scheduling.md)

## Overview

Extend single-peer sync to concurrent multi-peer sync. Fan out to all registered peers simultaneously using `tokio::spawn`. Merge results sequentially using existing CAS three-way merge. Optionally use `iroh-gossip` for broadcast when peer count > 3.

## Key Insights

- Current `SyncEngine::sync_with_peer()` handles one peer -- wrap in concurrent executor
- Each peer sync is independent (different connection) -- safe to parallelize
- Merge ordering: apply peer results one-by-one using three-way merge against running local state
- `iroh-gossip` (HyParView + PlumTree) useful for > 3 peers; for <= 3, direct fan-out is simpler
- YAGNI: start with direct fan-out, add gossip later if needed

## Requirements

**Functional:**
- `SyncEngine::sync_all_peers(policy)` -- sync with all registered devices concurrently
- Per-peer result tracking: success/failure/skipped
- If any peer sync fails, continue with remaining peers (best-effort)
- Merge results sequentially to avoid concurrent CAS mutations
- CLI: `sync now --all` flag to trigger batch sync

**Non-functional:**
- Concurrent connections bounded by device count (typically 2-5)
- Total batch sync time ~ max(individual peer sync times) + merge overhead
- No deadlocks on CAS -- sequential merge after parallel fetch

## Architecture

```
sync_all_peers(policy)
  ├── snapshot local vault
  ├── for each registered peer (tokio::spawn):
  │     ├── connect via iroh
  │     ├── exchange snapshots (with delta compression from Phase 3)
  │     └── return PeerSyncResult { peer_id, remote_snapshot, diffs }
  ├── collect all PeerSyncResults (join_all)
  ├── for each result (sequential):
  │     ├── three_way_merge(local, remote, common_base, policy)
  │     └── apply merge to local vault
  │     └── update local snapshot
  └── return BatchSyncResult
```

## Related Code Files

**Create:**
- `crates/sync/src/batch_sync.rs` -- `BatchSyncExecutor` and `BatchSyncResult`

**Modify:**
- `crates/sync/src/lib.rs` -- add `pub mod batch_sync;`, add `sync_all_peers()` to `SyncEngine`
- `crates/cli/src/commands/sync_cmd.rs` -- add `--all` flag to `sync now`

## Implementation Steps

1. Create `crates/sync/src/batch_sync.rs`:
   - Define result types:
     ```rust
     pub struct PeerSyncOutcome {
         pub peer_id: String,
         pub status: PeerSyncStatus,
         pub notes_synced: usize,
         pub duration: Duration,
     }
     pub enum PeerSyncStatus { Success, Failed(String), Skipped(String) }
     pub struct BatchSyncResult {
         pub outcomes: Vec<PeerSyncOutcome>,
         pub merge_result: MergeResult,
         pub total_duration: Duration,
     }
     ```

2. Implement `sync_all_peers` on `SyncEngine`:
   ```rust
   pub async fn sync_all_peers(&mut self, policy: &ConflictPolicy) -> Result<BatchSyncResult>
   ```
   - Get all peers from `self.registry.list()`
   - Filter peers with `last_seen` (skip if never connected and no address)
   - Spawn a `tokio::spawn` task per peer calling existing `sync_with_peer()`
   - Use `futures::future::join_all` to await all tasks
   - Collect results; log failures, continue with successes
   - Sequential merge: for each successful peer result, call `three_way_merge()` on local state

3. Handle merge ordering:
   <!-- Updated: Validation Session 1 - Sort peers by peer_id for deterministic merge -->
   - Sort peers by `peer_id` alphabetically before sequential merge (deterministic)
   - After each peer merge, update local snapshot so next peer merges against latest state
   - Use `self.cas.create_snapshot()` after each merge application

4. Add `--all` flag to CLI sync command in `crates/cli/src/commands/sync_cmd.rs`:
   ```rust
   SyncCmd::Now {
       peer: Option<String>,
       policy: Option<String>,
       all: bool,  // NEW
   }
   ```

5. In CLI handler: if `all` flag, call `sync_all_peers()` instead of `sync_with_peer()`.

6. Display batch results: table of peer outcomes (peer_id, status, notes_synced, duration).

7. Run `cargo check -p agentic-note-sync -p agentic-note-cli`.

8. Unit test: mock transport with 3 peers, verify all contacted, merge applied sequentially.

9. Integration test: 3 temp vaults with different changes, batch sync, verify convergence.

## Todo List

- [ ] Create `batch_sync.rs` module
- [ ] Define `PeerSyncOutcome`, `BatchSyncResult` types
- [ ] Implement `sync_all_peers()` with concurrent fan-out
- [ ] Implement sequential merge of peer results
- [ ] Add `--all` flag to CLI sync command
- [ ] Wire CLI handler
- [ ] Display batch results table
- [ ] Unit test: multi-peer mock sync
- [ ] Integration test: 3-vault convergence
- [ ] `cargo check` passes

## Success Criteria

- All registered peers contacted concurrently
- Partial failures don't block other peers
- Final local state is consistent merge of all peer changes
- CLI output shows per-peer status

## Risk Assessment

- **Merge ordering**: different merge orders could yield different results for 3-way text merge. Mitigation: sort peers by peer_id (confirmed in validation)
- **Connection timeouts**: slow peers block batch completion. Mitigation: per-peer timeout (configurable, default 30s)
- **CAS snapshot overhead**: re-snapshotting after each peer merge adds latency. Acceptable for <= 5 peers

## Security Considerations

- Only registered (paired) peers are contacted
- Each connection uses iroh QUIC (TLS 1.3)
- No new trust model -- reuses existing device registry

## Next Steps

Phase 8 (Integration) tests batch sync with delta compression end-to-end.
