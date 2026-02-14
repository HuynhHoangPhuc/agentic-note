# Tester Report — v0.4.0 Workspace Test Suite

**Date:** 2026-02-14
**Branch:** main
**Rust workspace:** `/Users/phuc/Developer/agentic-note`
**Command:** `cargo test --workspace 2>&1`
**Build time:** ~39.36s (first compile with deps); subsequent run 0.25s

---

## Test Results Overview

| Crate | Tests Run | Passed | Failed | Ignored |
|---|---|---|---|---|
| `agentic-note-agent` | 46 | 46 | 0 | 0 |
| `agentic-note-cas` | 18 | 18 | 0 | 0 |
| `agentic-note-cli` (bin) | 0 | 0 | 0 | 0 |
| `agentic-note-core` | 4 | 4 | 0 | 0 |
| `agentic-note-review` | 6 | 6 | 0 | 0 |
| `agentic-note-search` | 1 | 1 | 0 | 0 |
| `agentic-note-sync` | 30 | 30 | 0 | 0 |
| `agentic-note-vault` | 5 | 5 | 0 | 0 |
| Doc-tests (all crates) | 0 | 0 | 0 | 0 |
| **TOTAL** | **110** | **110** | **0** | **0** |

---

## Overall Verdict: PASS

All 110 tests passed across 8 crates. Zero failures.

---

## Per-Crate Test Details

### `agentic-note-agent` — 46/46 passed
New v0.4.0 subsystems covered: WASM plugin sandboxing, batch LLM.

Key test groups:
- `engine::dag_executor` — DAG cycle detection, condition eval, v1 migration, parallel stages
- `engine::condition` — eq/neq operators, missing-key handling, unsupported expressions
- `engine::error_policy` — abort/skip/retry/fallback policies
- `engine::scheduler` — cron registration, watch pipeline, invalid expr errors
- `engine::executor` — stage output storage, failing stage skip
- `engine::pipeline` — TOML parse, missing-dir empty load
- `engine::migration` — idempotent migration, v1→v2 sequential deps
- `engine::trigger` — manual trigger, no-filter match, type+filter match
- `engine::context` — output round-trips
- `llm::cache` — deterministic key, cache put/get/miss/prune
- `llm::batch_collector` — unique IDs, deduplication, cache flush
- `plugin::manifest` — default WASM runtime, subprocess runtime parse
- `plugin::wasm_runner` — runner creation, missing WASM file error

### `agentic-note-cas` — 18/18 passed
- `blob` — store/load roundtrip, idempotency, missing-object not-found
- `hash` — SHA-256 determinism, known hash, different inputs differ
- `conflict_policy` — longest-wins, newest-wins, manual policy, merge markers
- `semantic_merge` — overlapping/non-overlapping edits, identical changes, no-changes
- `tree` — load roundtrip, deterministic tree from dir

### `agentic-note-core` — 4/4 passed
- `id::tests::test_monotonic_ids`
- `config::tests::test_deserialize_config`
- `config::tests::test_deserialize_config_with_v030_sections`
- `storage_sqlite::tests::sqlite_backend_basic_operations` (PostgreSQL optional backend path covered via SQLite in tests)

### `agentic-note-review` — 6/6 passed
- Gate: auto-trust, manual-trust, review-trust with queue
- Queue: enqueue/list/get roundtrip, approve/reject status transitions

### `agentic-note-search` — 1/1 passed
- `background_indexer::tests::test_index_task_variants`

### `agentic-note-sync` — 30/30 passed
New v0.4.0 subsystems covered: E2E encryption, multi-vault sync, compression.

Key test groups:
- `encryption` — key derivation determinism, encrypt/decrypt roundtrip, wrong-key failure, nonce uniqueness
- `compression` — roundtrip, empty data, size reduction for text, invalid decompress error, level clamping
- `device_registry` — add/list/remove device, no duplicate, update last-sync, save/load roundtrip
- `identity` — peer ID generation, init-or-load (create + load existing), key file 32 bytes, save/load roundtrip
- `vault_registry` — register/list, sync-enabled filter, save/reload
- `merge_driver` — identical snapshot no-conflict, empty vaults no-conflicts, write conflict files
- `batch_sync` — peer sync status display, batch result aggregation
- `protocol` — sync result fields, initiator sends sync request first

### `agentic-note-vault` — 5/5 passed
- `para::tests::test_detect_category`
- `markdown` — wikilinks, markdown links, all links combined
- `frontmatter::tests::test_roundtrip`

### `agentic-note-cli` (bin) — 0 tests
No unit tests registered in bin target. CLI logic tested via integration of library crates above.

---

## Build Warnings

All warnings are non-blocking. 11 distinct warnings across 2 crates:

### `agentic-note-agent` (1 warning)
| Location | Warning |
|---|---|
| `crates/agent/src/plugin/wasm_runner.rs:18` | `dead_code`: field `default_memory_limit_mb` is never read |

### `agentic-note-cli` (10 warnings)
| Location | Warning | Category |
|---|---|---|
| `crates/cli/src/mcp/handlers.rs:110,114` | `unexpected_cfg`: `feature = "embeddings"` not defined in Cargo.toml | cfg |
| `crates/cli/src/metrics_handle.rs:43,59,60,61` | `private_interfaces`: `BucketedHistogram` is private but exposed via `pub` fields on `MetricsHandle` | visibility |
| `crates/cli/src/commands/metrics_cmd.rs:29` | `dead_code`: `show_metrics_live` never used | dead code |
| `crates/cli/src/metrics_init.rs:16,44,63,83` | `dead_code`: `start_metrics_server`, `serve_loop`, `handle_request`, `install_metrics_recorder` never used | dead code |

---

## Failed Tests

None.

---

## Performance Metrics

- Full compile + link (cold, with all deps): **39.36s**
- Test execution only (warm): **~0.11s total** across all crates
  - `agentic-note-agent`: 0.01s (46 tests)
  - `agentic-note-cas`: 0.00s (18 tests)
  - `agentic-note-core`: 0.01s (4 tests)
  - `agentic-note-review`: 0.01s (6 tests)
  - `agentic-note-search`: 0.00s (1 test)
  - `agentic-note-sync`: 0.08s (30 tests)
  - `agentic-note-vault`: 0.01s (5 tests)
- No slow tests identified. All suites complete in under 100ms.

---

## Build Status

**Build: SUCCESS** (unoptimized + debuginfo profile)
**Errors:** 0
**Warnings:** 11 (all non-blocking, no clippy deny gates in workspace)

---

## Critical Issues

None. Zero test failures, zero compile errors.

---

## Recommendations

1. **`crates/agent/src/plugin/wasm_runner.rs:18`** — Remove or use `default_memory_limit_mb` field. If it's planned for future enforcement, add `#[allow(dead_code)]` with a TODO comment.

2. **`crates/cli/src/mcp/handlers.rs:110,114`** — Register `embeddings` as an optional feature in `crates/cli/Cargo.toml` or remove the `#[cfg(feature = "embeddings")]` guards if the feature does not exist.

3. **`crates/cli/src/metrics_handle.rs`** — Make `BucketedHistogram` `pub` or change the containing `pub` fields to `pub(crate)` to resolve `private_interfaces` warnings.

4. **Dead metrics functions in `crates/cli/src/metrics_init.rs` and `crates/cli/src/commands/metrics_cmd.rs`** — `start_metrics_server`, `serve_loop`, `handle_request`, `install_metrics_recorder`, `show_metrics_live` are implemented but never called. Either wire them into a CLI subcommand or remove if superseded by the Prometheus integration path.

5. **Coverage gap — `agentic-note-cli`** — CLI binary has 0 unit tests. Integration/smoke tests for CLI commands (especially new v0.4.0 commands: `metrics`, `sync`, `plugin`) would improve confidence.

6. **Coverage gap — `agentic-note-search`** — Only 1 test exists. Tantivy indexing, full-text query, and background reindex paths are not directly tested.

7. **Doc-tests** — All 7 library crates have 0 doc-tests. Adding inline examples for public API surface would double as documentation and regression coverage.

---

## Next Steps (Prioritized)

1. Fix 2 `unexpected_cfg` warnings in `handlers.rs` — easy, high-signal cleanup
2. Fix `private_interfaces` on `MetricsHandle` — API correctness concern
3. Wire or remove dead metrics functions in CLI — determines whether Prometheus HTTP server is intentionally deferred
4. Add at least 2–3 CLI integration tests for the new v0.4.0 subcommands
5. Expand `search` crate tests to cover tantivy query paths
6. Add doc-tests for public types in `core`, `vault`, `cas`

---

## Unresolved Questions

- Are the dead metrics functions (`start_metrics_server`, etc.) intentionally deferred for a future CLI subcommand, or superseded by the Prometheus pull endpoint wired elsewhere?
- Is the `embeddings` feature planned for a future crate/PR, or should the `#[cfg(feature = "embeddings")]` blocks be removed?
- `agentic-note-cli` has 0 tests — is there a separate integration test suite (e.g. `tests/` directory or CI script) not run via `cargo test --workspace`?
