# Phase 27: Async Batch API for LLM Providers

## Context Links

- [Research: Async Batch API](research/researcher-testing-async-batch.md)
- Current LLM trait: `crates/agent/src/llm/mod.rs` (has `batch_chat` — concurrent real-time)
- Current batch collector: `crates/agent/src/llm/batch_collector.rs` (dedup + concurrent)
- OpenAI Batch API: upload JSONL → poll → download results (50% cost savings)

## Overview

- **Priority:** P2
- **Status:** completed
- **Effort:** 2.5h
- **Description:** Add async Batch API support (OpenAI file-upload style) alongside existing concurrent real-time batching. New optional trait methods with `NotSupported` default. Only OpenAI implements initially.

## Key Insights

- `async-openai` crate already supports Files + Batch APIs — no raw HTTP needed
- `tokio-retry` with `ExponentialBackoff` for polling (1s base, 60s max, ~20 retries)
- Existing `batch_chat` is concurrent real-time; new `batch_submit`/`batch_poll`/`batch_results` are async file-based
- Anthropic has Message Batches API (different endpoint); defer to future if needed
- Batch API: 50% cost reduction, 24h completion window, 50k requests/file limit

## Requirements

**Functional:**
- `LlmProvider` trait gains 3 optional async methods: `batch_submit`, `batch_poll`, `batch_results`
- Default impl returns `Err(AgenticError::Agent("batch not supported"))`
- OpenAI provider implements all 3 using `async-openai` Files + Batch APIs
- JSONL format per OpenAI spec: `{"custom_id", "method", "url", "body"}`
- Poll with exponential backoff until `completed`/`failed`/`expired`
- Anthropic/Ollama keep default (not supported)

**Non-functional:**
- Batch submit < 500ms (upload latency)
- Poll interval: 1s base, 60s max, 20 retries (~20 min total)
- Graceful error on batch failure/expiry

## Architecture

```
LlmProvider trait:
  chat()              — existing real-time
  batch_chat()        — existing concurrent real-time
  batch_submit()      — NEW: upload JSONL, returns BatchId
  batch_poll()        — NEW: check status, returns BatchStatus
  batch_results()     — NEW: download results, returns Vec<String>

OpenAI Batch Flow:
  1. Serialize requests to JSONL temp file
  2. POST /v1/files (upload, purpose=batch) → file_id
  3. POST /v1/batches (file_id, endpoint, window=24h) → batch_id
  4. GET /v1/batches/{id} (poll with backoff) → status
  5. GET /v1/files/{output_file_id}/content → results JSONL
  6. Parse results, match by custom_id, return ordered Vec
```

## Related Code Files

**Modify:**
- `crates/agent/src/llm/mod.rs` — add `BatchId`, `BatchStatus`, 3 optional trait methods
- `crates/agent/src/llm/openai.rs` — implement batch methods using async-openai
- `crates/agent/Cargo.toml` — add `async-openai`, `tokio-retry` deps
- `Cargo.toml` (workspace) — add workspace deps

**Create:**
- `crates/agent/src/llm/batch_api.rs` — shared types (`BatchId`, `BatchStatus`, JSONL helpers) (~80 LOC)

**No Delete.**

## Implementation Steps

1. Add workspace deps: `async-openai = "0.27"`, `tokio-retry = "0.3"`
2. Add to `crates/agent/Cargo.toml`: both as optional behind `batch-api` feature flag
3. Create `crates/agent/src/llm/batch_api.rs`:
   ```rust
   #[derive(Debug, Clone)]
   pub struct BatchId(pub String);

   #[derive(Debug, Clone, PartialEq)]
   pub enum BatchStatus {
       Validating, InProgress, Completed, Failed(String), Expired,
   }
   ```
4. Extend `LlmProvider` trait in `mod.rs`:
   ```rust
   async fn batch_submit(&self, _reqs: &[(Vec<Message>, ChatOpts)]) -> Result<BatchId> {
       Err(AgenticError::Agent("batch API not supported".into()))
   }
   async fn batch_poll(&self, _id: &BatchId) -> Result<BatchStatus> {
       Err(AgenticError::Agent("batch API not supported".into()))
   }
   async fn batch_results(&self, _id: &BatchId) -> Result<Vec<String>> {
       Err(AgenticError::Agent("batch API not supported".into()))
   }
   ```
5. Implement in `openai.rs` (behind `#[cfg(feature = "batch-api")]`):
   - `batch_submit`: serialize to JSONL → upload file → create batch → return BatchId
   - `batch_poll`: use tokio-retry ExponentialBackoff → poll GET /v1/batches/{id}
   - `batch_results`: download output file → parse JSONL → match custom_id → return ordered
6. Add `batch-api` feature to `crates/agent/Cargo.toml` (default off)
7. Write tests: mock provider returns NotSupported, OpenAI batch types serialize correctly
8. Compile check: `cargo check -p agentic-note-agent` and `cargo check -p agentic-note-agent --features batch-api`

## Todo List

- [x] Add async-openai and tokio-retry workspace deps
- [x] Create batch_api.rs with BatchId/BatchStatus types
- [x] Extend LlmProvider trait with 3 optional batch methods
- [x] Implement OpenAI batch_submit (JSONL upload + batch create)
- [x] Implement OpenAI batch_poll (exponential backoff)
- [x] Implement OpenAI batch_results (download + parse JSONL)
- [x] Add batch-api feature flag
- [x] Write unit tests
- [x] Compile check passes

## Success Criteria

- `LlmProvider` trait compiles with default methods (no breaking change)
- Anthropic/Ollama gracefully return `NotSupported`
- OpenAI batch flow: submit → poll → results works end-to-end
- Feature flag: `--features batch-api` enables OpenAI batch, default build excludes it
- `cargo test -p agentic-note-agent` passes
- 0 compiler warnings

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| async-openai API change | Low | Medium | Pin version, wrap in thin adapter |
| OpenAI Batch API rate limits | Medium | Low | Exponential backoff handles throttling |
| 24h completion window too slow | Low | Low | Document limitation; use concurrent real-time for urgent |
| async_trait + default methods | Low | Medium | Tested pattern; same as existing batch_chat |

## Security Considerations

- API key handling unchanged — reuse existing OpenAI provider config
- JSONL temp files may contain prompts — write to tempdir, delete after upload
- No new secret storage needed

## Next Steps

- Phase 28 adds integration tests for batch flow with mock server
- Phase 30 documents batch API in rustdoc with examples
- Future: Anthropic Message Batches API implementation
