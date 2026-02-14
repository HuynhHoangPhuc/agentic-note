# Research: Metrics, Background Workers & Semantic Merge — Rust (v0.3.0)

Date: 2026-02-14 | Context: agentic-note (local-first CLI app, tokio runtime, rusqlite, tantivy, iroh)

---

## 1. Metrics & Observability

### Crate Options

| Crate | Version | Role | Weight |
|---|---|---|---|
| `metrics` | 0.24 | Facade (macros only, no exporter) | Minimal |
| `metrics-exporter-prometheus` | 0.16 | Scrape endpoint or file dump | Light |
| `prometheus` | 0.14 | Direct client library | Medium |
| `opentelemetry` + `opentelemetry-otlp` | 0.27 | Full traces + metrics pipeline | Heavy |
| `tokio-metrics` | 0.3 | Tokio runtime/task internals | Minimal |

### Comparison

| | `metrics` facade | `prometheus` direct | `opentelemetry` |
|---|---|---|---|
| API verbosity | Low (macros) | Medium | High |
| Binary size impact | Minimal | Small | Large (~2 MB) |
| Traces support | No | No | Yes |
| Best fit for CLI | **Yes** | Possible | No |
| Async-native | Yes (tokio feature) | No (sync) | Yes |
| Ecosystem maturity | Stable | Stable | Beta (Rust SDK) |

### Recommendation: `metrics` facade + `metrics-exporter-prometheus`

For a CLI tool (not a daemon), full OTel is overkill. Use:
```toml
metrics = "0.24"
metrics-exporter-prometheus = "0.16"   # optional feature flag
tokio-metrics = "0.3"                  # runtime/task visibility
```

- Use `metrics::counter!`, `metrics::histogram!` macros everywhere.
- On `--metrics` flag: install prometheus exporter to write to a local `.prom` file or serve on `127.0.0.1:9091` for scrape by local Prometheus/Grafana.
- `tokio-metrics` gives task stall/poll latency without extra instrumentation.
- Existing `tracing` crate already in workspace — add `tracing-subscriber` JSON layer for structured logs; no OTel needed.

### Dashboard Options

| Option | Crate | Fit |
|---|---|---|
| TUI live view | `ratatui` 0.29 | Good for interactive `watch` command |
| Web scrape | prometheus + Grafana | Overkill for local CLI |
| Simple stdout | `indicatif` (already in workspace) | Already present — reuse |

Recommendation: `ratatui` for an optional `agentic-note metrics watch` subcommand; `indicatif` for inline progress. No separate web dashboard.

---

## 2. Background Worker Patterns

### Core Pattern: Tokio + mpsc + CancellationToken

```
CLI command
    │ send IndexTask via mpsc::channel
    ▼
BackgroundIndexer (tokio::spawn)
    ├── select! { task_rx, notify_rx, shutdown }
    ├── debounce pending paths (tokio::time::sleep)
    └── run tantivy indexer batch
```

### File Watcher Integration

```toml
notify = "7"                       # cross-platform FS events
notify-debouncer-full = "0.4"      # built-in debounce + tokio channel bridge
tokio-util = "0.7"                 # CancellationToken, TaskTracker
```

`notify-debouncer-full` wraps `notify` and delivers debounced events into a `tokio::sync::mpsc` channel natively — no manual bridging needed.

### Graceful Shutdown Pattern

```rust
// Standard pattern (no extra crate needed)
let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

tokio::select! {
    _ = worker_loop(&mut rx) => {}
    _ = shutdown_rx.changed() => { flush_and_exit().await; }
}
```

`tokio_util::task::TaskTracker` + `CancellationToken` (from `tokio-util`) is the canonical 2025 approach — zero extra dependencies beyond `tokio-util` (already used transitively).

### Debounce / Batching for Indexer

- Collect paths into a `HashSet<PathBuf>` for 200ms window using `tokio::time::sleep`.
- On timeout OR batch size > 50 files: flush batch to tantivy writer.
- Prevents re-indexing same file multiple times on rapid saves.

### Crates Summary

| Crate | Version | Purpose |
|---|---|---|
| `notify-debouncer-full` | 0.4 | FS events → debounced tokio channel |
| `tokio-util` | 0.7 | CancellationToken, TaskTracker |
| `tokio::sync::mpsc` | (stdlib) | Task queue |
| `tokio::sync::watch` | (stdlib) | Shutdown signal broadcast |

No `tokio-graceful-shutdown` crate needed — the tokio-util primitives are sufficient and already in the dependency tree.

---

## 3. Semantic Merge for Markdown Notes

### Problem Space

agentic-note uses iroh (P2P sync). Conflicts arise when the same note is edited offline on 2 devices. Need merge strategy beyond last-write-wins.

### Options

| Approach | Accuracy | Complexity | Latency |
|---|---|---|---|
| 3-way text diff (similar-cli, diffy) | Low (syntactic) | Low | <1ms |
| CRDT (yrs / automerge-rs) | High (structural) | High | <5ms |
| Embedding cosine similarity | Medium | Medium | 50–200ms (local) |
| LLM-assisted merge | High | Low to implement | 500ms–2s |

### Recommendation: Tiered Strategy

**Tier 1 — Syntactic (free):** Use `diffy` crate (0.4) for 3-way diff at paragraph level. If clean merge: auto-apply.

**Tier 2 — LLM-assisted (on conflict):** Pass both versions + ancestor to existing `crates/agent` infrastructure. Prompt: "merge these two versions of a markdown note, preserving intent of both edits." Output replaces conflicted block. This reuses existing agent code (KISS/DRY).

**Tier 3 — User resolution:** If agent confidence < threshold or offline: present unified diff in `ratatui` conflict viewer.

CRDT (yrs/automerge-rs) is overkill for a local-first app where iroh already handles P2P sync and CAS. Adding a CRDT layer duplicates iroh's content-addressing semantics.

### Crates

```toml
diffy = "0.4"   # 3-way merge, pure Rust, no deps
```

LLM merge uses `crates/agent` (already exists) — no new crates.

---

## Summary Recommendations

| Topic | Decision |
|---|---|
| Metrics | `metrics` 0.24 + `metrics-exporter-prometheus` 0.16 (feature-gated) |
| Runtime visibility | `tokio-metrics` 0.3 |
| TUI dashboard | `ratatui` 0.29 (optional `watch` subcommand) |
| File watcher | `notify-debouncer-full` 0.4 |
| Shutdown/task mgmt | `tokio-util` 0.7 CancellationToken (already transitive) |
| Semantic merge | `diffy` 0.4 + existing `crates/agent` LLM |

---

## Unresolved Questions

1. Does iroh 0.96 expose conflict events via its sync API, or must the app detect conflicts by comparing CAS hashes post-sync?
2. Is there a latency budget for background indexing that would make the 200ms debounce window too slow for UX (e.g., instant search after save)?
3. Should `metrics-exporter-prometheus` be always-on or only behind a compile feature flag to avoid binary bloat in release builds?
4. For the ratatui `watch` subcommand: does it replace the existing `indicatif` progress bars or run alongside them?

---

Sources:
- [Rust Observability: Logging, Tracing, and Metrics with OpenTelemetry and Tokio](https://dasroot.net/posts/2026/01/rust-observability-opentelemetry-tokio/)
- [How to setup and use metrics in Rust](https://www.hamzak.xyz/blog-posts/how-to-setup-and-use-metrics-in-rust)
- [tokio-graceful-shutdown crate](https://crates.io/crates/tokio-graceful-shutdown)
- [Tokio Graceful Shutdown docs](https://tokio.rs/tokio/topics/shutdown)
- [notify-debouncer-mini](https://lib.rs/crates/notify-debouncer-mini)
- [tokio-debouncer](https://crates.io/crates/tokio-debouncer)
- [ratatui GitHub](https://github.com/ratatui/ratatui)
- [Using LLMs to resolve merge conflicts (ACM ISSTA 2022)](https://dl.acm.org/doi/10.1145/3533767.3534396)
- [OT vs CRDT for real-time collaboration](https://www.tiny.cloud/blog/real-time-collaboration-ot-vs-crdt/)
- [CRDT for LLM agents (arXiv 2025)](https://arxiv.org/html/2510.18893)
