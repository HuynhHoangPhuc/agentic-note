# Research: Test Coverage & Async Batch API for Rust Workspace
Date: 2026-02-18 | Scope: agentic-note (8-crate workspace)

---

## Topic 1: Comprehensive Test Coverage in Rust Workspaces

### Tool Recommendation: cargo-llvm-cov (winner)

**cargo-llvm-cov** is the officially recommended tool. Prefer over tarpaulin for:
- Cross-platform (macOS/Linux/Windows); tarpaulin is Linux-only
- LLVM source-based instrumentation = more accurate branch/line coverage
- Native `--workspace` flag for multi-crate projects
- HTML + JSON + lcov output formats for CI integration

```bash
cargo llvm-cov --workspace --html
cargo llvm-cov --workspace --lcov --output-path lcov.info  # for CI
```

**tarpaulin** only viable if CI is Linux-only and team prefers simpler setup.

Refs: [cargo-llvm-cov](https://lib.rs/crates/cargo-llvm-cov) | [comparison thread](https://github.com/rusqlite/rusqlite/issues/1195) | [Rust coverage guide](https://blog.rng0.io/how-to-do-code-coverage-in-rust/)

---

### Property-Based Testing

Use **proptest** (recommended over quickcheck) for CAS/merge/sync logic:
- Shrinks failing inputs automatically (quickcheck does not shrink well)
- Macro-based API, integrates with standard `#[test]`

**Caveat with async**: `proptest!` macro doesn't support `async fn` directly.
Workaround — wrap in sync test, call `tokio::runtime::Runtime::block_on`:

```rust
proptest! {
    #[test]
    fn test_merge_idempotent(data in any::<Vec<u8>>()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { /* test merge/CAS logic */ });
    }
}
```

Refs: [proptest async issue](https://github.com/AltSysrq/proptest/issues/179) | [proptest intro](https://www.lpalmieri.com/posts/an-introduction-to-property-based-testing-in-rust/)

---

### Cross-Crate Integration Test Patterns

Workspace integration tests live in `crate/tests/` dirs. For cross-crate workflows:

1. **Dedicated integration test crate** — create `crates/integration-tests/` that depends on multiple crates; keeps unit tests isolated
2. **Feature flags** for test helpers — expose `#[cfg(test)]` helpers via a `test-utils` feature
3. **Shared fixtures** — put common test helpers in `crates/test-utils/` crate (dev-dependency only)
4. **tokio::test with multi-thread** — for async cross-crate tests, use `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`

Path to 90%+ coverage:
- Add `proptest` for CAS hash collision, merge conflict, CRDT convergence scenarios
- Integration tests for end-to-end note create → sync → retrieve flows
- Use `tokio::time::pause()` for deterministic timer-based tests (avoids slow sleep)

Refs: [Tokio testing guide](https://tokio.rs/tokio/topics/testing) | [Rust testing overview](https://www.shuttle.dev/blog/2024/03/21/testing-in-rust)

---

## Topic 2: Async Batch API for LLM Providers in Rust

### OpenAI Batch API Overview

**Flow**: Upload JSONL file → Create batch job → Poll status → Download results

Key endpoints:
- `POST /v1/files` — upload `.jsonl` (purpose: `batch`), returns `file_id`
- `POST /v1/batches` — create batch with `file_id`, `endpoint`, `completion_window` (max `24h`)
- `GET /v1/batches/{id}` — poll; status: `validating` → `in_progress` → `completed`/`failed`
- `GET /v1/files/{output_file_id}/content` — download results JSONL

JSONL request format per line:
```json
{"custom_id":"req-1","method":"POST","url":"/v1/chat/completions","body":{...}}
```

Limits: 50,000 requests/file, 200 MB max, results within 24h window.

Refs: [OpenAI Batch API](https://platform.openai.com/docs/api-reference/batch) | [async-openai crate](https://crates.io/crates/async-openai)

---

### Rust Implementation Pattern

**async-openai** crate already supports Batch + Files APIs — prefer using it over rolling own HTTP client.

**Polling pattern with tokio + exponential backoff:**

```rust
use tokio_retry::{strategy::ExponentialBackoff, Retry};

async fn poll_batch(client: &Client, batch_id: &str) -> Result<Batch> {
    let strategy = ExponentialBackoff::from_millis(1000)
        .max_delay(Duration::from_secs(60))
        .take(20);  // ~20 retries, max ~20 min total

    Retry::spawn(strategy, || async {
        let batch = client.batches().retrieve(batch_id).await?;
        match batch.status {
            BatchStatus::Completed => Ok(batch),
            BatchStatus::Failed | BatchStatus::Expired => Err(permanent_err()),
            _ => Err(transient_err()),  // retryable
        }
    }).await
}
```

Use `tokio-retry` crate for structured retry logic.
Refs: [tokio-retry](https://github.com/srijs/rust-tokio-retry) | [ExponentialBackoff docs](https://docs.rs/tokio-retry/latest/tokio_retry/strategy/struct.ExponentialBackoff.html)

---

### Integration with Existing LlmProvider Trait

Extend the trait with optional batch methods (default impl returns `Err(NotSupported)`):

```rust
pub trait LlmProvider: Send + Sync {
    // existing real-time method
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse>;

    // new optional batch methods
    async fn batch_submit(&self, reqs: Vec<CompletionRequest>) -> Result<BatchId> {
        Err(LlmError::NotSupported("batch".into()))
    }
    async fn batch_poll(&self, id: &BatchId) -> Result<BatchStatus> {
        Err(LlmError::NotSupported("batch".into()))
    }
    async fn batch_results(&self, id: &BatchId) -> Result<Vec<CompletionResponse>> {
        Err(LlmError::NotSupported("batch".into()))
    }
}
```

Only OpenAI/Anthropic providers override batch methods; Ollama falls back gracefully.

---

## Summary

| Concern | Recommendation |
|---|---|
| Coverage tool | `cargo-llvm-cov --workspace` |
| Property testing | `proptest` + sync wrapper for async |
| Cross-crate tests | Dedicated `crates/integration-tests/` + `crates/test-utils/` |
| Batch HTTP client | `async-openai` (has Batch API built-in) |
| Polling retry | `tokio-retry` with `ExponentialBackoff` |
| Trait extension | Optional default methods, `NotSupported` fallback |

---

## Unresolved Questions

1. Does the existing `LlmProvider` trait use `async_trait` macro or RPITIT? Affects how default async methods are added.
2. Anthropic does not have a public Batch API identical to OpenAI's — need to verify if they have message batches API (they do as of 2024, but endpoint differs).
3. What's the target coverage per crate vs aggregate 90%? Some crates (e.g., storage) may need higher bar than CLI crates.
4. Should batch results be streamed as they arrive or collected and returned atomically?
