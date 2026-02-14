# Phase 7: P2P Sync via iroh

## Context Links
- [Research: iroh P2P](/Users/phuc/Developer/agentic-note/plans/260213-2325-v02-features/research/researcher-iroh-p2p-crdt.md)
- [Prior P2P design](/Users/phuc/Developer/agentic-note/plans/260213-1610-agentic-note-mvp/phase-06-p2p-sync.md)
- [CAS crate](/Users/phuc/Developer/agentic-note/crates/cas/src/lib.rs)
- [Phase 5: Conflict Policies](phase-05-conflict-auto-resolution.md)
- [Plan Overview](plan.md)

## Overview
- **Priority:** P2
- **Status:** completed
- **Effort:** 8h
- **Depends on:** Phase 1 (types/config), Phase 5 (conflict policies)
- **Description:** New `sync` crate with iroh networking, Ed25519 device identity, device registry, explicit sync protocol, post-sync merge with conflict policies.

## Key Insights
- iroh pre-1.0 — thin adapter trait essential for future updates
- iroh-blobs aligns with CAS (content-addressed) — transfer blobs directly
- iroh-docs deprecated — custom sync protocol on iroh connections
- Ed25519 keypair per device, stored in `.agentic/identity.key`
- Device registry: JSON file (`.agentic/devices.json`) — human-readable, inspectable
- iroh version: pin exact `=X.Y.Z` at implementation time (check crates.io)
- Entire sync crate behind `sync` cargo feature flag
<!-- Updated: Validation Session 1 - JSON device registry, exact iroh pin, feature-gated -->
- Explicit `sync now` (not continuous) — simpler, no background daemon
- Sync protocol: exchange snapshot IDs → diff → transfer missing blobs → merge trees

## Requirements

### Functional
- F1: Ed25519 identity per device (`device init`, `device show`)
- F2: Device registry: known peers stored in `.agentic/devices.json`
- F3: Device pairing: `device pair <peer-id>` — exchange public keys
- F4: `sync now` command: connect to peer, exchange snapshots, transfer blobs, merge
- F5: `sync status` command: show last sync time, peer info, pending conflicts
- F6: Thin iroh adapter trait — isolate iroh API behind `SyncTransport` trait
- F7: Post-sync merge uses `three_way_merge()` with configured ConflictPolicy
- F8: Sync creates pre-sync and post-sync CAS snapshots for safety

### Non-Functional
- Sync of 100 changed notes <30s on LAN
- Identity key file has 0600 permissions
- iroh relay fallback for NAT traversal
- Feature-gated behind `sync` cargo feature

## Architecture

```
crates/sync/
├── Cargo.toml
└── src/
    ├── lib.rs             # re-exports, SyncEngine facade
    ├── identity.rs        # Ed25519 keypair generation/storage
    ├── device_registry.rs # Known peers storage (JSON file)
    ├── transport.rs       # SyncTransport trait (thin iroh adapter)
    ├── iroh_transport.rs  # iroh implementation of SyncTransport
    ├── protocol.rs        # Sync protocol (exchange snapshots, diff, transfer)
    └── merge_driver.rs    # Post-sync merge orchestration
```

### Sync Protocol Flow
```
Device A initiates sync with Device B:

1. A connects to B via iroh (QUIC + relay fallback)
2. A sends: { type: "sync_request", snapshot_id: "abc123" }
3. B responds: { type: "sync_response", snapshot_id: "def456" }
4. Both compute: diff(common_ancestor, local_snapshot)
5. A sends missing blobs to B via iroh-blobs
6. B sends missing blobs to A via iroh-blobs
7. Both run three_way_merge(ancestor, local, remote, policy)
8. Both create post-sync snapshot
9. Exchange: { type: "sync_complete", snapshot_id: "new_id" }
```

### Transport Trait (Thin Adapter)
```rust
#[async_trait]
pub trait SyncTransport: Send + Sync {
    async fn connect(&self, peer_id: &str) -> Result<Box<dyn SyncConnection>>;
    async fn listen(&self) -> Result<Box<dyn SyncConnection>>;
    fn local_peer_id(&self) -> String;
}

#[async_trait]
pub trait SyncConnection: Send + Sync {
    async fn send(&mut self, msg: &SyncMessage) -> Result<()>;
    async fn recv(&mut self) -> Result<SyncMessage>;
    async fn send_blob(&mut self, id: &ObjectId, data: &[u8]) -> Result<()>;
    async fn recv_blob(&mut self) -> Result<(ObjectId, Vec<u8>)>;
    async fn close(&mut self) -> Result<()>;
}
```

## Related Code Files

| File | Action | Changes |
|------|--------|---------|
| `/Users/phuc/Developer/agentic-note/crates/sync/Cargo.toml` | modify | Add iroh, iroh-blobs, ed25519-dalek, agentic-note-cas deps |
| `/Users/phuc/Developer/agentic-note/crates/sync/src/lib.rs` | modify | Module declarations, SyncEngine facade |
| `/Users/phuc/Developer/agentic-note/crates/sync/src/identity.rs` | create | Ed25519 keypair management |
| `/Users/phuc/Developer/agentic-note/crates/sync/src/device_registry.rs` | create | Known peers JSON storage |
| `/Users/phuc/Developer/agentic-note/crates/sync/src/transport.rs` | create | SyncTransport + SyncConnection traits |
| `/Users/phuc/Developer/agentic-note/crates/sync/src/iroh_transport.rs` | create | iroh implementation |
| `/Users/phuc/Developer/agentic-note/crates/sync/src/protocol.rs` | create | Sync protocol state machine |
| `/Users/phuc/Developer/agentic-note/crates/sync/src/merge_driver.rs` | create | Post-sync merge orchestration |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/mod.rs` | modify | +Device, +Sync commands |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/device.rs` | create | device init/show/pair commands |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/sync.rs` | create | sync now/status commands |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/main.rs` | modify | +Device, +Sync dispatch |
| `/Users/phuc/Developer/agentic-note/crates/cli/Cargo.toml` | modify | +agentic-note-sync dep |

## Implementation Steps

1. Update `crates/sync/Cargo.toml`:
   ```toml
   [dependencies]
   agentic-note-core = { workspace = true }
   agentic-note-cas = { workspace = true }
   tokio = { workspace = true }
   serde = { workspace = true }
   serde_json = { workspace = true }
   tracing = { workspace = true }
   iroh = { workspace = true }
   iroh-blobs = { workspace = true }
   ed25519-dalek = { workspace = true }
   async-trait = { workspace = true }
   ```

2. Create `crates/sync/src/identity.rs`:
   - `DeviceIdentity` struct: `keypair: ed25519_dalek::SigningKey`, `peer_id: String`
   - `DeviceIdentity::generate() -> Self` — new random keypair
   - `DeviceIdentity::load(path: &Path) -> Result<Self>` — load from file
   - `DeviceIdentity::save(&self, path: &Path) -> Result<()>` — save with 0600 perms
   - `DeviceIdentity::init_or_load(agentic_dir: &Path) -> Result<Self>` — load if exists, generate if not
   - Peer ID derived from public key (hex-encoded)

3. Create `crates/sync/src/device_registry.rs`:
   - `DeviceRegistry` struct: `devices: Vec<KnownDevice>`, `path: PathBuf`
   - `KnownDevice`: `peer_id: String`, `name: Option<String>`, `last_sync: Option<DateTime<Utc>>`
   - `load(path: &Path) -> Result<Self>`, `save(&self) -> Result<()>`
   - `add_device(peer_id, name)`, `remove_device(peer_id)`, `list() -> &[KnownDevice]`

4. Create `crates/sync/src/transport.rs`:
   - Define `SyncTransport` and `SyncConnection` traits (see Architecture above)
   - Define `SyncMessage` enum:
     ```rust
     pub enum SyncMessage {
         SyncRequest { snapshot_id: String },
         SyncResponse { snapshot_id: String },
         BlobRequest { ids: Vec<ObjectId> },
         BlobBatch { blobs: Vec<(ObjectId, Vec<u8>)> },
         SyncComplete { snapshot_id: String },
         Error { message: String },
     }
     ```

5. Create `crates/sync/src/iroh_transport.rs`:
   - `IrohTransport` implementing `SyncTransport`
   - Uses `iroh::Endpoint` for QUIC connections
   - Uses `iroh_blobs` for efficient blob transfer
   - Wraps iroh connection in `IrohSyncConnection` implementing `SyncConnection`
   - All iroh-specific code isolated here — if iroh API changes, only this file changes

6. Create `crates/sync/src/protocol.rs`:
   - `SyncProtocol` struct with state machine:
     ```rust
     pub async fn sync(
         conn: &mut dyn SyncConnection,
         cas: &Cas,
         vault_path: &Path,
         policy: &ConflictPolicy,
     ) -> Result<SyncResult>
     ```
   - Steps:
     1. Create pre-sync snapshot
     2. Send SyncRequest with local snapshot ID
     3. Receive SyncResponse with remote snapshot ID
     4. Find common ancestor (walk snapshot chain)
     5. Compute diff local vs ancestor, remote vs ancestor
     6. Exchange missing blobs
     7. Run merge
     8. Create post-sync snapshot
   - `SyncResult`: `{ merged: usize, conflicts: Vec<ConflictInfo>, snapshot_id: String }`

7. Create `crates/sync/src/merge_driver.rs`:
   - `merge_after_sync(cas, local_snap, remote_snap, ancestor_snap, policy) -> Result<MergeResult>`
   - Delegates to `cas::three_way_merge()` with policy from Phase 5
   - For manual conflicts: create conflict files in `.agentic/conflicts/`
   - Return merge summary

8. Create `crates/sync/src/lib.rs`:
   - Module declarations
   - `SyncEngine` facade:
     ```rust
     pub struct SyncEngine {
         identity: DeviceIdentity,
         registry: DeviceRegistry,
         transport: Box<dyn SyncTransport>,
         cas: Cas,
     }
     impl SyncEngine {
         pub async fn sync_with_peer(&self, peer_id: &str, policy: &ConflictPolicy) -> Result<SyncResult>;
         pub fn device_info(&self) -> &DeviceIdentity;
         pub fn known_devices(&self) -> &[KnownDevice];
     }
     ```

9. Create CLI commands:
   - `crates/cli/src/commands/device.rs`: `device init`, `device show`, `device pair <PEER_ID>`
   - `crates/cli/src/commands/sync.rs`: `sync now [--peer <PEER_ID>]`, `sync status`
   - Update `Commands` enum and main.rs

10. Write tests:
    - Identity generation + save/load round-trip
    - Device registry CRUD
    - Protocol state machine with mock transport
    - Merge driver with each conflict policy
    - Integration: two in-memory transports sync a vault

## Todo List

- [x] Implement identity.rs (Ed25519 keypair)
- [x] Implement device_registry.rs (JSON peer storage)
- [x] Define SyncTransport/SyncConnection traits
- [x] Define SyncMessage enum
- [x] Implement iroh_transport.rs (iroh adapter)
- [x] Implement protocol.rs (sync state machine)
- [x] Implement merge_driver.rs (post-sync merge)
- [x] Implement SyncEngine facade
- [x] Create device init/show/pair CLI commands
- [x] Create sync now/status CLI commands
- [x] Update CLI Commands enum + dispatch
- [x] Tests: identity round-trip
- [x] Tests: device registry
- [x] Tests: protocol with mock transport
- [x] Tests: merge driver
- [ ] Tests: integration sync (real iroh - deferred, requires network)
- [x] cargo check + cargo test pass

## Success Criteria
- `device init` generates Ed25519 keypair, `device show` displays peer ID
- `sync now --peer <ID>` connects, exchanges blobs, merges with configured policy
- Conflicts resolved per ConflictPolicy from config
- Pre/post sync snapshots created
- `sync status` shows last sync time + pending conflicts

## Risk Assessment
- **High:** iroh API instability — thin adapter mitigates (only iroh_transport.rs changes)
- **High:** NAT traversal may fail — iroh relay as fallback, document LAN-first approach
- **Medium:** Common ancestor finding — may need snapshot chain/DAG, start with "latest common snapshot" heuristic
- **Medium:** Large vault sync — iroh-blobs handles streaming, but first sync of 5k notes may be slow

## Security Considerations
- Ed25519 keypair stored with 0600 permissions
- Peer authentication via public key (device pairing is explicit trust)
- All data in transit encrypted by iroh (QUIC TLS)
- No automatic sync — user explicitly initiates
- Device registry is local-only, not synced

## Next Steps
- Phase 8 (Integration) tests cross-feature scenarios with sync
