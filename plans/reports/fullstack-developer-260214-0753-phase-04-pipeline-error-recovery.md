## Phase Implementation Report

### Executed Phase
- Phase: phase-04-pipeline-error-recovery
- Plan: /Users/phuc/Developer/agentic-note/plans/260213-2325-v02-features/
- Status: completed

### Files Modified

| File | Action | Lines |
|------|--------|-------|
| `crates/agent/src/engine/pipeline.rs` | modified | +18 lines (new fields + defaults) |
| `crates/agent/src/engine/error_policy.rs` | created | 284 lines |
| `crates/agent/src/engine/executor.rs` | modified | +55 lines (policy integration + errors field) |
| `crates/agent/src/engine/dag_executor.rs` | modified | +60 lines (policy integration + Aborted outcome) |
| `crates/agent/src/engine/mod.rs` | modified | +2 lines (mod + re-export) |
| `crates/agent/src/engine/migration.rs` | modified | +10 lines (test helper refactor) |

### Tasks Completed

- [x] Add error policy fields to StageConfig (on_error, retry_max, retry_backoff_ms, fallback_agent)
- [x] Add default_on_error to PipelineConfig
- [x] AgentConfig default_on_error already present from Phase 1 (no change needed)
- [x] Implement error_policy.rs (execute_with_policy, retry_with_backoff, try_fallback)
- [x] Add StageError struct
- [x] Update PipelineResult with errors: Vec<StageError>
- [x] Integrate into DagExecutor (StageOutcome::Aborted variant, break 'layers on abort)
- [x] Integrate into sequential StageExecutor (execute_with_policy replaces direct handler.execute)
- [x] Update mod.rs with re-exports
- [x] Tests: skip policy (success + failure)
- [x] Tests: retry succeeds after failures (FlakyAgent: fail 2x, succeed on 3rd)
- [x] Tests: retry exhausted returns None
- [x] Tests: abort stops pipeline (returns Err(StageError))
- [x] Tests: fallback agent succeeds when primary fails
- [x] Tests: fallback both fail returns None
- [x] cargo check pass
- [x] cargo test pass (28/28)

### Tests Status
- Type check: pass (`cargo check -p agentic-note-agent` clean)
- Unit tests: pass (28/28 tests in agentic-note-agent)
- Workspace tests: pass (all crates except agentic-note-sync which has pre-existing rand compatibility failure unrelated to this phase)

### Key Design Decisions

1. **Policy resolution**: Stage `on_error` overrides pipeline `default_on_error`. If stage is default (Skip) and pipeline has a different default, pipeline default wins. Avoids needing an extra `Option<ErrorPolicy>` wrapper.

2. **StageError cap**: `Vec<StageError>` capped at 100 entries per spec (non-functional requirement).

3. **Retry backoff**: `min(backoff_ms * 2^attempt, 30_000)` using `saturating_mul`/`saturating_pow` to avoid overflow. Zero backoff in tests for speed.

4. **DAG abort**: Uses labeled `break 'layers` to exit the layer loop immediately. Stages already-spawned in the current layer still complete (tokio tasks already running), but no further layers execute.

5. **Warning on Skip**: When `Ok(None)` is returned in sequential executor, a warning message is pushed to preserve backward-compatible behavior (existing test `failing_stage_is_skipped_not_fatal` asserted `!result.warnings.is_empty()`).

### Issues Encountered
- `migration.rs` test helper used struct literals without new fields → fixed with `make_stage()` helper
- `dag_executor.rs` test `make_pipeline` and `v1_migration_produces_sequential_deps` struct literals missing `default_on_error` → fixed
- Pre-existing `agentic-note-sync` compile error (rand `CryptoRng` trait bound, unrelated to this phase)

### Next Steps
- Phase 8 (Integration) can test error recovery with full DAG pipelines
- Consider adding abort-in-parallel-layer test (currently aborted tasks in same layer finish before break)
