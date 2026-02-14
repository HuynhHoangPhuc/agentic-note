# Phase Implementation Report

## Executed Phase
- Phase: Phase 21 - Batch LLM Requests
- Plan: /Users/phuc/Developer/agentic-note/plans/260214-2002-v040-stability-production-hardening
- Status: completed

## Files Modified

| File | Lines | Action |
|------|-------|--------|
| `crates/agent/src/llm/cache.rs` | 165 | created |
| `crates/agent/src/llm/batch_collector.rs` | 200 | created |
| `crates/agent/src/llm/mod.rs` | 99 | modified - exported new modules, added default `batch_chat` to trait |
| `crates/agent/src/llm/openai.rs` | 140 | modified - added concurrent `batch_chat` override |
| `crates/agent/src/llm/anthropic.rs` | 121 | modified - added concurrent `batch_chat` override |
| `crates/agent/Cargo.toml` | 31 | modified - added sha2, futures, rusqlite, chrono |
| `crates/agent/src/plugin/runner.rs` | 1 line | fixed pre-existing `E0521` borrow lifetime bug blocking test compilation |

## Tasks Completed

- [x] `cache.rs` - SQLite-backed `LlmCache` with `new`, `get`, `put`, `prune`, `compute_key` (SHA-256)
- [x] `batch_collector.rs` - `BatchCollector` with `add` / `flush`, deduplication, concurrent execution via `futures::join_all`, read-through cache
- [x] `LlmProvider::batch_chat` default method added to trait in `mod.rs`
- [x] `OpenAiProvider::batch_chat` override with concurrent `join_all`
- [x] `AnthropicProvider::batch_chat` override with concurrent `join_all`
- [x] `Cargo.toml` updated with `sha2`, `futures`, `rusqlite`, `chrono`
- [x] 8 unit tests written (5 in `cache.rs`, 3 in `batch_collector.rs` incl. async flush test)
- [x] Pre-existing `plugin/runner.rs` borrow lifetime error fixed (cloned Arc before move closure)

## Tests Status
- Type check: pass (`cargo check -p agentic-note-agent` clean)
- Unit tests: **46 passed / 0 failed** (`cargo test -p agentic-note-agent --lib`)
  - `llm::cache` - 5 tests pass
  - `llm::batch_collector` - 3 tests pass (incl. async cache-hit counting test)
  - All pre-existing engine/plugin tests continue to pass

## Issues Encountered

1. `AgenticError::Storage` does not exist - used `AgenticError::Database` instead (correct variant from `crates/core/src/error.rs`).
2. Pre-existing `E0521` lifetime bug in `crates/agent/src/plugin/runner.rs:70` blocked `--lib` test compilation. Fixed by replacing `let runner_ref = runner` (borrow) with `Arc::clone(runner)` (owned clone) — 1-line change, no behavioral difference.

## Next Steps

- `LlmCacheConfig` from `crates/core/src/config.rs` can be wired to `LlmCache::new(db_path)` + `prune(ttl_secs)` at app startup in the CLI or agent orchestrator.
- `BatchCollector` is ready to be used anywhere multiple LLM calls are made in a single agent pipeline stage.
