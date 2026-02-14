---
phase: 3
title: "Delta Sync Compression"
status: complete
effort: 2.5h
depends_on: [1]
---

## Context Links

- [Sync lib.rs](../../crates/sync/src/lib.rs)
- [Sync protocol.rs](../../crates/sync/src/protocol.rs)
- [Sync iroh_transport.rs](../../crates/sync/src/iroh_transport.rs)
- [CAS blob.rs](../../crates/cas/src/blob.rs)
- [Research: Delta Sync](research/researcher-sync-scheduling.md)

## Overview

<!-- Updated: Validation Session 1 - Remove fast-rsync, use zstd compression only -->
Replace full-snapshot transfer with `zstd` blob compression. CAS content hashing provides dedup (skip unchanged blobs). Reduces sync bandwidth by 60-80% for typical text notes. Simpler than rsync delta, zero abandoned deps.

## Key Insights

- `zstd` level 3 gives ~400MB/s compress with 3-5x ratio for markdown text
- CAS already hashes all blobs (SHA-256) -- skip unchanged blobs entirely
- Only changed blobs need transfer -- CAS diff identifies them
- No rsync delta needed (fast-rsync abandoned ~2021) -- KISS approach validated

## Requirements

**Functional:**
- Compress changed blobs with zstd before transmission
- Decompress blobs on receiving side
- Skip unchanged blobs (CAS hash match)
- Config: `sync.compression_enabled` (default true), `sync.compression_level` (default 3)

**Non-functional:**
- Delta computation < 10ms for typical note (< 100KB)
- Compression ratio > 5x for text diffs
- No data loss -- SHA-256 verification after apply

## Architecture

```
Sender:
  CAS diff(local_snap, common_base) -> changed blob IDs
  For each changed blob:
    zstd::compress(blob_data, level) -> compressed_blob
    Send compressed_blob + hash

Receiver:
  zstd::decompress(compressed_blob) -> blob_data
  Verify SHA-256(blob_data) == expected_hash
  Store in CAS BlobStore
```

## Related Code Files

**Create:**
- `crates/sync/src/compression.rs` -- zstd compress/decompress wrappers

**Modify:**
- `crates/sync/src/protocol.rs` -- add `CompressedBlob` payload variant
- `crates/sync/src/lib.rs` -- add `pub mod compression;`
- `crates/sync/Cargo.toml` -- add `zstd`
- Root `Cargo.toml` -- add workspace deps

## Implementation Steps

1. Add workspace dep to root `Cargo.toml`:
   ```toml
   zstd = "0.13"
   ```

2. Add to `crates/sync/Cargo.toml`:
   ```toml
   zstd = { workspace = true }
   ```

3. Create `crates/sync/src/compression.rs`:
   - `pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>>` -- wraps `zstd::encode_all`
   - `pub fn decompress(data: &[u8]) -> Result<Vec<u8>>` -- wraps `zstd::decode_all`

4. Add `CompressedBlob` to `crates/sync/src/protocol.rs`:
   ```rust
   pub enum SyncPayload {
       FullBlob { hash: ObjectId, data: Vec<u8> },
       CompressedBlob { hash: ObjectId, compressed_data: Vec<u8> },
   }
   ```

5. Modify sync initiator flow in `protocol.rs`:
   - After `diff_trees()` identifies changed blobs:
     a. If `compression_enabled`: compress blob data with zstd
     b. Send `CompressedBlob` payload
     c. Otherwise: send `FullBlob` payload

6. Modify sync responder flow:
   - On receiving `CompressedBlob`: decompress, verify SHA-256, store in CAS
   - On receiving `FullBlob`: store directly in CAS

7. Read `SyncConfig.compression_enabled` and `compression_level` from config.

8. Run `cargo check -p agentic-note-sync`.

9. Unit tests:
   - Test compression/decompression round-trip
   - Test SHA-256 verification after decompress
   - Test config toggle (compression enabled/disabled)

10. Integration test: two temp vaults, modify a note in one, sync with compression, verify content matches.

## Todo List

- [ ] Add workspace dep (zstd)
- [ ] Create `compression.rs` module
- [ ] Implement compress/decompress wrappers
- [ ] Add `CompressedBlob` to protocol
- [ ] Integrate compression into sync initiator flow
- [ ] Integrate decompression into sync responder flow
- [ ] Wire config (compression_enabled, compression_level)
- [ ] Unit tests for round-trip correctness
- [ ] Integration test: compressed sync between two vaults
- [ ] `cargo check` passes

## Success Criteria

- Compressed sync transfers 60-80% less data for typical markdown notes
- SHA-256 verification passes after decompress (no data corruption)
- Config toggle to disable compression
- Fallback to uncompressed when disabled

## Risk Assessment

- **zstd level tuning**: level 3 is good default; validated for text-heavy workloads
- **Memory**: zstd operates on full blob in memory; fine for notes (< 1MB)

## Security Considerations

- SHA-256 verification after delta apply prevents corruption
- No new network endpoints; reuses existing iroh QUIC transport
- Compressed data validated via hash before decompression (no zip bomb risk for notes)

## Next Steps

Phase 4 (Batch Multi-Peer Sync) builds on this delta transport.
