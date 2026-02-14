# Phase 4: Pipeline Error Recovery

## Context Links
- [Research: Error Recovery](/Users/phuc/Developer/agentic-note/plans/260213-2325-v02-features/research/researcher-embeddings-dag-plugins.md)
- [Current executor](/Users/phuc/Developer/agentic-note/crates/agent/src/engine/executor.rs)
- [Phase 3: DAG Pipeline](phase-03-dag-pipeline-engine.md)
- [Plan Overview](plan.md)

## Overview
- **Priority:** P2
- **Status:** completed
- **Effort:** 3h
- **Depends on:** Phase 3 (DAG executor)
- **Description:** Per-stage error policies (skip/retry/abort/fallback), exponential backoff retry, fallback agent, error accumulator in PipelineResult.

## Key Insights
- Current behavior: stage fails → log warning, skip, continue (hardcoded in executor.rs lines 88-96)
- `ErrorPolicy` enum already defined in Phase 1 (core/types.rs)
- Retry with exponential backoff: `tokio::time::sleep(Duration::from_millis(base * 2^attempt))`
- Fallback agent: resolve from handler registry, same StageContext
- Error accumulator: `Vec<StageError>` with stage name, attempt count, error message

## Requirements

### Functional
- F1: `StageConfig` gains `on_error: ErrorPolicy` (default Skip)
- F2: `StageConfig` gains `retry_max: u32` (default 3), `retry_backoff_ms: u64` (default 1000)
- F3: `StageConfig` gains `fallback_agent: Option<String>`
- F4: `PipelineConfig` gains `default_on_error: ErrorPolicy` (default Skip)
- F5: Skip policy: log + continue (current behavior)
- F6: Retry policy: exponential backoff, up to retry_max attempts
- F7: Abort policy: stop pipeline immediately, return partial result
- F8: Fallback policy: try fallback_agent, if that fails too → Skip
- F9: `StageError` struct in PipelineResult with stage_name, attempts, error, policy_applied
- F10: Global default from config.toml: `[agent] default_on_error = "skip"`

### Non-Functional
- Retry backoff capped at 30s max per attempt
- Error accumulator doesn't grow unbounded (max 100 entries)

## Architecture

```
crates/agent/src/engine/
├── dag_executor.rs   # modify: integrate error policies into stage execution
├── error_policy.rs   # NEW: retry logic, fallback dispatch, policy application
├── executor.rs       # modify: also support error policies in sequential mode
└── pipeline.rs       # modify: +on_error, retry, fallback fields
```

## Related Code Files

| File | Action | Changes |
|------|--------|---------|
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/pipeline.rs` | modify | +on_error, retry_max, retry_backoff_ms, fallback_agent on StageConfig; +default_on_error on PipelineConfig |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/error_policy.rs` | create | execute_with_policy(), retry_with_backoff(), try_fallback() |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/dag_executor.rs` | modify | Call execute_with_policy() instead of direct handler.execute() |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/executor.rs` | modify | Same — use execute_with_policy() in sequential path |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/mod.rs` | modify | +mod error_policy, +StageError re-export |
| `/Users/phuc/Developer/agentic-note/crates/core/src/config.rs` | modify | +default_on_error field to AgentConfig |

## Implementation Steps

1. Add fields to `StageConfig` in `pipeline.rs`:
   ```rust
   #[serde(default)]
   pub on_error: ErrorPolicy,
   #[serde(default = "default_retry_max")]
   pub retry_max: u32,      // default 3
   #[serde(default = "default_retry_backoff")]
   pub retry_backoff_ms: u64,  // default 1000
   pub fallback_agent: Option<String>,
   ```
2. Add `default_on_error: ErrorPolicy` to `PipelineConfig` (serde default = Skip).
3. Add `default_on_error: ErrorPolicy` to `AgentConfig` in core config.rs.
4. Create `crates/agent/src/engine/error_policy.rs`:
   - `StageError` struct:
     ```rust
     pub struct StageError {
         pub stage_name: String,
         pub agent: String,
         pub attempts: u32,
         pub error: String,
         pub policy_applied: ErrorPolicy,
     }
     ```
   - `execute_with_policy()` function:
     ```rust
     pub async fn execute_with_policy(
         handler: &dyn AgentHandler,
         ctx: &mut StageContext,
         config: &toml::Value,
         stage: &StageConfig,
         handlers: &HashMap<String, Arc<dyn AgentHandler>>,
     ) -> Result<Option<Value>, StageError>
     ```
     - Match on `stage.on_error`:
       - `Skip` → try once, on error return Ok(None) + log
       - `Retry` → call `retry_with_backoff()`, on exhaust return Ok(None)
       - `Abort` → try once, on error return Err(StageError) (caller stops pipeline)
       - `Fallback` → try primary, on fail try fallback_agent, on both fail → Skip
   - `retry_with_backoff()`:
     ```rust
     for attempt in 0..max_retries {
         match handler.execute(ctx, config).await {
             Ok(v) => return Ok(Some(v)),
             Err(e) => {
                 let delay = min(backoff_ms * 2u64.pow(attempt), 30_000);
                 tokio::time::sleep(Duration::from_millis(delay)).await;
             }
         }
     }
     ```
5. Update `PipelineResult` in `executor.rs`:
   - Add `pub errors: Vec<StageError>` field
   - Replace ad-hoc `skipped`/`warnings` with structured `StageError` entries
6. Modify `dag_executor.rs` stage execution to call `execute_with_policy()`.
   - On `Abort` error: break out of layer loop, return partial result.
7. Modify `executor.rs` sequential path similarly.
8. Update `mod.rs` with re-exports.
9. Write tests:
   - Retry: agent fails 2x then succeeds on 3rd → completes
   - Retry exhausted → skipped
   - Abort: first failure stops pipeline
   - Fallback: primary fails, fallback succeeds
   - Fallback: both fail → skip

## Todo List

- [ ] Add error policy fields to StageConfig and PipelineConfig
- [ ] Add default_on_error to AgentConfig
- [ ] Implement error_policy.rs (execute_with_policy, retry, fallback)
- [ ] Add StageError struct
- [ ] Update PipelineResult with errors field
- [ ] Integrate into DagExecutor
- [ ] Integrate into sequential StageExecutor
- [ ] Tests: retry succeeds after failures
- [ ] Tests: retry exhausted
- [ ] Tests: abort stops pipeline
- [ ] Tests: fallback agent
- [ ] cargo check + cargo test pass

## Success Criteria
- Stage with `on_error = "retry"` retries 3x with exponential backoff
- Stage with `on_error = "abort"` stops entire pipeline
- Fallback agent runs when primary fails
- PipelineResult.errors contains structured error info
- Existing v1 pipelines still work (ErrorPolicy defaults to Skip)

## Risk Assessment
- **Low:** Retry with sleep is straightforward tokio pattern
- **Medium:** Fallback agent lookup requires access to handler registry — pass reference through

## Security Considerations
- Backoff prevents tight retry loops (min 1s, max 30s)
- No credentials exposed in StageError (just error message string)

## Next Steps
- Phase 8 (Integration) tests error recovery with DAG pipelines
