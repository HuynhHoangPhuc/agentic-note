---
title: "Phase 36: Integration Tests — LLM + P2P Sync"
description: "E2E tests with real LLM APIs behind live-tests feature flag; P2P sync tests via iroh relay."
status: in-progress
priority: P1
effort: 6h
phase: 36
---

# Phase 36: Integration Tests — LLM + P2P Sync

## Context Links

- Research: [Integration Testing & crates.io Research](./research/researcher-testing-cratesio.md) — Topic 1
- Plan: [plan.md](./plan.md)
- Codebase: `crates/integration-tests/`, `crates/test-utils/`, `crates/agent/src/llm/`, `crates/sync/src/`

## Overview

- **Date:** 2026-02-18
- **Priority:** P1
- **Status:** in-progress
- **Description:** Implement integration tests for LLM providers and sync behavior. Current slice uses a local mock HTTP server by default, `--features live-tests` for real LLM calls, and hermetic in-memory transport for sync protocol coverage. Fix runtime bugs discovered during testing.

## Current Implementation Slice

- Landed: OpenAI + Anthropic custom `base_url` support
- Landed: hermetic mock-server-backed LLM integration tests
- Landed: hermetic sync protocol tests for identical peers, one-sided fast-forward, non-conflicting merge, and manual conflict materialization
- Landed: `live-tests` feature and CI workflow for real OpenAI + Anthropic checks
- Remaining: broader sync breadth such as 3-peer convergence and structural conflict coverage

## Key Insights

- Mock HTTP server on ephemeral loopback port is sufficient for provider integration coverage; inject custom base URL into LLM providers
- LLM base URL is now configurable in provider config and constructors for OpenAI/Anthropic
- Hermetic in-memory transport is enough to validate sync protocol semantics before introducing iroh relay complexity
- Use `tokio::sync::OnceCell` for shared relay server across tests in the same module (avoid re-binding per test)
- Feature flag pattern: `#[cfg(feature = "live-tests")]` gates real API calls; default CI runs wiremock path
- CI: add nightly job `cargo test --features live-tests` with API key secrets injected (separate workflow)
- Keep wiremock stubs as JSON fixtures in `crates/integration-tests/fixtures/`

## Requirements

### Functional — LLM Tests
- Wiremock stubs: happy-path responses for OpenAI chat completions, Anthropic messages, Ollama generate
- Test: `para_classifier` agent classifies a note → asserts PARA category returned
- Test: `distiller` agent summarizes a note → asserts non-empty summary
- Test: `batch_llm` sends multiple requests → asserts all complete (mocked)
- Live tests (feature-gated): same tests against real APIs; assert valid structured responses

### Functional — P2P Sync Tests
- Test: two endpoints discover each other via local relay → connect → exchange a sync snapshot → verify merge
- Test: conflict scenario → `NewestWins` policy resolves correctly
- Test: `batch_sync` with 3 peers → all converge to same vault state
- Each test isolated: fresh temp dirs, fresh endpoints, relay guard dropped after test

### Non-Functional
- Default `cargo test` (no flags): uses wiremock stubs, passes offline
- `--features live-tests`: hits real APIs; requires `OPENAI_API_KEY`, `ANTHROPIC_API_KEY` env vars
- P2P tests: hermetic (no external network); current suite uses in-memory transport
- Test timeout: 30s per test (tokio timeout wrapper)

## Architecture

```
crates/integration-tests/
├── Cargo.toml                          — add wiremock, live-tests feature
└── tests/
    ├── llm_agent_integration_test.rs   (new) — LLM + agent pipeline tests
    ├── p2p_sync_integration_test.rs    (new) — iroh relay-based sync tests
    └── fixtures/
        ├── openai_chat_response.json   (new) — wiremock stub body
        ├── anthropic_message_response.json (new)
        └── ollama_generate_response.json   (new)

crates/test-utils/src/
├── llm_mock_server.rs                  (new) — wiremock server builder helper
└── p2p_test_harness.rs                 (new) — relay + endpoint setup helper
```

**LLM mock server (test-utils/llm_mock_server.rs):**
```rust
pub struct LlmMockServer {
    server: MockServer,
    pub base_url: String,
}
impl LlmMockServer {
    pub async fn start_openai() -> Self {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_raw(include_str!("../fixtures/openai_chat_response.json"), "application/json"))
            .mount(&server).await;
        Self { base_url: server.uri(), server }
    }
}
```

**P2P harness (test-utils/p2p_test_harness.rs):**
```rust
pub struct P2pTestHarness {
    pub relay_url: Url,
    _relay_guard: RelayServerGuard,  // drops relay on teardown
}
impl P2pTestHarness {
    pub async fn new() -> Self {
        let (relay_url, guard) = iroh::test_utils::run_relay_server().await.unwrap();
        Self { relay_url, _relay_guard: guard }
    }
    pub async fn make_endpoint(&self) -> Endpoint {
        Endpoint::builder()
            .insecure_skip_relay_cert_verify(true)
            .relay_url(self.relay_url.clone())
            .bind().await.unwrap()
    }
}
```

**LLM provider base_url override (user-facing + test):**
- Add `base_url: Option<String>` to `OpenAiConfig`, `AnthropicConfig`, `OllamaConfig`
- Expose in `config.toml` schema: `[llm.providers.openai] base_url = "https://custom-endpoint.example.com"`
- Priority: config.toml `base_url` > env var `OPENAI_BASE_URL` > default API URL
- Users benefit: Azure OpenAI, local proxies (LiteLLM), corporate gateways
- In tests: set via env var `OPENAI_BASE_URL` / `ANTHROPIC_BASE_URL` pointing to wiremock
<!-- Updated: Validation Session 1 - Expose base_url as user-facing config, not just test infra -->

## Related Code Files

### Modify
- `crates/integration-tests/Cargo.toml` — add wiremock + live-tests feature
- `crates/test-utils/Cargo.toml` — add wiremock
- `crates/test-utils/src/lib.rs` — expose new modules
- `crates/agent/src/llm/openai.rs` — add `base_url` override
- `crates/agent/src/llm/anthropic.rs` — add `base_url` override
- `crates/agent/src/llm/ollama.rs` — add `base_url` override (already has `base_url`)

### Create
- `crates/test-utils/src/mock_llm_server.rs` (~80 LOC)
- `crates/integration-tests/tests/llm_agent_integration.rs` (~150 LOC)
- `crates/integration-tests/tests/p2p_sync_protocol.rs` (~200 LOC)
- `crates/integration-tests/tests/fixtures/openai_chat_response.json`
- `crates/integration-tests/tests/fixtures/anthropic_message_response.json`
- `crates/integration-tests/tests/fixtures/ollama_generate_response.json`

## Implementation Steps

1. **Add `live-tests` feature** to `crates/integration-tests/Cargo.toml`:
   ```toml
   [features]
   live-tests = []

   [dev-dependencies]
   wiremock = "0.6"
   ```
   Also add wiremock to `crates/test-utils/Cargo.toml`.

2. **Create fixture JSON files** — minimal valid API responses:
   - `openai_chat_response.json`: `{"choices":[{"message":{"content":"inbox"}}],...}`
   - `anthropic_message_response.json`: `{"content":[{"text":"inbox"}],...}`
   - `ollama_generate_response.json`: `{"response":"inbox","done":true}`

3. **Add `base_url` to LLM provider config + code** — openai.rs, anthropic.rs:
   - Add `base_url: Option<String>` to provider config structs in `crates/core/src/config.rs` (under `[llm.providers.*]`)
   - In provider constructors: read `config.base_url` first, then fallback to env var `OPENAI_BASE_URL`, then default
   ```rust
   pub struct OpenAiProvider {
       base_url: String,  // resolved from config > env > default
       api_key: String,
       model: String,
   }
   impl OpenAiProvider {
       pub fn with_base_url(mut self, url: String) -> Self { self.base_url = url; self }
   }
   ```
   <!-- Updated: Validation Session 1 - base_url is user-facing config, not just test infra -->

4. **Create `test-utils/src/llm_mock_server.rs`:**
   - `LlmMockServer::start_openai()`, `start_anthropic()`, `start_ollama()`
   - Each mounts appropriate endpoint + fixture response

5. **Create `test-utils/src/p2p_test_harness.rs`:**
   - `P2pTestHarness::new()` starts relay
   - `make_endpoint()` creates configured iroh Endpoint
   - `make_vault(dir: &TempDir) -> SyncEngine` — helper to init vault + sync engine in temp dir

6. **Create `llm_agent_integration_test.rs`:**
   ```rust
   #[tokio::test]
   async fn test_para_classifier_with_mock() {
       let mock = LlmMockServer::start_openai().await;
       let provider = OpenAiProvider::new("fake-key", "gpt-4o")
           .with_base_url(mock.base_url.clone());
       let result = para_classifier::classify(&provider, "test note body").await;
       assert!(result.is_ok());
   }

   #[cfg(feature = "live-tests")]
   #[tokio::test]
   async fn test_para_classifier_live() {
       let key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY required");
       let provider = OpenAiProvider::new(&key, "gpt-4o");
       let result = para_classifier::classify(&provider, "Meeting notes from standup").await;
       assert!(result.is_ok());
   }
   ```

7. **Create `p2p_sync_integration_test.rs`:**
   ```rust
   #[tokio::test]
   async fn test_two_peer_sync_basic() {
       let harness = P2pTestHarness::new().await;
       let dir1 = TempDir::new().unwrap();
       let dir2 = TempDir::new().unwrap();
       // create note in vault1, sync to vault2, assert note exists in vault2
   }
   ```

8. **Run tests:** `cargo test -p integration-tests` — all pass offline

9. **Verify `--features live-tests` compiles** (even if API keys absent — test should be skipped or fail clearly)

10. **Add CI workflow** `.github/workflows/live-tests.yml` — manual trigger, `live-tests` feature, secrets injected

## Todo List

- [x] Add live-tests feature to integration-tests Cargo.toml
- [ ] Add wiremock to test-utils Cargo.toml
- [ ] Create OpenAI fixture JSON
- [ ] Create Anthropic fixture JSON
- [ ] Create Ollama fixture JSON
- [x] Add base_url override to openai.rs + anthropic.rs
- [x] Create test-utils/src/mock_llm_server.rs
- [ ] Create test-utils/src/p2p_test_harness.rs
- [x] Expose new modules from test-utils/src/lib.rs
- [x] Create llm_agent_integration.rs (mock + live-tests gated)
- [x] Create p2p_sync_protocol.rs (hermetic protocol coverage)
- [x] `cargo test -p integration-tests` passes offline
- [x] Add `.github/workflows/live-tests.yml`
- [x] Fix runtime bugs found during hermetic sync testing

## Success Criteria

- `cargo test -p integration-tests` passes with no network access (mock server stubs)
- P2P sync tests pass: identical peers, one-sided fast-forward, non-conflicting merge, manual conflict materialization
- `cargo test -p integration-tests --features live-tests` passes with real API keys (manual verification)
- Zero test-to-test state leakage (each test uses fresh temp dirs + endpoints)

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| iroh `test_utils` API change between 0.96.x patch | Medium | Pin iroh version; check changelog before upgrade |
| LLM live tests flaky due to rate limits | High | Add retry with backoff in live test harness; separate nightly CI job |
| base_url override breaks existing LLM integration | Low | Unit test: existing provider behavior unchanged when override absent |

## Security Considerations

- API keys only in env vars; never hardcoded in fixtures or test files
- CI secrets scoped to live-tests workflow only; not available in PR builds from forks
- Fixture JSON: use fake/placeholder API key values in response bodies (not real)

## Next Steps

- Phase 37: Stress + fuzz testing — uses fixtures and harness from this phase
