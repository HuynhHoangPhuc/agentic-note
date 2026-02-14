# Phase 22: Prometheus Integration

## Context Links
- [plan.md](plan.md)
- [crates/cli/src/metrics_init.rs](/Users/phuc/Developer/agentic-note/crates/cli/src/metrics_init.rs) — current stub
- [crates/cli/src/commands/metrics_cmd.rs](/Users/phuc/Developer/agentic-note/crates/cli/src/commands/metrics_cmd.rs) — CLI commands
- [crates/core/src/config.rs](/Users/phuc/Developer/agentic-note/crates/core/src/config.rs) — MetricsConfig

## Overview
- **Priority:** P2
- **Status:** Complete
- **Implementation Status:** complete
- **Review Status:** complete
- **Effort:** 2h
- **Description:** Replace metrics stubs with prometheus-client crate. Expose `/metrics` endpoint via minimal hyper HTTP server. Key metrics: pipeline latency, search latency, sync duration, note count.

## Key Insights
- Current `metrics_init.rs` is a no-op stub (14 LOC)
- `MetricsConfig` already exists with `enabled` and `prometheus_port` fields
- `prometheus-client` crate (official Rust client) replaces existing `metrics = "0.24"` crate
- Remove `metrics` from workspace Cargo.toml; all metric recording via prometheus-client registry
- Minimal hyper server on localhost only (no external exposure)
- Background tokio task serves `/metrics` endpoint
<!-- Updated: Validation Session 1 - Switch from metrics crate to prometheus-client confirmed -->

## Requirements

### Functional
- Replace `install_metrics_recorder()` with actual prometheus-client registry
- Expose HTTP GET `/metrics` on `localhost:{prometheus_port}`
- Instrument these metrics:
  - `pipeline_execution_duration_seconds` (histogram, labels: pipeline_name, status)
  - `pipeline_stage_duration_seconds` (histogram, labels: pipeline, stage, agent)
  - `search_query_duration_seconds` (histogram, labels: mode)
  - `sync_duration_seconds` (histogram, labels: peer_id, status)
  - `notes_total` (gauge)
  - `llm_requests_total` (counter, labels: provider, status)
  - `llm_cache_hits_total` (counter)
  - `review_queue_pending` (gauge)
- CLI `metrics show` reads from prometheus registry (not stub)

### Non-Functional
- Endpoint binds localhost only (127.0.0.1)
- <1ms overhead per metric recording
- Graceful shutdown when CLI exits

## Architecture

```
CLI main.rs
    |
    +-- metrics_init::start_metrics_server(config)
    |       |
    |       +-- Creates prometheus_client::registry::Registry
    |       +-- Spawns tokio task with hyper HTTP server
    |       +-- Returns MetricsHandle (for recording + shutdown)
    |
    +-- Pass MetricsHandle to DagExecutor, SearchEngine, SyncEngine
    |
    +-- On exit: MetricsHandle.shutdown()
```

## Related Code Files

### Modify
- `Cargo.toml` — add prometheus-client, hyper, http-body-util, hyper-util
- `crates/cli/Cargo.toml` — add prometheus-client, hyper deps
- `crates/cli/src/metrics_init.rs` — full rewrite: prometheus registry + HTTP server
- `crates/cli/src/commands/metrics_cmd.rs` — read from real registry
- `crates/cli/src/main.rs` — initialize metrics server, pass handle
- `crates/agent/src/engine/dag_executor.rs` — record pipeline/stage duration
- `crates/agent/src/llm/mod.rs` — record LLM request count
- `crates/search/src/lib.rs` — record search duration
- `crates/sync/src/lib.rs` — record sync duration

### Create
- `crates/cli/src/metrics_handle.rs` — MetricsHandle struct with metric accessors

## Implementation Steps

1. Add workspace dependencies:
   ```toml
   prometheus-client = "0.23"
   hyper = { version = "1", features = ["server", "http1"] }
   http-body-util = "0.1"
   hyper-util = { version = "0.1", features = ["tokio"] }
   ```
2. Create `metrics_handle.rs` with `MetricsHandle`:
   ```rust
   pub struct MetricsHandle {
       pub registry: Arc<Mutex<Registry>>,
       pub pipeline_duration: Family<PipelineLabels, Histogram>,
       pub search_duration: Family<SearchLabels, Histogram>,
       pub sync_duration: Family<SyncLabels, Histogram>,
       pub notes_total: Gauge,
       pub llm_requests: Family<LlmLabels, Counter>,
       pub llm_cache_hits: Counter,
       pub review_pending: Gauge,
       shutdown_tx: Option<oneshot::Sender<()>>,
   }
   ```
3. Rewrite `metrics_init.rs`:
   - Create Registry, register all metrics
   - Spawn hyper server on `127.0.0.1:{port}` serving `/metrics`
   - Handler encodes registry to OpenMetrics text format
   - Return MetricsHandle
4. Update `metrics_cmd.rs` to read from MetricsHandle (format for CLI display)
5. Instrument DagExecutor: record pipeline + stage durations after execution
6. Instrument LlmProvider: increment counter on each chat call
7. Instrument SearchEngine: record query duration
8. Instrument SyncEngine: record sync duration
9. Pass MetricsHandle through CLI -> subsystems (or use global Arc)

## Todo List
- [x]Add prometheus-client + hyper deps
- [x]Create MetricsHandle struct
- [x]Rewrite metrics_init.rs with HTTP server
- [x]Update metrics_cmd.rs for real data
- [x]Instrument pipeline execution
- [x]Instrument LLM requests
- [x]Instrument search queries
- [x]Instrument sync operations
- [x]Add tests (metric recording, HTTP endpoint)

## Success Criteria
- `curl localhost:9091/metrics` returns valid OpenMetrics text
- Pipeline execution records duration histogram
- `metrics show` CLI displays real values
- Disabled by default (`metrics.enabled = false`); no server spawned when disabled
- Graceful shutdown (no leaked tasks)

## Risk Assessment
- **Port conflict**: Port 9091 may be in use. Config allows custom port.
- **hyper complexity**: hyper v1 API verbose. Keep handler minimal (~30 LOC).
- **Thread safety**: Registry behind Arc<Mutex>. Low contention (writes are fast).

## Security Considerations
- Bind to 127.0.0.1 only (never 0.0.0.0)
- No authentication on /metrics (localhost only)
- No sensitive data in metric labels (no note content, no API keys)

## Next Steps
- Independent of other phases
- Future: push-based metrics (remote write) for headless deployments
