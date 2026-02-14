# Phase Implementation Report

## Executed Phase
- Phase: phase-07-llm-agents-review-queue
- Plan: none (direct task)
- Status: completed

## Files Modified

| File | Lines | Action |
|------|-------|--------|
| `crates/agent/Cargo.toml` | 20 | added reqwest, agentic-note-search, agentic-note-review deps |
| `crates/agent/src/lib.rs` | 6 | added `pub mod llm` and `pub mod agents` |
| `crates/agent/src/llm/mod.rs` | 83 | created: Message, ChatOpts, LlmProvider trait, ProviderRegistry |
| `crates/agent/src/llm/openai.rs` | 96 | created: OpenAI/Ollama provider |
| `crates/agent/src/llm/anthropic.rs` | 85 | created: Anthropic Claude provider |
| `crates/agent/src/agents/mod.rs` | 34 | created: re-exports + register_builtin_agents() |
| `crates/agent/src/agents/para_classifier.rs` | 50 | created: PARA classification agent |
| `crates/agent/src/agents/distiller.rs` | 50 | created: note distillation agent |
| `crates/agent/src/agents/zettelkasten_linker.rs` | 97 | created: FTS+LLM link suggestion agent |
| `crates/agent/src/agents/vault_writer.rs` | 85 | created: proposed-changes aggregator |
| `crates/review/Cargo.toml` | 16 | full rewrite with rusqlite, chrono, ulid, serde_json |
| `crates/review/src/lib.rs` | 5 | re-exports for queue and gate modules |
| `crates/review/src/queue.rs` | 185 | created: SQLite review queue with CRUD |
| `crates/review/src/gate.rs` | 100 | created: approval gate for TrustLevel routing |

## Tasks Completed

- [x] LlmProvider trait + Message + ChatOpts + ProviderRegistry
- [x] OpenAI provider (POST /chat/completions, json_mode, Bearer auth)
- [x] Anthropic provider (POST /v1/messages, x-api-key, system field split)
- [x] ParaClassifier agent — LLM returns `{para, tags, confidence}`
- [x] Distiller agent — LLM returns `{summary, key_ideas}`
- [x] ZettelkastenLinker agent — FTS candidates + LLM ranking, Mutex wrapping for non-Sync SearchEngine
- [x] VaultWriter agent — aggregates upstream stage outputs into proposed-changes JSON
- [x] register_builtin_agents() function
- [x] ReviewQueue (SQLite): enqueue / list / get / approve / reject
- [x] Gate function: TrustLevel::Auto → Apply, Review/Manual → Queued
- [x] 6 tests in review crate (3 queue CRUD, 3 gate logic)
- [x] Files kept under 200 lines

## Tests Status
- Type check: pass (zero errors, zero warnings after cleanup)
- Unit tests (agent): 8 passed / 0 failed
- Unit tests (review): 6 passed / 0 failed
- Integration tests: n/a

## Issues Encountered

1. `SearchEngine` wraps `rusqlite::Connection` which is not `Sync`. Fixed by changing `ZettelkastenLinker` and `register_builtin_agents` to accept `Option<Arc<Mutex<SearchEngine>>>` instead of `Option<Arc<SearchEngine>>`.
2. `ulid` crate not in review's Cargo.toml — added as workspace dep.
3. Unused `json` import in zettelkasten_linker removed after first compile.

## Next Steps

- CLI integration: wire `register_builtin_agents` into the CLI's `agent run` command
- ReviewQueue path: pass `.agentic/reviews.db` from AppConfig to gate/queue
- VaultWriter apply logic: CLI reads `GateAction::Apply` result and patches frontmatter on disk
- Ollama support: use `OpenAiProvider::new_custom` with `base_url = "http://localhost:11434/v1"`
