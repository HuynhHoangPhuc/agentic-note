# Phase Implementation Report

## Executed Phase
- Phase: phase-07-p2p-sync-iroh
- Plan: /Users/phuc/Developer/agentic-note/plans/260213-2325-v02-features/
- Status: completed

## Files Modified

### crates/sync/src/
| File | Status | Notes |
|------|--------|-------|
| `identity.rs` | rewritten | switched from ed25519-dalek 2 to iroh::SecretKey; added tests |
| `device_registry.rs` | extended | added 6 unit tests |
| `transport.rs` | unchanged | scaffolded traits were already correct |
| `iroh_transport.rs` | created (new) | IrohTransport + IrohSyncConnection using iroh 0.96.1 API |
| `protocol.rs` | rewritten | full sync state machine (initiator + responder) with tests |
| `merge_driver.rs` | rewritten | delegates to cas::three_way_merge; write_conflict_files; tests |
| `lib.rs` | rewritten | SyncEngine facade with new_with_iroh, sync_with_peer, pair_device |

### crates/cli/src/
| File | Status | Notes |
|------|--------|-------|
| `commands/device.rs` | created (new) | device init/show/pair/list/unpair |
| `commands/sync_cmd.rs` | created (new) | sync now/status with policy parsing |
| `commands/mod.rs` | updated | +Device, +Sync variants in Commands enum |
| `main.rs` | updated | unified async match dispatch for all commands |
| `Cargo.toml` | updated | +agentic-note-sync dep |

## Tasks Completed

- [x] identity.rs — iroh::SecretKey, generate/load/save/init_or_load, 0600 perms
- [x] device_registry.rs — JSON CRUD with chrono timestamps, 6 tests
- [x] transport.rs — SyncTransport/SyncConnection traits + SyncMessage enum (pre-existing, unchanged)
- [x] iroh_transport.rs — IrohTransport::bind(SecretKey), connect(peer_id), listen(); ALPN `/agentic-note/sync/1`; length-prefix framing for messages
- [x] protocol.rs — run_sync_initiator + run_sync_responder state machines; find_common_ancestor heuristic; blob exchange; pre/post-sync snapshots
- [x] merge_driver.rs — merge_after_sync delegates to three_way_merge; write_conflict_files creates .agentic/conflicts/*.conflict
- [x] lib.rs — SyncEngine: new_with_iroh, new_with_transport, sync_with_peer, device_info, known_devices, pair_device
- [x] CLI: device init/show/pair/list/unpair
- [x] CLI: sync now --peer <ID> --policy <POLICY>, sync status
- [x] Commands enum + main.rs dispatch updated
- [x] CLI Cargo.toml +agentic-note-sync

## Tests Status
- Type check: **pass** (cargo check --workspace: 0 errors)
- Unit tests: **pass** — 16/16 passed
  - identity: 5 tests (generate, save/load roundtrip, init_or_load x2, key size)
  - device_registry: 6 tests (CRUD, no-duplicate, roundtrip, last_sync)
  - protocol: 2 tests (sync_result_fields, initiator_sends_sync_request_first)
  - merge_driver: 3 tests (empty vault, identical snapshots, conflict files dir)
- Integration tests (real iroh network): deferred — would require 2 endpoints binding to OS ports; appropriate for a separate integration test binary

## Key Implementation Decisions

1. **iroh::SecretKey instead of ed25519-dalek 2** — iroh 0.96 bundles ed25519-dalek 3.0.0-pre.1 internally which conflicts with workspace's dalek 2. Used iroh's own SecretKey (same 32-byte format, `From<[u8;32]>` impl) for identity. File format is identical (32 raw bytes).

2. **peer_id = base32 PublicKey string** — iroh's `PublicKey::to_string()` is base32, not hex. This is what `ep.connect(EndpointAddr::new(pk), ALPN)` requires.

3. **Length-prefix framing** — iroh QUIC streams are raw byte streams; JSON messages are framed with 4-byte big-endian length prefix. Max msg size 64 MiB.

4. **find_common_ancestor heuristic** — checks if remote/local snapshot IDs are known locally; falls back to oldest local snapshot; falls back to empty tree. Sufficient for initial implementation.

5. **anyhow::Result in CLI** — existing CLI commands return `anyhow::Result`; new device/sync commands follow same pattern for consistency.

6. **No tokio trait imports needed** — iroh-quinn's SendStream/RecvStream implement tokio::io::AsyncWrite/AsyncRead, and `write_all`/`read_exact` are resolved without explicit trait imports in scope (compiler finds them through the impl).

## Issues Encountered
- iroh `SecretKey::generate` takes rand_core 0.9 RngCore but workspace rand is 0.8 → fixed by generating raw bytes with `rand::RngCore::fill_bytes` and using `SecretKey::from([u8;32])`
- `Snapshot::load` signature uses `&ObjectId` (= `&String`) not `&str` → fixed with `.to_string()` calls
- `Tree::empty_id()` doesn't exist → fixed by serializing an empty Tree and storing it to get its ID
- `if let ... = cli.command` moved value before match → fixed by using single match for all commands

## Next Steps
- Phase 8: Integration tests using real iroh endpoints (two in-process endpoints)
- Document `device pair` workflow: how to exchange peer addresses out-of-band
- Consider adding `device addr` command to display local EndpointAddr for sharing

## Unresolved Questions
- Out-of-band peer address exchange: currently `sync now --peer <PEER_ID>` only passes the base32 public key. Without a relay or DNS address lookup, peers need to know each other's relay URL or direct IP. The EndpointAddr includes relay URL after `ep.online()`. A future `device addr` command should print the full EndpointAddr JSON for sharing. For LAN use, iroh's mDNS or direct IP address works.
