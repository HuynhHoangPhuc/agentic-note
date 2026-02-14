---
phase: 1
title: "Core Types & Config Extensions"
status: complete
effort: 1h
depends_on: []
---

## Context Links

- [Core types.rs](../../crates/core/src/types.rs)
- [Core error.rs](../../crates/core/src/error.rs)
- [Core config.rs](../../crates/core/src/config.rs)
- [Research: Metrics & Workers](research/researcher-metrics-workers.md)

## Overview

Extend core crate with new types, error variants, and config sections needed by all v0.3.0 features. This phase unblocks all others.

## Key Insights

- Existing `AgenticError` needs `Metrics`, `Scheduler`, `Indexer` variants
- Config needs sections for `[scheduling]`, `[metrics]`, `[indexer]`
- `ConflictPolicy` needs `SemanticMerge` variant for tiered merge
- Keep additions minimal -- only what downstream phases require

## Requirements

**Functional:**
- New error variants for metrics, scheduling, indexing failures
- Config structs for scheduling (cron expression, watch paths), metrics (exporter toggle, port), indexer (debounce, batch size)
- `ConflictPolicy::SemanticMerge` variant
- `SyncConfig` gets `delta_enabled: bool`, `compression_level: i32`

**Non-functional:**
- All new types derive `Debug, Clone, Serialize, Deserialize`
- Defaults for all new config fields (backward compatible)
- Zero breaking changes to existing API

## Architecture

No new modules. Extend existing files only.

## Related Code Files

**Modify:**
- `crates/core/src/types.rs` -- add `ConflictPolicy::SemanticMerge`
- `crates/core/src/error.rs` -- add error variants
- `crates/core/src/config.rs` -- add config sections
- `crates/core/src/lib.rs` -- re-export new types if needed

## Implementation Steps

1. Add error variants to `AgenticError` in `crates/core/src/error.rs`:
   ```rust
   #[error("Metrics error: {0}")]
   Metrics(String),
   #[error("Scheduler error: {0}")]
   Scheduler(String),
   #[error("Indexer error: {0}")]
   Indexer(String),
   ```

2. Add `SemanticMerge` to `ConflictPolicy` in `crates/core/src/types.rs`:
   ```rust
   pub enum ConflictPolicy {
       NewestWins,
       LongestWins,
       MergeBoth,
       SemanticMerge, // NEW: tiered diffy + LLM merge
       Manual,
   }
   ```

3. Add config structs in `crates/core/src/config.rs`:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct SchedulerConfig {
       pub enabled: bool,
       pub default_cron: Option<String>,
       pub watch_debounce_ms: u64, // default: 500
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MetricsConfig {
       pub enabled: bool,
       pub prometheus_port: u16, // default: 9091
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct IndexerConfig {
       pub background: bool,     // default: true
       pub debounce_ms: u64,     // default: 200
       pub batch_size: usize,    // default: 50
   }
   ```

4. Extend `SyncConfig` with delta fields:
   ```rust
   pub struct SyncConfig {
       // existing fields...
       pub delta_enabled: bool,        // default: true
       pub compression_level: i32,     // default: 3 (zstd)
   }
   ```

5. Add new config sections to `AppConfig`:
   ```rust
   pub struct AppConfig {
       // existing fields...
       pub scheduler: SchedulerConfig,
       pub metrics: MetricsConfig,
       pub indexer: IndexerConfig,
   }
   ```

6. Implement `Default` for all new config structs with sensible values.

7. Run `cargo check -p agentic-note-core` to verify compilation.

8. Update existing test in `config.rs` to include new sections.

## Todo List

- [ ] Add 3 new error variants
- [ ] Add `SemanticMerge` to `ConflictPolicy`
- [ ] Create `SchedulerConfig` struct + Default
- [ ] Create `MetricsConfig` struct + Default
- [ ] Create `IndexerConfig` struct + Default
- [ ] Extend `SyncConfig` with delta fields
- [ ] Add new sections to `AppConfig`
- [ ] Update deserialization test
- [ ] `cargo check` passes

## Success Criteria

- `cargo check` passes for all workspace crates
- Existing tests still pass
- All new config fields have defaults (no breaking changes to existing config.toml)

## Risk Assessment

- **Low risk**: additive-only changes, no behavior changes
- Downstream crates will not compile if types are wrong -- caught immediately

## Security Considerations

- No secrets in new config fields
- `compression_level` validated to zstd range (1-22)

## Next Steps

Unblocks Phases 2-7.
