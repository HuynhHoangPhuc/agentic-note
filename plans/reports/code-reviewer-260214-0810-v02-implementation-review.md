# Code Review: v0.2.0 Implementation

**Reviewer:** code-reviewer
**Date:** 2026-02-14
**Scope:** All 8 workspace crates, v0.2.0 additions

---

## Scope

- **Files reviewed:** 40+ Rust source files across 8 crates (core, vault, cas, search, agent, review, sync, cli)
- **LOC:** ~8,486 total
- **Tests:** 71 test functions across 23 test modules
- **Focus:** New v0.2.0 code -- DAG executor, error policies, plugin system, CAS merge, P2P sync, MCP handlers, device management

---

## Overall Assessment

Solid foundational implementation. Clean architecture with good separation of concerns across crates. Error handling is consistent (thiserror + per-crate error variants). Test coverage is meaningful, not just superficial. Several security and correctness issues need attention before production use, particularly in the sync protocol and plugin system.

---

## Critical Issues

### C1. Plugin subprocess execution lacks sandboxing
**File:** `/Users/phuc/Developer/agentic-note/crates/agent/src/plugin/runner.rs`
**Lines:** 55-61

The `PluginAgent` spawns arbitrary executables with no sandboxing, PATH restrictions, or capability limitations. The plugin inherits the parent process's environment variables (including potential secrets like API keys from `ProviderConfig`).

**Impact:** A malicious or compromised plugin gets full user privileges.

**Recommendation:**
```rust
let mut child = Command::new(&exe_path)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .current_dir(&self.plugin_dir)
    .env_clear()                          // Clear inherited env
    .env("PATH", "/usr/bin:/usr/local/bin") // Minimal PATH
    .env("HOME", &self.plugin_dir)          // Restrict home
    .spawn()
```
At minimum, call `.env_clear()` before spawn to prevent leaking `api_key` values from `ProviderConfig`.

### C2. API key stored in plaintext config
**File:** `/Users/phuc/Developer/agentic-note/crates/core/src/config.rs`
**Line:** 56

`ProviderConfig.api_key` is a plain `String` deserialized from TOML. The config file `.agentic/config.toml` is not excluded from CAS snapshots by default, meaning API keys could be synced to peer devices or stored in CAS blob objects.

**Recommendation:**
- Support env var references: `api_key = "$OPENAI_API_KEY"` with runtime resolution
- Exclude `.agentic/config.toml` from snapshot tree building
- Add `Debug` impl for `ProviderConfig` that redacts the key

### C3. Sync protocol: no authentication of peer identity
**File:** `/Users/phuc/Developer/agentic-note/crates/sync/src/protocol.rs`
**Lines:** 39-173

The initiator connects to a peer and immediately trusts whatever snapshot ID the peer sends back. There is no verification that the peer is who they claim to be beyond the TLS identity from iroh. The `find_common_ancestor` function has flawed logic (see H1 below). A rogue peer could inject arbitrary blobs.

**Recommendation:** Add a challenge-response exchange after the TLS handshake to verify the peer is in the local `DeviceRegistry`. Reject connections from unknown peers.

---

## High Priority

### H1. `find_common_ancestor` logic is incorrect
**File:** `/Users/phuc/Developer/agentic-note/crates/sync/src/protocol.rs`
**Lines:** 294-320

The function attempts to find a common ancestor but the logic is broken:
- It first checks if `remote_id` exists locally -- if so, it returns `remote_id` as ancestor. But this means the remote's current snapshot IS the ancestor, which is only true if local has advanced and remote hasn't.
- Then it checks if `local_id` loads -- which it always will since we just created it. So it returns `local_id` as ancestor, which is wrong (it's the current local state, not an ancestor).
- Fallback to `snapshots.last()` (oldest) is a rough heuristic.

**Impact:** Incorrect ancestor selection causes incorrect merge results -- files could be duplicated, lost, or falsely flagged as conflicts.

**Recommendation:** Implement proper snapshot parentage tracking. Each snapshot should record its parent ID. Walk both chains to find the first common ancestor (LCA algorithm).

### H2. Embedding tokenization is fake
**File:** `/Users/phuc/Developer/agentic-note/crates/search/src/embedding.rs`
**Lines:** 34-43

The "tokenization" maps words to sequential IDs starting at 1000, using CLS=101 and SEP=102. This is NOT a valid tokenizer for any ONNX embedding model. Real models (all-MiniLM-L6-v2) use WordPiece/SentencePiece tokenizers with a specific vocabulary. The embeddings produced will be meaningless.

**Impact:** Semantic search returns garbage results. Cosine similarity scores will be noise.

**Recommendation:** Integrate `tokenizers` crate (HuggingFace) or ship the tokenizer JSON alongside the ONNX model. Use the real vocabulary for tokenization.

### H3. `outputs_snapshot` race in DAG parallel execution
**File:** `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/dag_executor.rs`
**Lines:** 140-146

Each parallel stage in a layer receives a snapshot of outputs taken BEFORE any stage in that layer runs. This is correct for read isolation. However, the `ctx_clone` is independent per task, so any mutations a handler makes to `ctx` (like writing to `note_content`) are silently lost. Only the returned `Value` is captured.

**Impact:** If a handler mutates `StageContext` fields beyond `outputs`, those mutations are discarded in DAG mode (but work in sequential mode). This creates a subtle behavior difference between v1/v2 pipelines.

**Recommendation:** Document this constraint clearly. Consider making `StageContext` fields other than `outputs` read-only (wrap in `Arc`).

### H4. Condition evaluator only supports single-level field access
**File:** `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/condition.rs`
**Lines:** 33-37

`split_once('.')` means conditions can only access `output_key.field`. Nested fields like `output_key.nested.field` silently fail (treats `nested.field` as a single key name, which won't match).

**Impact:** Users writing conditions for nested JSON outputs get silently incorrect evaluations (always false, causing unintended skips).

**Recommendation:** Support dotted path traversal:
```rust
fn resolve_json_path(value: &Value, path: &str) -> Option<&Value> {
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}
```

### H5. `iroh_transport.rs` recv has no timeout
**File:** `/Users/phuc/Developer/agentic-note/crates/sync/src/iroh_transport.rs`
**Lines:** 122-146

The `recv()` method reads from the QUIC stream without a timeout. A misbehaving peer that sends the 4-byte length prefix but never sends the body would block the receiver indefinitely.

**Recommendation:** Wrap `read_exact` calls with `tokio::time::timeout`.

### H6. 64 MiB max message size for sync is excessive
**File:** `/Users/phuc/Developer/agentic-note/crates/sync/src/iroh_transport.rs`
**Line:** 132

`MAX_MSG = 64 * 1024 * 1024` for a note-taking app is very large. An attacker could force the receiver to allocate 64 MiB per message with a crafted length prefix.

**Recommendation:** Reduce to a more reasonable limit (e.g., 4 MiB) or implement streaming for large blob transfers.

---

## Medium Priority

### M1. Duplicate `AutoResolution` struct definition
**Files:**
- `/Users/phuc/Developer/agentic-note/crates/cas/src/conflict_policy.rs` (lines 22-28)
- `/Users/phuc/Developer/agentic-note/crates/cas/src/merge.rs` (lines 18-24)

`AutoResolution` is defined identically in both files. The one in `merge.rs` is used in `MergeResult`; the one in `conflict_policy.rs` is unused.

**Recommendation:** Remove the duplicate from `conflict_policy.rs` and re-export from `merge.rs`.

### M2. BlobStore temp directory collision in tests
**File:** `/Users/phuc/Developer/agentic-note/crates/cas/src/blob.rs`
**Lines:** 57-59

`temp_store()` uses only `std::process::id()` for uniqueness. If multiple test processes run with the same PID (container recycling), tests could interfere. The `conflict_policy.rs` tests already fixed this by adding `subsec_nanos()`.

**Recommendation:** Use `tempfile::TempDir` consistently across all test helpers to guarantee isolation.

### M3. `resolve_note_path` prefix matching is too loose
**File:** `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/handlers.rs`
**Lines:** 217-229

`name.starts_with(target)` means a target of "a" matches "anything.md". A ULID prefix search should require a minimum length to avoid accidental matches.

**Recommendation:** Require at least 4+ characters for prefix matching, or only match exact ULID strings (26 chars).

### M4. `DeviceRegistry.path` is `#[serde(skip)]` but required for `save()`
**File:** `/Users/phuc/Developer/agentic-note/crates/sync/src/device_registry.rs`
**Lines:** 18-20, 41-46

If someone deserializes a `DeviceRegistry` from JSON (not via `load()`), the `path` field defaults to empty, and `save()` would attempt to write to an empty path.

**Recommendation:** Make `save()` take `&Path` as parameter, or make `path` non-optional with proper validation.

### M5. Error policy comparison uses `Default` check which can misfire
**File:** `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/dag_executor.rs`
**Line:** 135

```rust
if stage.on_error == ErrorPolicy::default() {
    stage.on_error = cfg.default_on_error.clone();
}
```

This means if a user explicitly sets `on_error = "skip"` (which IS the default), the pipeline default overrides it. The intent ("use pipeline default if stage doesn't specify") cannot be distinguished from "stage explicitly chose Skip".

**Recommendation:** Use `Option<ErrorPolicy>` for `StageConfig.on_error` so `None` means "use pipeline default" and `Some(Skip)` means "explicitly skip."

### M6. Three-way merge does not write merged content to disk
**File:** `/Users/phuc/Developer/agentic-note/crates/cas/src/merge.rs`

The `three_way_merge` function computes what should happen but never applies the merge to the working directory. The `applied` list tracks paths but doesn't actually update files. The `write_conflict_files` in `merge_driver.rs` only handles conflicts, not the successfully merged files.

**Recommendation:** Implement a `apply_merge_to_vault` function that restores merged blob content to the vault directory.

---

## Low Priority

### L1. `ObjectId` is a type alias, not a newtype
**File:** `/Users/phuc/Developer/agentic-note/crates/cas/src/hash.rs`
**Line:** 7

`pub type ObjectId = String` provides no type safety. Any `String` is accepted where an `ObjectId` is expected.

### L2. `NoteId::new()` has side effects (monotonic clock)
**File:** `/Users/phuc/Developer/agentic-note/crates/core/src/types.rs`
**Line:** 15-17

`Ulid::new()` reads the system clock. The `Default` impl delegates to `new()`. This is fine but worth noting: `Default::default()` is not pure.

### L3. Conflict markers use non-standard format
**File:** `/Users/phuc/Developer/agentic-note/crates/cas/src/conflict_policy.rs`
**Lines:** 125-128

Using `<<<< LOCAL` / `>>>> REMOTE` instead of standard Git markers (`<<<<<<< LOCAL` / `>>>>>>> REMOTE` with 7 chars). Tools that parse Git-style conflict markers won't recognize these.

### L4. `parse_para` duplicated between CLI handlers and could use `FromStr` impl
**File:** `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/handlers.rs`
**Lines:** 183-192

Add `FromStr` impl on `ParaCategory` in core to avoid manual match arms in multiple places.

---

## Edge Cases Found

1. **Empty note body with embeddings:** `generate_embedding("")` returns a zero vector, which has undefined cosine similarity with any other vector. The cosine function returns 0.0 due to the norm check, but this means empty notes have no similarity to anything -- correct but should be documented.

2. **Concurrent snapshot creation during sync:** If a user edits a note while `run_sync_initiator` is between pre-sync snapshot (step 1) and post-sync snapshot (step 7), the edit could be captured in the post-sync snapshot but not in the merge, leading to data appearing "from nowhere" after sync.

3. **Plugin timeout race:** In `runner.rs` line 64-70, stdin is written before the timeout starts (at line 76). A plugin that blocks on reading stdin (because it expects more data than was written) would hang the write, which is outside the timeout window.

4. **`merge_both` with binary content:** `String::from_utf8_lossy` replaces invalid UTF-8 with replacement characters. If both blob versions contain binary data, the merged output with conflict markers corrupts the content.

5. **Device ID generation in CAS is weak:** `cas.rs` line 41-50 uses hostname + nanoseconds for device ID. Multiple containers starting simultaneously on the same host could collide. Use a proper UUID or random bytes.

---

## Positive Observations

- Clean workspace organization with well-defined crate boundaries
- Consistent error handling via `thiserror` with domain-specific variants
- Good use of `async_trait` for pluggable agent handlers
- DAG executor with topological sort and parallel layer execution is well-designed
- Comprehensive test coverage for error policies (skip, retry, abort, fallback)
- Identity key file gets proper 0o600 permissions on Unix
- Length-prefix framing for sync protocol with max size check
- V1-to-V2 pipeline migration for backwards compatibility
- Good separation of transport trait from iroh implementation

---

## Recommended Actions (Priority Order)

1. **[Critical]** Clear environment variables in plugin subprocess spawning (C1)
2. **[Critical]** Support env var references for API keys in config, exclude config from CAS snapshots (C2)
3. **[High]** Fix `find_common_ancestor` with proper snapshot parent tracking (H1)
4. **[High]** Replace fake tokenizer with real WordPiece tokenizer (H2)
5. **[High]** Add recv timeout in iroh transport (H5)
6. **[High]** Use `Option<ErrorPolicy>` to distinguish explicit vs default (M5)
7. **[Medium]** Remove duplicate `AutoResolution` struct (M1)
8. **[Medium]** Implement vault file application after merge (M6)
9. **[Medium]** Tighten `resolve_note_path` prefix matching (M3)
10. **[Low]** Standardize conflict markers to Git format (L3)

---

## Metrics

| Metric | Value |
|--------|-------|
| Total LOC | ~8,486 |
| Test functions | 71 |
| Files with tests | 23/40+ |
| Crates | 8 |
| Critical issues | 3 |
| High issues | 6 |
| Medium issues | 6 |
| Low issues | 4 |

---

## Unresolved Questions

1. Is the CAS snapshot tree supposed to include `.agentic/` directory contents? If yes, API keys and identity keys would be snapshotted.
2. Is there an intended mechanism for plugins to declare required capabilities or permissions?
3. Should the sync protocol support partial/incremental sync, or is full-snapshot exchange the intended design?
4. What happens when `DeviceCmd::Pair` receives a peer_id that is already in the registry but with a different name? Currently it silently no-ops (doesn't update the name).
