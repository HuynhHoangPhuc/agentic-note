---
phase: 7
title: "Metrics & Observability"
status: complete
effort: 2h
depends_on: [1]
---

## Context Links

- [CLI main.rs](../../crates/cli/src/main.rs)
- [CLI commands/mod.rs](../../crates/cli/src/commands/mod.rs)
- [Core config.rs](../../crates/core/src/config.rs)
- [Research: Metrics Crates](research/researcher-metrics-workers.md)

## Overview

<!-- Updated: Validation Session 1 - No TUI, simple CLI table + prometheus exporter -->
Add `metrics` 0.24 facade with macros across all crates. Feature-gated `metrics-exporter-prometheus` for scrape endpoint. Simple `metrics show` CLI table (no ratatui). Track: note ops, pipeline duration, sync duration, indexing latency.

## Key Insights

- `metrics` facade is minimal (macros only, no runtime cost without exporter)
- `metrics-exporter-prometheus` behind feature flag to avoid binary bloat
- TUI dashboard deferred to v0.4 (validated: KISS for v0.3)
- Existing `tracing` + `indicatif` stay for logs/progress; metrics is orthogonal
- `tokio-metrics` 0.3 gives runtime task poll latency for free

## Requirements

**Functional:**
- Instrument key operations with `counter!`, `histogram!`, `gauge!` macros
- Metrics tracked:
  - `note_operations_total` (counter, labels: operation=create|read|update|delete)
  - `pipeline_execution_duration_seconds` (histogram, labels: pipeline_name)
  - `pipeline_stage_duration_seconds` (histogram, labels: stage_name)
  - `sync_duration_seconds` (histogram, labels: peer_id)
  - `sync_bytes_transferred` (counter, labels: direction=sent|received)
  - `indexer_batch_duration_seconds` (histogram)
  - `indexer_files_processed_total` (counter)
  - `notes_total` (gauge, current note count)
<!-- Updated: Validation Session 1 - Defer TUI to v0.4, use simple CLI table -->
- CLI: `metrics show` -- formatted table of current metrics
- CLI: `--metrics` global flag to enable prometheus exporter on `127.0.0.1:9091`

**Non-functional:**
- Zero overhead when no exporter installed (facade pattern)
- Prometheus exporter adds < 2MB to binary

## Architecture

```
All crates: metrics::counter!(), metrics::histogram!()
  │
  ├── No exporter installed → macros are no-ops
  │
  ├── --metrics flag → PrometheusExporter on 127.0.0.1:9091
  │     └── GET /metrics → Prometheus scrape format
  │
  └── metrics show → formatted CLI table (reads from registry)
```

## Related Code Files

**Create:**
- `crates/cli/src/commands/metrics_cmd.rs` -- `metrics watch` TUI command
- `crates/cli/src/metrics_init.rs` -- exporter initialization

**Modify:**
- `crates/cli/src/main.rs` -- add `--metrics` flag, init exporter, add `Metrics` command
- `crates/cli/src/commands/mod.rs` -- add `Metrics` command variant
- `crates/cli/Cargo.toml` -- add metrics deps (some feature-gated)
- `crates/search/src/background_indexer.rs` -- instrument indexer batch
- `crates/agent/src/engine/dag_executor.rs` -- instrument pipeline/stage duration
- `crates/sync/src/lib.rs` -- instrument sync duration
- `crates/vault/src/note.rs` -- instrument note CRUD operations
- Root `Cargo.toml` -- add workspace deps

## Implementation Steps

1. Add workspace deps to root `Cargo.toml`:
   ```toml
   metrics = "0.24"
   tokio-metrics = "0.3"
   metrics-exporter-prometheus = { version = "0.16", optional = true }
   ```

2. Add to `crates/cli/Cargo.toml`:
   ```toml
   [dependencies]
   metrics = { workspace = true }
   tokio-metrics = { workspace = true }

   [features]
   default = []
   prometheus = ["metrics-exporter-prometheus"]

   [dependencies.metrics-exporter-prometheus]
   workspace = true
   optional = true
   ```

3. Add `metrics` to crates that emit metrics:
   - `crates/vault/Cargo.toml` -- `metrics = { workspace = true }`
   - `crates/agent/Cargo.toml` -- `metrics = { workspace = true }`
   - `crates/sync/Cargo.toml` -- `metrics = { workspace = true }`
   - `crates/search/Cargo.toml` -- `metrics = { workspace = true }`

4. Create `crates/cli/src/metrics_init.rs`:
   ```rust
   pub fn install_prometheus_exporter(port: u16) -> Result<()> {
       #[cfg(feature = "prometheus")]
       {
           let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
           builder.with_http_listener(([127, 0, 0, 1], port))
               .install()
               .map_err(|e| AgenticError::Metrics(format!("{e}")))?;
       }
       Ok(())
   }
   ```

5. Instrument `crates/vault/src/note.rs`:
   ```rust
   pub fn create(...) -> Result<Note> {
       metrics::counter!("note_operations_total", "operation" => "create").increment(1);
       let start = std::time::Instant::now();
       // ... existing logic ...
       metrics::histogram!("note_operation_duration_seconds", "operation" => "create")
           .record(start.elapsed().as_secs_f64());
       Ok(note)
   }
   ```

6. Instrument `crates/agent/src/engine/dag_executor.rs`:
   - Record `pipeline_execution_duration_seconds` around `run_pipeline()`
   - Record `pipeline_stage_duration_seconds` around each stage in `run_stage()`

7. Instrument `crates/sync/src/lib.rs`:
   - Record `sync_duration_seconds` around `sync_with_peer()`
   - Record `sync_bytes_transferred` for delta payloads

8. Instrument `crates/search/src/background_indexer.rs`:
   - Record `indexer_batch_duration_seconds` per flush
   - Increment `indexer_files_processed_total` per file

9. Add `--metrics` global flag to CLI:
   ```rust
   #[arg(long, global = true)]
   metrics: bool,
   ```
   - If set: call `metrics_init::install_prometheus_exporter(config.metrics.prometheus_port)`

10. Create `crates/cli/src/commands/metrics_cmd.rs`:
    - `pub fn show() -> Result<()>` -- print formatted table of current metrics
    - Read from metrics registry: note counts, pipeline stats, sync stats
    - Format as human-readable table (no ratatui dependency)

11. Add `Metrics` to `Commands` enum, dispatch in main.rs.

12. Run `cargo check -p agentic-note-cli --features prometheus`.

13. Unit test: verify metrics macros compile without exporter (no-op mode).

14. Integration test: start exporter, perform note create, scrape /metrics, verify counter incremented.

## Todo List

- [ ] Add workspace deps (metrics, tokio-metrics, feature-gated prometheus)
- [ ] Create `metrics_init.rs` -- exporter setup
- [ ] Instrument vault note operations
- [ ] Instrument DAG executor pipeline/stage duration
- [ ] Instrument sync duration and bytes
- [ ] Instrument background indexer
- [ ] Add `--metrics` CLI flag
- [ ] Create `metrics_cmd.rs` CLI table (metrics show)
- [ ] Unit test: metrics compile in no-op mode
- [ ] Integration test: prometheus scrape
- [ ] `cargo check` passes (with and without features)

## Success Criteria

- Metrics macros compile and work as no-ops without exporter feature
- With `--metrics` flag: prometheus endpoint serves valid scrape data
- All key operations instrumented (note ops, pipeline, sync, indexer)
- Binary size increase < 2MB with prometheus feature

## Risk Assessment

- **metrics 0.24 compatibility**: verify exporter 0.16 matches facade 0.24 API
- **Feature flag combinations**: test `--features prometheus` independently

## Security Considerations

- Prometheus endpoint binds to `127.0.0.1` only (not 0.0.0.0)
- No sensitive data in metrics labels (no note content, no API keys)
- Endpoint disabled by default (opt-in via `--metrics`)

## Next Steps

Phase 8 (Integration) validates metrics across all features.
