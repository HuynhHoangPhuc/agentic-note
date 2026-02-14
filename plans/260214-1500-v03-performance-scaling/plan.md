---
title: "v0.3.0 Performance & Scaling"
description: "Batch sync, delta compression, semantic merge, pipeline scheduling, background indexer, metrics"
status: complete
priority: P1
effort: 15h
branch: main
tags: [performance, sync, scheduling, metrics, indexing]
created: 2026-02-14
---

## Summary

Six features for v0.3.0: multi-peer batch sync, delta-based compression, semantic conflict resolution, pipeline scheduling, background indexing, and metrics/observability.

## Phases

| # | Phase | Effort | Status | Depends On |
|---|-------|--------|--------|------------|
| 1 | [Core Types & Config](phase-01-core-types-config-extensions.md) | 1h | complete | -- |
| 2 | [Background Indexing Worker](phase-02-background-indexing-worker.md) | 2h | complete | 1 |
| 3 | [Delta Sync Compression](phase-03-delta-sync-compression.md) | 2.5h | complete | 1 |
| 4 | [Batch Multi-Peer Sync](phase-04-batch-multi-peer-sync.md) | 2h | complete | 3 |
| 5 | [Semantic Conflict Resolution](phase-05-semantic-conflict-resolution.md) | 2.5h | complete | 1 |
| 6 | [Pipeline Scheduling](phase-06-pipeline-scheduling.md) | 2h | complete | 1, 2 |
| 7 | [Metrics & Observability](phase-07-metrics-observability.md) | 2h | complete | 1 |
| 8 | [Integration & Testing](phase-08-integration-testing.md) | 1h | complete | 2-7 |

## Dependency Graph

```
Phase 1 (Core) ─┬─> Phase 2 (Indexer)  ─┬─> Phase 6 (Scheduling)
                 ├─> Phase 3 (Delta)     ─── Phase 4 (Batch Sync)
                 ├─> Phase 5 (Merge)     │
                 ├─> Phase 7 (Metrics)   │
                 └───────────────────────┴─> Phase 8 (Integration)
```

## New Workspace Dependencies

```toml
zstd = "0.13"
tokio-cron-scheduler = "0.13"
notify-debouncer-full = "0.4"
diffy = "0.4"
metrics = "0.24"
metrics-exporter-prometheus = { version = "0.16", optional = true }
tokio-metrics = "0.3"
```

## Risk Summary

- `notify-debouncer-full` 0.4 shared between indexer (Phase 2) and scheduler (Phase 6) -- two separate watchers (validated)
- Feature-gate heavy dep (`metrics-exporter-prometheus`) to avoid binary bloat
- Background worker shutdown must be graceful -- use `CancellationToken` pattern
- Pipeline schedules derived from TOML trigger section (persistent by design)

## Research Reports

- [Sync & Scheduling](research/researcher-sync-scheduling.md)
- [Metrics, Workers & Merge](research/researcher-metrics-workers.md)

## Validation Log

### Session 1 — 2026-02-14
**Trigger:** Initial plan validation before implementation
**Questions asked:** 6

#### Questions & Answers

1. **[Risk]** fast-rsync v0.1.1 was last updated ~2021 (Dropbox). It's a small API but unmaintained. Should we use it or build a simpler custom delta?
   - Options: Use fast-rsync (Recommended) | Skip delta, use zstd only | Custom chunked delta
   - **Answer:** Skip delta, use zstd only
   - **Rationale:** Eliminates abandoned dep risk. zstd compression alone gives 3-5x bandwidth savings for text. Simpler implementation, fewer failure modes. Delta can be added in v0.4 if needed.

2. **[Architecture]** Background indexer and pipeline scheduler BOTH create notify-debouncer-full watchers on the vault. Two FS watchers on same directory — acceptable or share one?
   - Options: Two separate watchers (Recommended) | Shared watcher + event bus | Shared watcher in core crate
   - **Answer:** Two separate watchers
   - **Rationale:** Simpler code, modules stay independent. OS handles multiple watchers fine. Avoids coupling search and agent crates.

3. **[Scope]** Pipeline schedules are stored in-memory only (tokio-cron-scheduler). Lost on CLI restart. Acceptable for v0.3?
   - Options: In-memory only (Recommended) | Persist to TOML file | Use pipeline TOML triggers
   - **Answer:** Use pipeline TOML triggers
   - **Rationale:** Schedules defined in pipeline.toml `[trigger]` section are already persistent by design. No need for separate schedule state. Users edit TOML once, scheduler reads on startup.

4. **[Scope]** The ratatui TUI dashboard (metrics watch) adds complexity for a CLI tool. Is it worth including in v0.3?
   - Options: Include behind feature flag | Defer to v0.4 (Recommended) | Replace with simple CLI table
   - **Answer:** Defer to v0.4
   - **Rationale:** Focus v0.3 on core performance features. Metrics facade + prometheus exporter + simple `metrics show` CLI table is sufficient. Removes ratatui + crossterm deps entirely from v0.3 scope.

5. **[Security]** Semantic merge Tier 2 sends note content to LLM for conflict resolution. This means private note content goes to external API. Is this acceptable behavior for merge-assistant agent?
   - Options: Yes, same as existing agents (Recommended) | Only with Ollama (local) | Require explicit opt-in
   - **Answer:** Yes, same as existing agents
   - **Rationale:** Consistent with existing agent behavior. Users already choose their LLM provider (including Ollama for local). No additional security boundary needed.

6. **[Architecture]** Batch sync merges peer results sequentially (peer A then peer B then peer C). Different merge order could yield different results. How to handle determinism?
   - Options: Sort by peer_id (Recommended) | Sort by last_seen timestamp | Don't enforce order
   - **Answer:** Sort by peer_id
   - **Rationale:** Deterministic alphabetical ordering ensures same result regardless of connection timing. Reproducible merges across all devices.

#### Confirmed Decisions
- Delta sync: zstd compression only, no fast-rsync — simpler, fewer deps
- FS watchers: two separate instances, no shared event bus — independence over DRY
- Scheduling: pipeline TOML triggers are persistent — no in-memory-only state
- TUI dashboard: deferred to v0.4 — removes ratatui/crossterm deps
- LLM merge: same security model as existing agents — no extra opt-in
- Batch merge order: sort by peer_id — deterministic

#### Action Items
- [ ] Remove `fast-rsync` from workspace deps — Phase 3 uses zstd-only compression
- [ ] Remove `ratatui`, `crossterm` from workspace deps — deferred to v0.4
- [ ] Update Phase 3 to use zstd compression without rsync delta
- [ ] Update Phase 6 to read triggers from pipeline TOML on startup instead of in-memory schedule management
- [ ] Update Phase 7 to remove TUI dashboard, add simple `metrics show` CLI table instead
- [ ] Update Phase 4 to sort peers by peer_id for deterministic merge ordering

#### Impact on Phases
- Phase 3: Remove fast-rsync. Simplify to zstd blob compression only. Reduces effort ~30min.
- Phase 4: Add peer_id sort for merge ordering in `sync_all_peers()`.
- Phase 6: Read trigger config from TOML on startup. Remove `schedule/unschedule/list-schedules` CLI commands. Simplifies significantly.
- Phase 7: Remove ratatui TUI. Replace `metrics watch` with `metrics show` (simple formatted table). Remove crossterm dep.
