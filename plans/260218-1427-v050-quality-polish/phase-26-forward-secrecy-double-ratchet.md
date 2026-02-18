# Phase 26: Forward Secrecy — Double Ratchet Protocol

## Context Links

- [Research: Forward Secrecy](research/researcher-forward-secrecy-cicd.md)
- Current encryption: `crates/sync/src/encryption.rs` (static X25519 DH, no forward secrecy)
- v0.4.0 noted limitation: "static X25519 DH provides no forward secrecy"

## Overview

- **Priority:** P1
- **Status:** completed
- **Effort:** 3h
- **Description:** Replace static X25519 DH with Double Ratchet protocol for per-message forward secrecy and break-in recovery. Maintain backward compat with version-byte envelope.

## Key Insights

- `ksi-double-ratchet` is Signal-spec compliant, maintained, minimal surface area
- DR adds session state persistence (~30-100 bytes per peer) in SQLite
- Version byte in encrypted envelope enables gradual migration (0x01=legacy, 0x02=DR)
- X3DH handshake establishes root key; DR takes over for ongoing messages
- Existing `encrypt_note`/`decrypt_note` become the legacy path

## Requirements

**Functional:**
- X3DH initial handshake between peers establishes shared root key
- Double Ratchet for all subsequent messages (DH ratchet + symmetric chain)
- Session state persisted in SQLite (ratchet state per peer)
- Version byte prefix on every encrypted envelope
- Fallback to legacy static DH when peer doesn't support DR (0x01 envelope)

**Non-functional:**
- Encryption/decryption latency stays under 5ms per message
- Session state table < 1KB per peer
- Zero plaintext leakage on single key compromise

## Architecture

```
EncryptedEnvelope:
  [version: u8] [payload: ...]
  0x01 → legacy static X25519 → decrypt_note()
  0x02 → Double Ratchet       → dr_decrypt()

Session Management:
  SQLite table: dr_sessions (peer_id TEXT PK, state BLOB, updated_at TEXT)
  State = serialized ratchet (ksi-double-ratchet RatchetState)

Handshake Flow:
  1. Initiator sends X3DH prekey bundle
  2. Responder completes X3DH → shared root key
  3. Both initialize DR with root key
  4. All subsequent payloads use DR-derived keys
```

## Related Code Files

**Modify:**
- `crates/sync/src/encryption.rs` — add version byte, DR encrypt/decrypt, session mgmt
- `crates/sync/src/lib.rs` — re-export new types
- `crates/sync/src/protocol.rs` — add X3DH handshake messages
- `crates/sync/Cargo.toml` — add `ksi-double-ratchet` dep

**Create:**
- `crates/sync/src/double_ratchet.rs` — DR wrapper (~150 LOC): init, encrypt, decrypt, session load/save
- `crates/sync/src/session_store.rs` — SQLite session persistence (~80 LOC)

**No Delete.**

## Implementation Steps

1. Add `ksi-double-ratchet` to workspace `Cargo.toml` and `crates/sync/Cargo.toml`
2. Define `EncryptedEnvelope` struct with version byte prefix:
   ```rust
   pub struct EncryptedEnvelope {
       pub version: u8,        // 0x01 or 0x02
       pub payload: Vec<u8>,   // version-specific data
   }
   ```
3. Create `crates/sync/src/session_store.rs`:
   - `SessionStore::new(db_path)` — create `dr_sessions` table
   - `SessionStore::load(peer_id)` → `Option<RatchetState>`
   - `SessionStore::save(peer_id, state)` → `Result<()>`
   - `SessionStore::delete(peer_id)` → `Result<()>`
4. Create `crates/sync/src/double_ratchet.rs`:
   - `init_x3dh_initiator(my_identity, peer_prekey)` → `(RatchetState, X3DHHeader)`
   - `init_x3dh_responder(my_identity, header)` → `RatchetState`
   - `dr_encrypt(state, plaintext)` → `(EncryptedEnvelope, RatchetState)`
   - `dr_decrypt(state, envelope)` → `(Vec<u8>, RatchetState)`
5. Update `encryption.rs`:
   - Rename existing functions to `legacy_encrypt`/`legacy_decrypt` (private)
   - Add public `encrypt(version, ...)` dispatcher
   - Add public `decrypt(envelope)` dispatcher that reads version byte
6. Update `protocol.rs`: add `SyncMessage::X3DHInit` and `SyncMessage::X3DHResponse` variants
7. Update `lib.rs` to export new modules
8. Add tests: DR round-trip, legacy fallback, session persistence, wrong-version handling
9. Compile check: `cargo check -p agentic-note-sync`

## Todo List

- [x] Add ksi-double-ratchet dependency
- [x] Create session_store.rs with SQLite persistence
- [x] Create double_ratchet.rs wrapper
- [x] Add EncryptedEnvelope with version byte
- [x] Update encryption.rs to dispatch on version
- [x] Add X3DH handshake messages to protocol.rs
- [x] Update lib.rs exports
- [x] Write unit tests (DR round-trip, legacy fallback, session CRUD)
- [x] Compile check passes

## Success Criteria

- All existing encryption tests still pass (legacy path)
- New DR tests: handshake → encrypt → decrypt round-trip
- Session state persists across restarts (SQLite)
- Legacy peers (0x01) can still communicate with new code
- `cargo test -p agentic-note-sync` passes
- 0 compiler warnings

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| ksi-double-ratchet API breaking change | Low | Medium | Pin exact version, vendor if needed |
| Session state corruption | Low | High | SQLite transactions, test recovery |
| Backward compat regression | Medium | High | Explicit version byte check + legacy tests |

## Security Considerations

- DR provides per-message forward secrecy: compromise of current key doesn't reveal past messages
- Break-in recovery: new DH ratchet step re-establishes security after compromise
- Session state is sensitive (contains ratchet keys) — store in SQLite with 0600 perms
- X3DH prekey bundle should be signed with Ed25519 identity key to prevent MITM
- Wipe old ratchet states from memory after use (zeroize)

## Next Steps

- Phase 28 adds property-based tests for DR (message reordering, session recovery)
- Phase 31 audits all error paths including DR failures
