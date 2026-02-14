# Phase 23: End-to-End Encryption

## Context Links
- [plan.md](plan.md)
- [crates/sync/src/lib.rs](/Users/phuc/Developer/agentic-note/crates/sync/src/lib.rs) — SyncEngine
- [crates/sync/src/identity.rs](/Users/phuc/Developer/agentic-note/crates/sync/src/identity.rs) — Ed25519 identity
- [crates/sync/src/protocol.rs](/Users/phuc/Developer/agentic-note/crates/sync/src/protocol.rs) — sync messages
- [crates/core/src/config.rs](/Users/phuc/Developer/agentic-note/crates/core/src/config.rs) — SyncConfig

## Overview
- **Priority:** P1
- **Status:** Complete
- **Implementation Status:** complete
- **Review Status:** complete
- **Effort:** 3h
- **Description:** Optional E2EE for P2P sync. Per-note encryption using X25519 key exchange (derived from Ed25519 identity) + ChaCha20-Poly1305 AEAD. Selective encryption per-note during sync transfer.

## Key Insights
- Ed25519 signing keys already exist per device (`identity.rs`)
- Ed25519 -> X25519 conversion supported by `ed25519-dalek` (clamp scalar)
- ChaCha20-Poly1305 is AEAD: authenticates + encrypts in one pass
- Per-note encryption preferred: smaller blast radius, selective sync, parallelizable
- Nonce: 12 bytes, use counter or random (random simpler, collision-resistant with 96-bit nonce)
- iroh QUIC already provides transport encryption (TLS 1.3), E2EE is defense-in-depth

## Requirements

### Functional
- `EncryptionConfig` in SyncConfig: `enabled: bool`, `require_encryption: bool`
- Key derivation: Ed25519 SigningKey -> X25519 StaticSecret (one-time at startup)
- Per-peer shared secret via X25519 Diffie-Hellman
- Encrypt note content + frontmatter before sync transfer
- Decrypt on receive before merge
- Encrypted sync messages tagged with sender public key + nonce
- `require_encryption` rejects unencrypted peers

### Non-Functional
- <1ms encryption overhead per note (ChaCha20 is fast)
- Zero overhead when encryption disabled
- Forward secrecy via ephemeral keys per sync session (optional, phase 2)

## Architecture

```
Sender (Device A)                              Receiver (Device B)
    |                                              |
    +-- Load Ed25519 key                          +-- Load Ed25519 key
    +-- Derive X25519 secret                      +-- Derive X25519 secret
    +-- X25519 DH(A_secret, B_public) = shared    +-- X25519 DH(B_secret, A_public) = shared
    |                                              |
    +-- For each note:                             +-- For each note:
    |   +-- Generate random 12-byte nonce          |   +-- Read nonce from message
    |   +-- ChaCha20Poly1305.encrypt(shared, nonce)|   +-- ChaCha20Poly1305.decrypt(shared, nonce)
    |   +-- Send: EncryptedNote { nonce, ciphertext }   +-- Merge decrypted note
```

Key derivation (Ed25519 -> X25519):
```rust
fn ed25519_to_x25519(signing_key: &SigningKey) -> x25519_dalek::StaticSecret {
    let hash = Sha512::digest(signing_key.as_bytes());
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&hash[..32]);
    // Clamp per X25519 spec
    key_bytes[0] &= 248;
    key_bytes[31] &= 127;
    key_bytes[31] |= 64;
    StaticSecret::from(key_bytes)
}
```

## Related Code Files

### Modify
- `Cargo.toml` — add chacha20poly1305, x25519-dalek workspace deps
- `crates/sync/Cargo.toml` — add chacha20poly1305, x25519-dalek, sha2
- `crates/sync/src/identity.rs` — add X25519 key derivation method
- `crates/sync/src/protocol.rs` — add encrypted message variants
- `crates/sync/src/lib.rs` — integrate encryption in sync_with_peer
- `crates/core/src/config.rs` — add EncryptionConfig to SyncConfig
- `crates/core/src/error.rs` — add Encryption(String) variant

### Create
- `crates/sync/src/encryption.rs` — encrypt/decrypt functions, key exchange

## Implementation Steps

1. Add dependencies to workspace Cargo.toml:
   ```toml
   chacha20poly1305 = "0.10"
   x25519-dalek = { version = "2", features = ["static_secrets"] }
   ```
2. Add `Encryption(String)` error variant to `error.rs`
3. Add `EncryptionConfig` to `SyncConfig`:
   ```rust
   pub struct EncryptionConfig {
       pub enabled: bool,           // default false
       pub require_encryption: bool, // reject unencrypted peers, default false
   }
   ```
4. Create `encryption.rs`:
   - `derive_x25519_secret(signing_key: &SigningKey) -> StaticSecret`
   - `derive_shared_secret(my_secret: &StaticSecret, peer_public: &PublicKey) -> [u8; 32]`
   - `encrypt_note(shared_key: &[u8; 32], plaintext: &[u8]) -> Result<EncryptedPayload>`
   - `decrypt_note(shared_key: &[u8; 32], payload: &EncryptedPayload) -> Result<Vec<u8>>`
   - `EncryptedPayload { nonce: [u8; 12], ciphertext: Vec<u8> }`
5. Extend `DeviceIdentity` with `x25519_public_key()` method
6. Extend sync protocol messages:
   - `SyncMessage::EncryptedNote { sender_pubkey, payload: EncryptedPayload }`
   - Negotiate encryption support in handshake
7. Integrate in `SyncEngine::sync_with_peer`:
   - If encryption enabled: derive shared secret from peer's X25519 public key
   - Encrypt each note blob before sending
   - Decrypt on receive before passing to merge driver
8. Handle `require_encryption` rejection in handshake
9. Add tests: encrypt/decrypt round-trip, key derivation determinism, reject unencrypted

## Todo List
- [x]Add chacha20poly1305 + x25519-dalek deps
- [x]Add Encryption error variant
- [x]Add EncryptionConfig to SyncConfig
- [x]Create encryption.rs with encrypt/decrypt
- [x]Add X25519 key derivation to identity.rs
- [x]Extend protocol messages
- [x]Integrate in SyncEngine
- [x]Handle require_encryption rejection
- [x]Add unit tests
- [x]Add integration test (two-device round-trip)

## Success Criteria
- encrypt -> decrypt round-trip produces original content
- Key derivation is deterministic (same Ed25519 key -> same X25519 key)
- Encrypted sync between two devices succeeds
- `require_encryption = true` rejects unencrypted peer
- Disabled by default (zero overhead when off)
- Existing unencrypted sync tests still pass

## Risk Assessment
- **Key management**: X25519 derived from Ed25519; if signing key leaked, encryption compromised. Mitigate: same threat model as current identity.
- **Nonce reuse**: Random nonce has negligible collision probability with 96-bit space. Safe for per-note encryption.
- **No forward secrecy**: Static DH means compromise of long-term key decrypts past sessions. Mitigate: document as known limitation, plan ephemeral keys in v0.5.

## Security Considerations
- AEAD (ChaCha20-Poly1305) provides authenticity + confidentiality
- Per-note encryption limits blast radius
- X25519 key never stored separately (derived from Ed25519 at runtime)
- Nonce included in ciphertext (not secret, but must be unique)
- Transport already encrypted (iroh QUIC TLS 1.3); E2EE is defense-in-depth
- **Known Limitation:** No forward secrecy — static X25519 DH means compromise of long-term Ed25519 key allows decryption of all past sync sessions. Ephemeral session keys planned for v0.5.
<!-- Updated: Validation Session 1 - No forward secrecy accepted, documented as known limitation -->

## Next Steps
- Depends on existing Ed25519 identity (already in crate)
- Future: ephemeral session keys for forward secrecy (v0.5)
- Future: key rotation protocol
