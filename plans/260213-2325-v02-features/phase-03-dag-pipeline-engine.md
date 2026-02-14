# Phase 3: DAG Pipeline Engine

## Context Links
- [Research: DAG Pipelines](/Users/phuc/Developer/agentic-note/plans/260213-2325-v02-features/research/researcher-embeddings-dag-plugins.md)
- [Current executor](/Users/phuc/Developer/agentic-note/crates/agent/src/engine/executor.rs)
- [Current pipeline config](/Users/phuc/Developer/agentic-note/crates/agent/src/engine/pipeline.rs)
- [Plan Overview](plan.md)

## Overview
- **Priority:** P1
- **Status:** completed
- **Effort:** 4h
- **Depends on:** Phase 1
- **Description:** Replace sequential pipeline execution with petgraph DAG. Parallel stages within layers via `tokio::join_all`. Conditional stages. TOML schema v2 with auto-upgrade from v1.

## Key Insights
- Current `StageExecutor::run_pipeline()` iterates `pipeline.stages` sequentially (line 70 of executor.rs)
- petgraph `toposort()` gives execution layers; stages in same layer run in parallel
- `depends_on: Vec<String>` field on StageConfig — empty means root node
- `condition: Option<String>` — simple expression like `classify.output.para == "projects"`
- v1 TOML has no `schema_version` or `depends_on`; auto-upgrade adds sequential deps

## Requirements

### Functional
- F1: `PipelineConfig` gains `schema_version` field (default 1)
- F2: `StageConfig` gains `depends_on: Vec<String>` and `condition: Option<String>`
- F3: `DagExecutor` builds petgraph DiGraph from stages, validates no cycles
- F4: Topological sort produces execution layers; stages in same layer run via `tokio::join_all`
- F5: Condition evaluator: simple `stage.output.field == "value"` expressions
- F6: Auto-upgrade v1 -> v2: each stage depends on previous stage
- F7: Cycle detection at load time with clear error message

### Non-Functional
- Backward compatible: v1 TOML files work without changes
- DAG build + toposort <1ms for 20-stage pipelines
- StageContext must be cloneable for parallel execution (already Clone)

## Architecture

```
crates/agent/src/engine/
├── mod.rs          # modify: re-export DagExecutor
├── pipeline.rs     # modify: +schema_version, +depends_on, +condition on StageConfig
├── executor.rs     # modify: rename to sequential fallback, extract shared logic
├── dag_executor.rs # NEW: petgraph DAG builder + parallel executor
├── condition.rs    # NEW: simple condition evaluator
├── migration.rs    # NEW: v1 -> v2 auto-upgrade
├── context.rs      # unchanged (already Clone)
└── trigger.rs      # unchanged
```

### DAG Execution Flow
```
Load TOML → migrate v1→v2 if needed → build DiGraph
  → toposort → group into layers
  → for each layer:
      parallel: tokio::join_all(stages in layer)
      each stage: check condition → execute agent → store output
  → collect PipelineResult
```

## Related Code Files

| File | Action | Changes |
|------|--------|---------|
| `/Users/phuc/Developer/agentic-note/crates/agent/Cargo.toml` | modify | +petgraph dep |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/mod.rs` | modify | +mod dag_executor, condition, migration; re-exports |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/pipeline.rs` | modify | +schema_version, +depends_on, +condition fields |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/dag_executor.rs` | create | DagExecutor with petgraph |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/condition.rs` | create | Condition evaluation |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/migration.rs` | create | v1->v2 schema upgrade |
| `/Users/phuc/Developer/agentic-note/crates/agent/src/engine/executor.rs` | modify | Keep as-is for backward compat, DagExecutor delegates to it for single-stage |

## Implementation Steps

1. Add `petgraph = { workspace = true }` to `crates/agent/Cargo.toml`.
2. Modify `crates/agent/src/engine/pipeline.rs`:
   - Add `schema_version: u32` (default 1) to `PipelineConfig`
   - Add `depends_on: Vec<String>` (default empty) to `StageConfig`
   - Add `condition: Option<String>` (default None) to `StageConfig`
   - Update `PipelineConfig::load()` to call `migration::migrate_v1_to_v2()` when schema_version=1
3. Create `crates/agent/src/engine/migration.rs`:
   - `migrate_v1_to_v2(config: &mut PipelineConfig)` — for each stage[i>0], set `depends_on = [stages[i-1].name]`; set `schema_version = 2`
4. Create `crates/agent/src/engine/dag_executor.rs`:
   - `DagExecutor` struct wrapping `StageExecutor` (reuse handler registry)
   - `build_dag(stages: &[StageConfig]) -> Result<DiGraph<usize, ()>>` — node per stage, edge per dependency
   - Validate: `petgraph::algo::toposort()` — if Err, return cycle error
   - `compute_layers(graph: &DiGraph, order: &[NodeIndex]) -> Vec<Vec<usize>>` — group stages by depth
   - `run_pipeline()`:
     ```rust
     for layer in layers {
         let futures = layer.iter().map(|idx| {
             let stage = &pipeline.stages[*idx];
             // check condition
             // clone ctx for parallel, execute
         });
         let results = tokio::join_all(futures).await;
         // merge outputs into shared context
     }
     ```
   - Handle: condition check before execution, skip if false
   - After all layers: build PipelineResult from accumulated outputs/skips/warnings
5. Create `crates/agent/src/engine/condition.rs`:
   - `evaluate_condition(expr: &str, outputs: &HashMap<String, Value>) -> Result<bool>`
   - Parse `stage_name.output.field == "value"` pattern
   - Support: `==`, `!=`, simple string comparison
   - Unsupported expressions return error (not silently true)
6. Modify `crates/agent/src/engine/mod.rs`:
   - Add modules, re-export `DagExecutor`
   - Update `AgentSpace` to use `DagExecutor` for v2 pipelines, `StageExecutor` for v1
7. Write tests:
   - DAG with parallel stages (A, B independent → C depends on both)
   - Cycle detection (A→B→A)
   - v1 migration produces correct sequential deps
   - Condition evaluation (true, false, invalid expression)
   - Parallel execution actually runs concurrently (timing-based check)

## Todo List

- [ ] Add petgraph dep
- [ ] Add schema_version, depends_on, condition to pipeline/stage configs
- [ ] Implement migration.rs (v1 -> v2)
- [ ] Implement condition.rs (expression evaluator)
- [ ] Implement dag_executor.rs (build DAG, toposort, parallel layers)
- [ ] Update AgentSpace to route v1/v2 pipelines
- [ ] Update mod.rs re-exports
- [ ] Tests: parallel DAG execution
- [ ] Tests: cycle detection
- [ ] Tests: v1 migration
- [ ] Tests: condition evaluation
- [ ] cargo check + cargo test pass

## Success Criteria
- v1 TOML files auto-upgrade and execute identically to before
- v2 TOML with parallel stages runs them concurrently
- Cycle in DAG produces clear error at load time
- Condition `false` skips stage cleanly

## Risk Assessment
- **Medium:** Parallel stage execution needs careful context cloning — StageContext is Clone, but outputs must merge after parallel layer completes. Use `Arc<Mutex<HashMap>>` for shared output map.
- **Low:** petgraph is stable (0.6.x for years)

## Security Considerations
- Condition evaluator is NOT arbitrary code execution — only `==`/`!=` on JSON values
- No eval, no regex in conditions

## Next Steps
- Phase 4 (Error Recovery) builds on this executor
- Phase 6 (Plugins) registers plugin agents into the same handler registry
