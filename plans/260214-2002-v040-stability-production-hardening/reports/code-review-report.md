# Code Review Report — v0.4.0 Stability & Production Hardening

**Date:** 2026-02-14
**Reviewer:** code-reviewer agent
**Scope:** 6 new subsystems (phases 20–25), unstaged changes only (`git diff HEAD`)
**Score: 7.5 / 10**

---

## Scope

| File | Phase | LOC (approx.) |
|---|---|---|
| `crates/core/src/storage.rs` | 20 – Storage abstraction | 38 |
| `crates/core/src/storage_sqlite.rs` | 20 – SQLite backend | 167 |
| `crates/core/src/storage_postgres.rs` | 20 – Postgres backend (feature-gated) | 121 |
| `crates/agent/src/llm/cache.rs` | 21 – LLM cache | 165 |
| `crates/agent/src/llm/batch_collector.rs` | 21 – Batch collector | 229 |
| `crates/cli/src/metrics_handle.rs` | 22 – Prometheus handle | 165 |
| `crates/cli/src/metrics_init.rs` | 22 – HTTP metrics server | 86 |
| `crates/cli/src/commands/metrics_cmd.rs` | 22 – CLI metrics command | 37 |
| `crates/sync/src/encryption.rs` | 23 – E2E encryption | 146 |
| `crates/sync/src/identity.rs` | 23 – Identity (x25519 addition) | +10 lines |
| `crates/sync/src/transport.rs` | 23 – Transport messages | +15 lines |
| `crates/sync/src/vault_registry.rs` | 24 – Multi-vault registry | 202 |
| `crates/sync/src/lib.rs` | 24 – sync_all_vaults | +56 lines |
| `crates/cli/src/commands/sync_cmd.rs` | 24 – AllVaults command | +32 lines |
| `crates/cli/src/commands/vault_registry_cmd.rs` | 24 – VaultRegistry CLI | 119 |
| `crates/agent/src/plugin/wasm_host.rs` | 25 – WASM host imports | 51 |
| `crates/agent/src/plugin/wasm_runner.rs` | 25 – WASM runner | 147 |
| `crates/agent/src/plugin/runner.rs` | 25 – Dispatch runner | +55 lines |
| `crates/agent/src/plugin/manifest.rs` | 25 – Manifest (runtime field) | +60 lines |
| `crates/core/src/config.rs` | 20/21/22/23/24/25 – Config structs | +133 lines |
| `crates/agent/src/llm/mod.rs` | 21 – batch_chat default | +11 lines |
| Config / Cargo changes | All phases | varies |

**Total new LOC reviewed:** ~1,820

---

## Overall Assessment

The release is architecturally sound and demonstrates disciplined incremental design. All six subsystems follow established patterns, include unit tests, and handle the common error paths. Code quality is uniformly readable.

The main concerns are:

1. A **medium-severity security gap** in the encryption layer (no KDF over the raw DH output).
2. **Memory-limit enforcement is silently dropped** in the WASM runner.
3. A **Mutex-in-async-context contention** pattern repeated across storage and LLM cache.
4. The **`_memory_limit_mb` parameter** in `wasm_runner.rs::execute` is accepted but entirely ignored.
5. **No integration wiring** – new subsystems have no call-sites in the main binary; features are unreachable at runtime without additional plumbing.

---

## Critical Issues

None.

---

## High Priority

### H1 — Raw DH output used directly as encryption key (no KDF)

**File:** `crates/sync/src/encryption.rs`, lines 39–42

```rust
pub fn derive_shared_secret(my_secret: &StaticSecret, peer_public: &PublicKey) -> [u8; 32] {
    let shared = my_secret.diffie_hellman(peer_public);
    *shared.as_bytes()   // ← raw X25519 output, not a proper key
}
```

The raw X25519 output has low-order bit ambiguity and must not be used directly as a symmetric cipher key. The standard practice is to pass it through HKDF-SHA256 before giving it to ChaCha20-Poly1305. Using the raw bytes can lead to distinguishability attacks and violates best-practice DH hygiene.

**Recommended fix:**

```rust
use hkdf::Hkdf;
use sha2::Sha256;

pub fn derive_shared_secret(my_secret: &StaticSecret, peer_public: &PublicKey) -> [u8; 32] {
    let dh = my_secret.diffie_hellman(peer_public);
    let hk = Hkdf::<Sha256>::new(None, dh.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"agentic-note-sync-v1", &mut okm).expect("32 bytes is valid HKDF output");
    okm
}
```

Add `hkdf = "0.12"` to the sync crate's `Cargo.toml`.

---

### H2 — WASM memory limit silently ignored

**File:** `crates/agent/src/plugin/wasm_runner.rs`, line 58

```rust
pub fn execute(
    ...
    _memory_limit_mb: Option<u32>,   // ← underscore prefix confirms it is unused
) -> Result<Value> {
```

The `_memory_limit_mb` parameter is accepted through the call chain (manifest → `runner.rs` line 82) but `WasmPluginRunner` stores `default_memory_limit_mb` and never applies it to the wasmtime `Store`. Wasmtime supports memory limits via `StoreLimits` / `ResourceLimiter`. Without this a malicious or buggy WASM plugin can allocate unlimited host memory.

**Recommended fix:**

```rust
use wasmtime::{ResourceLimiter, Store};

struct MemoryLimiter { limit_bytes: usize }

impl ResourceLimiter for MemoryLimiter {
    fn memory_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> anyhow::Result<bool> {
        Ok(desired <= self.limit_bytes)
    }
    fn table_growing(&mut self, _cur: u32, _des: u32, _max: Option<u32>) -> anyhow::Result<bool> { Ok(true) }
}
```

Then create `store.limiter(|state| &mut state.limiter)` after adding `limiter` to `HostState`.

---

### H3 — `Mutex<rusqlite::Connection>` held across `spawn_blocking` boundary

**Files:** `crates/core/src/storage_sqlite.rs` (lines 50–64, 72–103, 115–127), `crates/agent/src/llm/cache.rs` (lines 50–72, 77–90, 96–111)

The pattern is:

```rust
let conn = self.conn.clone();  // Arc<Mutex<Connection>>
tokio::task::spawn_blocking(move || {
    let conn = conn.lock()...;  // lock acquired inside blocking thread
    // long DB operation
})
```

While technically sound (the lock is held only in the blocking thread), callers using `Arc<SqliteBackend>` from many async tasks will serialize all DB operations behind one Mutex. Under concurrent load this becomes a single-threaded bottleneck. For SQLite this is expected, but the `LlmCache` Mutex could also cause contention since `put` is called after each batch request.

**Recommendation:** Document the single-writer SQLite constraint explicitly in the struct docstrings. For `LlmCache`, consider using `rusqlite`'s `Connection::execute_batch` WAL mode (`PRAGMA journal_mode=WAL`) to improve read concurrency, and add this as a follow-up issue.

---

## Medium Priority

### M1 — `convert_placeholders` in `storage_postgres.rs` is fragile

**File:** `crates/core/src/storage_postgres.rs`, lines 90–105

The function converts `?1` → `$1` by stripping the `?` when followed by a digit. It does not handle:

- `?` inside a string literal (e.g. `WHERE name = 'what?1'`)
- Multi-digit placeholders `?10`, `?11` — the `?` is replaced but the digit string remains, yielding `$10` correctly by accident only because digits are left in place.
- Named parameters or `?` without a number (bare `?` — SQLite supports this)

This is acceptable for internal use where all callers control SQL strings, but it should be clearly documented as "only handle `?N` positional parameters in non-string contexts" and an assertion or test for edge cases added.

---

### M2 — `PluginAgent` holds an `Option<Arc<Mutex<WasmPluginRunner>>>` per agent instance

**File:** `crates/agent/src/plugin/runner.rs`, lines 19–35

`WasmPluginRunner` owns a `wasmtime::Engine` (heavyweight — compiles LLVM JIT code on construction). Creating one per `PluginAgent` instance defeats the intent of module caching and wastes significant memory and startup time when multiple WASM plugins are registered.

The `Engine` should be a shared singleton or be provided externally (e.g. via `Arc<WasmPluginRunner>` at the registry level), not instantiated per `PluginAgent`.

---

### M3 — No forward secrecy; long-lived static X25519 secret

**File:** `crates/sync/src/encryption.rs`, lines 24–33; `crates/sync/src/identity.rs`, lines 57–63

The comment in `encryption.rs` correctly acknowledges: _"static X25519 DH provides no forward secrecy."_ This is documented but not flagged to users. Since the X25519 secret is derived deterministically from the Ed25519 signing key, compromising the signing key retroactively decrypts all past sync traffic.

For a v0.4.0 stability release this is an acceptable known limitation, but it needs a user-facing warning in the docs and a roadmap item for ephemeral DH (X3DH or noise protocol).

---

### M4 — `MetricsHandle::encode()` calls `unwrap()` on Mutex lock

**File:** `crates/cli/src/metrics_handle.rs`, line 153

```rust
let registry = self.registry.lock().unwrap();
```

If any thread panics while holding the registry lock, subsequent calls to `encode` (served on every `/metrics` HTTP request) will panic and crash the metrics server. Use `.unwrap_or_else(|e| e.into_inner())` to recover from a poisoned lock.

---

### M5 — `sync_all_vaults` creates one `SyncEngine` per vault sequentially

**File:** `crates/sync/src/lib.rs`, lines 135–147

```rust
for entry in enabled {
    let status = sync_single_vault_entry(entry).await;
    ...
}
```

Each vault opens an iroh transport (network bind) sequentially. For a user with many vaults this introduces unnecessary latency. Use `futures::future::join_all` or a `FuturesUnordered` to run vault syncs concurrently.

---

### M6 — `BatchCollector::flush` — cache write errors are silently swallowed

**File:** `crates/agent/src/llm/batch_collector.rs`, lines 113–115

```rust
if let Err(e) = cache.put(&req.cache_key, &response, &req.model) {
    tracing::warn!("llm cache write failed: {e}");
}
```

Cache write failures are logged at `warn` but not propagated. This is acceptable in most scenarios, but the `LlmCache::put` failure mode includes database corruption (`rusqlite` returns `SQLITE_CORRUPT`). A warn-and-continue policy is fine here, but the metric `llm_cache_hits` is never incremented on cache hits inside `BatchCollector` — the `MetricsHandle` is not threaded through. This means the Prometheus metric is always zero in practice.

---

### M7 — `wasm_host.rs` host imports do not validate `ptr`/`len` against memory bounds before read

**File:** `crates/agent/src/plugin/wasm_host.rs`, lines 21–33

```rust
let start = ptr as usize;
let end = start + len as usize;
if end <= data.len() {
    let msg = String::from_utf8_lossy(&data[start..end]);
```

`len` is an `i32` from the WASM side; casting a negative `i32` to `usize` wraps to a very large number, and `start + len as usize` silently overflows on 32-bit targets. The check `end <= data.len()` will fail safely in that case, but the arithmetic itself is UB-adjacent.

**Fix:** Cast through explicit i32 range check first:

```rust
if ptr < 0 || len < 0 { return; }
let start = ptr as usize;
let end = start.checked_add(len as usize).unwrap_or(usize::MAX);
```

---

## Low Priority

### L1 — `VaultRegistry::register` calls `fs::canonicalize` on a path that may not exist

**File:** `crates/sync/src/vault_registry.rs`, lines 78–80

```rust
let canonical = std::fs::canonicalize(&path)
    .unwrap_or_else(|_| path.clone());
```

The silent fallback means a typo in a vault path will register successfully but never match anything on `unregister` (which also falls back). The error is silently swallowed. Either validate the path exists at registration time and return an error, or document the fallback behaviour.

---

### L2 — `PluginsConfig::wasm` defaults do not sync with `PluginManifest` defaults

**Files:** `crates/core/src/config.rs` lines 400–406; `crates/agent/src/plugin/manifest.rs` lines 37–44

Both places define `64 MB` / `1_000_000 fuel` as defaults, but they are hardcoded independently. If one changes the other will silently diverge. Extract them to a single shared constant or inherit manifest defaults from the config.

---

### L3 — `show_metrics()` and `show_metrics_live()` are duplicated display paths

**File:** `crates/cli/src/commands/metrics_cmd.rs`

`show_metrics()` prints a hardcoded static list; `show_metrics_live()` prints real data. The CLI in `main.rs` does not appear to call either (no `Commands::Metrics` handler visible in the diff). The live path should replace the static path entirely once wired up.

---

### L4 — `PostgresBackend` feature `postgres` not applied in workspace sqlx features

**File:** `Cargo.toml` line 50

```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "any"] }
```

The `postgres` feature is missing from the workspace default. Building with `--features postgres` on the `core` crate activates the code but relies on `sqlx/any` which may not provide full typed Postgres support. Add `"postgres"` to the workspace `sqlx` features list so it is available unconditionally (or gate it behind the feature properly).

---

### L5 — `LlmCacheConfig::max_entries` not enforced

**File:** `crates/core/src/config.rs` lines 333–343; `crates/agent/src/llm/cache.rs`

`max_entries: 10000` is stored in config but `LlmCache::put` never checks the entry count. The `prune(ttl_secs)` function removes old entries but is never called automatically. There is no eviction policy. Over time the SQLite cache will grow unboundedly.

---

## Edge Cases Found During Review

1. **WASM `out_ptr`/`out_len` not validated before `memory.read`** — `wasm_runner.rs` lines 108–115: if a buggy plugin returns `out_ptr=0, out_len=0`, `vec![0u8; 0]` is parsed as `null` JSON, which silently succeeds or fails depending on how the caller interprets the value. No check that `out_ptr != 0`.

2. **`BatchCollector::flush` with zero requests** — `canonical` is empty, `slot_responses` is empty, `out` is empty, function returns `Ok({})`. Callers iterating results without checking len could silently miss responses. This is benign but should be documented.

3. **`sync_single_vault_entry` uses `ConflictPolicy::default()`** — the per-vault policy from the vault's config is never consulted. All multi-vault sync uses the global default policy, ignoring per-vault overrides.

4. **Vault registry stored at a fixed global path** (`~/.agentic-note/vaults.toml`) — no support for multiple users on the same machine, no XDG base dir support. Environment override mechanism is absent.

5. **WASM module reuse across concurrent requests is prevented** — `WasmPluginRunner` is wrapped in `Arc<Mutex<>>`, so all concurrent plugin invocations serialize on one runner even when they could use independent `Store` instances from a shared `Engine`. The `Mutex` is necessary for `&mut self` methods but prevents any parallelism.

---

## Positive Observations

- All six phases have unit tests with meaningful assertions, including negative cases (wrong key decryption, missing WASM file, cache miss/hit, prune).
- Error messages are consistently formatted with context (`"llm cache open: {e}"`) — excellent observability.
- `StorageBackend` trait design is clean: thin, async-friendly, and backend-agnostic. The `spawn_blocking` bridge is implemented correctly for both SQLite and the batch ops.
- `EncryptedPayload` serializes nonce alongside ciphertext — correct and self-contained.
- `batch_chat` default implementation on `LlmProvider` is a sensible non-breaking extension.
- `PluginRuntime::Wasm` defaulting to WASM in new manifests while preserving subprocess via `runtime = "subprocess"` is a clean backward-compatible migration path.
- `MetricsHandle` is properly `Clone` via `Arc` shares — no double-registration of metrics.
- The Prometheus HTTP server shutdown channel (`oneshot::Sender`) is a correct pattern for graceful shutdown.
- Feature-gating `storage_postgres` behind `#[cfg(feature = "postgres")]` keeps the default binary slim.

---

## Recommended Actions (Prioritized)

1. **[H1 – Security]** Apply HKDF-SHA256 over the X25519 DH output before use as ChaCha20 key in `encryption.rs`. Add `hkdf` crate dependency.
2. **[H2 – Security]** Implement `ResourceLimiter` in `wasm_runner.rs` to enforce the `memory_limit_mb` parameter.
3. **[M4 – Reliability]** Replace `.unwrap()` with poison-recovery in `MetricsHandle::encode()`.
4. **[M7 – Safety]** Add negative-value guards for `ptr`/`len` in `wasm_host::host_log`.
5. **[M2 – Performance]** Lift `WasmPluginRunner` (specifically the `Engine`) to a shared singleton to avoid per-agent JIT compilation cost.
6. **[M5 – Performance]** Parallelize `sync_all_vaults` loop with `join_all`.
7. **[M6 – Observability]** Thread `MetricsHandle` into `BatchCollector::flush` to increment `llm_cache_hits` counter on cache hits.
8. **[M3 – Docs]** Add user-facing documentation noting the no-forward-secrecy limitation; add roadmap item.
9. **[L5 – Correctness]** Wire `LlmCache::prune` into application startup or a periodic task; enforce `max_entries`.
10. **[L1 – UX]** Validate vault path exists at registration time in `VaultRegistry::register`.

---

## Metrics

| Metric | Value |
|---|---|
| New/changed files reviewed | 23 |
| New LOC | ~1,820 |
| Unit test coverage (new code) | Good — all new modules have tests |
| Critical issues | 0 |
| High issues | 2 |
| Medium issues | 7 |
| Low issues | 5 |
| Edge cases flagged | 5 |

---

## Unresolved Questions

1. Are the new subsystems (storage abstraction, LLM cache, Prometheus, encryption, vault registry, WASM) actually wired into the CLI binary's `main.rs`? The diff shows the `VaultRegistry` CLI command added, but there is no evidence in the diff that `MetricsHandle`, `LlmCache`, `SqliteBackend`, or encryption config are consulted at startup. If these remain unused at runtime, the v0.4.0 release ships non-functional features.

2. The `sqlx` workspace feature list does not include `"postgres"` — is the Postgres feature expected to work when the core crate is compiled with `--features postgres`?

3. Is the wasmtime version `28` pinned by choice or by workspace constraint? v28 has known breaking API changes from v26. Verify compatibility with the WASM ABI expected by plugin authors.

4. The `EncryptionConfig::enabled = false` default means encryption is opt-in. Is there a migration plan to make it default-on in a future release without breaking existing un-encrypted sync deployments?
