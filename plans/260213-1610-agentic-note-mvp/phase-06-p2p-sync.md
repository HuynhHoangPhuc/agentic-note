# Phase 06: P2P Sync — DEFERRED TO v2

<!-- Updated: Validation Session 1 - P2P sync deferred due to iroh API instability -->

## Context
- Parent: [plan.md](plan.md)
- Deps: Phase 05 (CAS & Versioning)
- Research: [Rust Crates API](research/researcher-rust-crates-api.md), [P2P Sync](../reports/researcher-260213-1538-obsidian-anytype-p2p-sync.md)

## Overview
- **Priority:** DEFERRED (iroh API unstable, ship local-only MVP first)
- **Status:** deferred
- **Effort:** 10h (when resumed)
- **Description:** iroh node setup, Ed25519 device keypair, device pairing, merkle diff sync protocol using CAS, sync CLI commands.

## Key Insights
- iroh 0.29: API changes every minor — pin exact version
- Use iroh blobs for content transfer (content-addressed, matches our CAS)
- Ed25519 keypair per device; account = set of authorized public keys
- iroh provides NAT traversal via relay servers (free but rate-limited from n0)
- `Node::persistent(path)` for durable node state; `Node::memory()` for tests
- Do NOT use iroh-docs (deprecated, migrating to Willow) — use blobs + custom sync protocol

## Requirements

**Functional:**
- Generate Ed25519 device keypair on first run
- Device pairing: exchange public keys between two devices
- Sync protocol: exchange merkle tree root → diff → transfer missing blobs
- Pull/push model: explicit `sync now` command (not continuous)
- List paired devices + their last sync timestamp
- `agentic-note device init` — generate keypair
- `agentic-note device show` — show device ID (public key)
- `agentic-note device pair <peer-id> <relay-url>` — add peer
- `agentic-note sync now` — sync with all paired devices
- `agentic-note sync status` — show sync state per device

**Non-functional:**
- Sync 100 changed files in < 10s on LAN
- Only transfer changed blobs (not full vault)
- Graceful handling of offline peers

## Architecture

```
crates/sync/src/
├── lib.rs          # pub mod re-exports, SyncEngine struct
├── identity.rs     # Ed25519 keypair gen/load/store
├── device.rs       # Device registry (paired devices, SQLite)
├── protocol.rs     # Sync protocol: handshake → diff → transfer
├── node.rs         # iroh node lifecycle (start/stop/connect)
└── merge.rs        # Post-sync merge orchestration (calls cas::merge)

.agentic/
├── identity.key    # Ed25519 signing key (0600 permissions)
├── devices.db      # SQLite: paired devices + sync state
└── iroh/           # iroh node persistent state
```

## Related Code Files

**Create:**
- `crates/sync/Cargo.toml` (update stub)
- `crates/sync/src/lib.rs`
- `crates/sync/src/identity.rs`
- `crates/sync/src/device.rs`
- `crates/sync/src/protocol.rs`
- `crates/sync/src/node.rs`
- `crates/sync/src/merge.rs`

**Modify:**
- `crates/cli/src/commands/mod.rs` — add Device, Sync subcommands
- `crates/cli/Cargo.toml` — add sync dep

## Cargo.toml Dependencies
```toml
[dependencies]
agentic-note-core = { path = "../core" }
agentic-note-cas = { path = "../cas" }
iroh = "0.29"
iroh-blobs = "0.29"
ed25519-dalek = { version = "2", features = ["rand_core"] }
rand = "0.8"
rusqlite = { version = "0.31", features = ["bundled"] }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
base64 = "0.22"
```

## Implementation Steps

1. **`identity.rs`:**
   - `Identity { signing_key: SigningKey, verifying_key: VerifyingKey }`
   - `Identity::generate() -> Self` — new Ed25519 keypair
   - `Identity::save(path: &Path) -> Result<()>` — write signing key bytes, set 0600 perms
   - `Identity::load(path: &Path) -> Result<Self>` — read key file
   - `Identity::device_id() -> String` — base64 of public key (display-friendly)
   - `Identity::sign(data: &[u8]) -> Signature`
   - `Identity::verify(data: &[u8], sig: &Signature, pubkey: &VerifyingKey) -> bool`

2. **`device.rs`:**
   - SQLite table: `devices(id TEXT PK, public_key BLOB, relay_url TEXT, last_sync TEXT, alias TEXT)`
   - `DeviceRegistry::add(id, pubkey, relay_url) -> Result<()>`
   - `DeviceRegistry::remove(id) -> Result<()>`
   - `DeviceRegistry::list() -> Result<Vec<PairedDevice>>`
   - `DeviceRegistry::update_last_sync(id, timestamp) -> Result<()>`

3. **`node.rs`:**
   - `SyncNode::start(data_dir: &Path) -> Result<Self>` — `Node::persistent(path).spawn().await`
   - `SyncNode::node_addr() -> NodeAddr` — for sharing with peers
   - `SyncNode::connect(peer: &NodeAddr) -> Result<Connection>`
   - `SyncNode::stop() -> Result<()>` — graceful shutdown
   - Store iroh state in `.agentic/iroh/`

4. **`protocol.rs`:** Sync protocol over iroh blobs
   - **Step 1:** Exchange latest snapshot hashes (signed with Ed25519)
   - **Step 2:** Receiver compares merkle trees via `cas::diff_trees`
   - **Step 3:** Sender transfers missing blobs via iroh-blobs
   - **Step 4:** Receiver stores blobs in local CAS
   - **Step 5:** Receiver runs `cas::three_way_merge` with common ancestor
   - **Step 6:** Both sides create new snapshot reflecting merged state
   - Message types: `SyncRequest { snapshot_hash, device_id, signature }`, `DiffResponse { needed_hashes: Vec<ObjectId> }`, `BlobTransfer { hash, data }`
   - Use iroh's built-in blob transfer for efficient data exchange

5. **`merge.rs`:**
   - `fn sync_merge(cas: &Cas, local_snap: &ObjectId, remote_snap: &ObjectId) -> Result<MergeResult>`
   - Find common ancestor (latest snapshot both sides have)
   - Call `cas::three_way_merge(ancestor, local, remote)`
   - Apply non-conflicting changes to vault
   - Write .conflict files for conflicts
   - Create new snapshot of merged state

6. **`lib.rs`:** `SyncEngine` facade
   - Holds SyncNode, DeviceRegistry, Cas reference, Identity
   - `SyncEngine::sync_all() -> Result<SyncReport>` — sync with all paired devices
   - `SyncEngine::sync_device(id: &str) -> Result<SyncReport>`
   - `SyncReport { synced_files: usize, conflicts: Vec<String>, duration: Duration }`

7. **CLI commands:** device init/show/pair, sync now/status

## Todo List
- [ ] Implement Ed25519 identity generation/storage
- [ ] Implement device registry (SQLite)
- [ ] Implement iroh node lifecycle
- [ ] Implement sync protocol (handshake/diff/transfer)
- [ ] Implement post-sync merge
- [ ] Add device/sync CLI commands
- [ ] Integration test: two nodes, create notes, sync, verify

## Success Criteria
- `device init` creates keypair, `device show` displays public key
- `device pair` adds remote device to registry
- `sync now` between two devices transfers new/changed notes
- Conflicts produce .conflict files
- Offline peer handled gracefully (timeout, skip, report)

## Risk Assessment
- **iroh API instability:** 0.29 may break — pin exact, wrap iroh calls in thin adapter layer
- **NAT traversal failures:** rely on iroh relay fallback; document self-hosted relay option
- **Common ancestor detection:** if devices diverge too far, ancestor may not exist — fall back to full transfer

## Security Considerations
- Ed25519 signing key stored with 0600 permissions
- All sync messages signed — reject unsigned/invalid signatures
- Device pairing requires explicit user action (no auto-discovery pairing)
- iroh transport is encrypted (QUIC + TLS)

## Next Steps
- Phase 09 (MCP) exposes sync tools
- Future: continuous sync mode, relay server, mobile sync
