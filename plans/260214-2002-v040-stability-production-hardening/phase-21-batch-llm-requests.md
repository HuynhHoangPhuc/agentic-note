# Phase 21: Batch LLM Requests

## Context Links
- [plan.md](plan.md)
- [crates/agent/src/llm/mod.rs](/Users/phuc/Developer/agentic-note/crates/agent/src/llm/mod.rs) — LlmProvider trait
- [crates/agent/src/llm/openai.rs](/Users/phuc/Developer/agentic-note/crates/agent/src/llm/openai.rs) — OpenAI provider
- [crates/agent/src/llm/anthropic.rs](/Users/phuc/Developer/agentic-note/crates/agent/src/llm/anthropic.rs) — Anthropic provider

## Overview
- **Priority:** P2
- **Status:** Complete
- **Implementation Status:** complete
- **Review Status:** complete
- **Effort:** 2.5h
- **Description:** Reduce LLM API calls via request batching and response caching. Collect multiple prompts within a pipeline run, deduplicate by content hash, batch where API supports it.

## Key Insights
- OpenAI Batch API is async (POST /v1/batches, poll for results) — good for bulk processing, not real-time
- For real-time pipelines: batch concurrent stage requests within a single DAG layer execution
- Content-hash deduplication cache prevents re-calling LLM for identical prompts
- Anthropic Message Batches API similar pattern (POST /v1/messages/batches)
- Cache in SQLite (content hash -> response, TTL-based expiration)

## Requirements

### Functional
- `BatchCollector` collects prompts from parallel DAG stages before sending
- Content-hash deduplication: SHA-256 of (model + messages + opts) -> cached response
- LLM response cache in SQLite with configurable TTL
- `LlmProvider` trait extended with optional `batch_chat` method (default falls back to sequential)
- OpenAI batch support: concurrent requests via reqwest (not async Batch API for real-time)
- Cache hit/miss metrics via tracing

### Non-Functional
- Cache TTL default: 24h (configurable)
- Max batch size: 20 requests (configurable)
- Zero overhead when batch size = 1

## Architecture

```
DAG Executor Layer N (parallel stages)
    |
    +-- Stage A needs LLM call --> BatchCollector.add(prompt_a)
    +-- Stage B needs LLM call --> BatchCollector.add(prompt_b)
    +-- Stage C needs LLM call --> BatchCollector.add(prompt_c) [same as A]
    |
    v
BatchCollector.flush()
    |
    +-- Deduplicate: prompt_c == prompt_a (cache key match)
    +-- Check cache: prompt_a cached? Return cached.
    +-- Send remaining: [prompt_a, prompt_b] concurrently via provider
    +-- Store responses in cache
    +-- Return results to stages
```

## Related Code Files

### Modify
- `crates/agent/src/llm/mod.rs` — add `batch_chat` to LlmProvider, add BatchCollector
- `crates/agent/src/llm/openai.rs` — implement batch_chat (concurrent reqwest)
- `crates/agent/src/llm/anthropic.rs` — implement batch_chat
- `crates/agent/src/engine/dag_executor.rs` — integrate BatchCollector in layer execution
- `crates/core/src/config.rs` — add LlmCacheConfig

### Create
- `crates/agent/src/llm/cache.rs` — LLM response cache (SQLite)
- `crates/agent/src/llm/batch_collector.rs` — request batching + deduplication

## Implementation Steps

1. Add `LlmCacheConfig` to config.rs:
   ```rust
   pub struct LlmCacheConfig {
       pub enabled: bool,        // default true
       pub ttl_secs: u64,        // default 86400 (24h)
       pub max_entries: usize,   // default 10000
   }
   ```
2. Create `cache.rs` with `LlmCache`:
   - SQLite table: `llm_cache (cache_key TEXT PK, response TEXT, created TEXT, model TEXT)`
   - `cache_key` = SHA-256 of `model + messages_json + opts_json`
   - `get(key) -> Option<String>`, `put(key, response)`, `prune()` for TTL cleanup
3. Extend `LlmProvider` trait with default `batch_chat`:
   ```rust
   async fn batch_chat(&self, requests: &[(Vec<Message>, ChatOpts)]) -> Result<Vec<String>> {
       // Default: sequential fallback
       let mut results = Vec::new();
       for (msgs, opts) in requests {
           results.push(self.chat(msgs, opts).await?);
       }
       Ok(results)
   }
   ```
4. Create `batch_collector.rs`:
   - `add(messages, opts) -> RequestId`
   - `flush(provider, cache) -> HashMap<RequestId, String>`
   - Deduplication by cache key
   - Concurrent execution via `futures::join_all` for non-cached requests
5. Implement `batch_chat` in OpenAI provider using concurrent reqwest tasks
6. Implement `batch_chat` in Anthropic provider similarly
7. Integrate BatchCollector in DagExecutor layer execution
8. Add cache hit/miss tracing logs

## Todo List
- [x]Add LlmCacheConfig to config.rs
- [x]Create LlmCache in cache.rs
- [x]Add batch_chat default to LlmProvider trait
- [x]Create BatchCollector
- [x]Implement OpenAI batch_chat
- [x]Implement Anthropic batch_chat
- [x]Integrate in DagExecutor
- [x]Add tests (cache hit/miss, dedup, batch)

## Success Criteria
- Duplicate prompts in same pipeline run only call LLM once
- Cached responses returned without API call
- Batch of N requests completes faster than N sequential calls
- Existing single-request tests pass unchanged

## Risk Assessment
- **Rate limiting**: Concurrent batch may hit API rate limits. Mitigate with configurable concurrency limit.
- **Cache staleness**: Stale responses for changed contexts. Mitigate with TTL + model in cache key.
- **Memory**: Large batch responses. Mitigate with max_entries + prune.

## Security Considerations
- Cache stores LLM responses in SQLite; may contain sensitive note content
- Cache DB inherits vault directory permissions (0600)
- Cache key includes model name to prevent cross-model contamination

## Next Steps
- Independent of other phases
- Future: OpenAI async Batch API for bulk offline processing
