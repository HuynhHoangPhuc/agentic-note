# Sync & Scheduling Research — agentic-note v0.3.0
Date: 2026-02-14

---

## 1. Delta Sync Algorithms

### Crates

| Crate | Version | Approach | Notes |
|-------|---------|----------|-------|
| `fast-rsync` | 0.1.1 | rsync-like block delta (pure Rust, SIMD) | Dropbox-maintained; fastest signature calc; uses MD4 (insecure, pair with SHA-256) |
| `librsync` | 0.3.x | FFI bindings to C librsync | Streaming ops; heavier dep; no pure Rust |
| `fastcdc` | 3.x | Content-defined chunking | Deterministic chunks; ideal for dedup |
| `gearhash` | 0.1.x | Gear hash for CDC boundary detection | Low-level; used internally by fastcdc |
| `zstd` | 0.13.x | Zstandard compression bindings | Best ratio for text; ~400 MB/s compress |
| `lz4` | 1.24.x | LZ4 bindings | Fastest speed; lower ratio than zstd |

### Recommendation
For markdown/text files in agentic-note:
- **Delta**: `fast-rsync` — pure Rust, SIMD-optimized, proven in production (Dropbox). Pair with SHA-256 to cover MD4 weakness.
- **Chunking**: `fastcdc` — only needed if implementing blob-level dedup in CAS layer (already uses content hashing, so skip for v0.3).
- **Compression**: `zstd` — better ratio than lz4 for text; worthwhile since notes are text-heavy. Use level 3 (fast + good ratio).

Skip `librsync` (FFI overhead, C dep) and `gearhash` (too low-level; YAGNI).

---

## 2. Cron Scheduling

### Crates

| Crate | Downloads/mo | Tokio-native | Features |
|-------|-------------|--------------|---------|
| `tokio-cron-scheduler` | ~156k | Yes | Full cron syntax, timezone, per-job notifications, optional Postgres/NATS persistence |
| `tokio-cron` | ~4k | Yes | Minimal cron for tokio; UTC + local TZ |
| `cron` | ~100k+ | No | Cron expression parsing only; no executor |
| `notify` (file watcher) | ~1M+ | Partial | Cross-platform FS events; needs channel bridge to tokio |

### File Watcher Integration
`notify` 6.x exposes an event channel. Bridge to tokio via `tokio::sync::mpsc`:
```rust
let (tx, mut rx) = tokio::sync::mpsc::channel(32);
let mut watcher = notify::recommended_watcher(move |res| {
    tx.blocking_send(res).ok();
})?;
```

### Background Task Pattern (tokio)
```rust
tokio::spawn(async move {
    let mut sched = JobScheduler::new().await?;
    sched.add(Job::new_async("0 */5 * * * *", |_, _| Box::pin(async {
        sync_task().await;
    }))?).await?;
    sched.start().await
});
```

### Recommendation
- **Scheduled sync**: `tokio-cron-scheduler` 0.13.x — best fit; tokio-native, well-maintained (68 dependent crates), cron + duration modes.
- **File-triggered sync**: `notify` 6.x — debounce events, convert to tokio channel, trigger delta sync on `.md` changes.
- Combine both: cron as fallback heartbeat (every 5 min), notify for immediate trigger.

---

## 3. Multi-Peer Sync Coordination

### iroh Ecosystem

| Crate | Role |
|-------|------|
| `iroh` | Core networking (QUIC/hole-punch) |
| `iroh-gossip` | Epidemic broadcast (HyParView + PlumTree) |
| `iroh-docs` | CRDT-based document sync (range-based set reconciliation) |

### iroh-gossip Architecture
- **Membership**: HyParView (partial view, fault-tolerant)
- **Broadcast**: PlumTree — eager push to "eager set", lazy hash-announce to "lazy set"
- Peers self-organize; no central coordinator

### Conflict Resolution Strategies

| Strategy | Crate/Approach | Use Case |
|----------|---------------|---------|
| Range-based set reconciliation | `iroh-docs` built-in | Document set sync across peers |
| Vector clocks | Manual / `vclock` crate | Fine-grained event ordering |
| Lamport timestamps | Simple counter in `core::types` | Causal ordering without per-peer clocks |
| Last-Write-Wins (LWW) | `ULID` timestamps (already in core) | Simple; good enough for note metadata |

### Multi-Peer Fan-Out Pattern
```
Local change → iroh-gossip broadcast → all peers receive
Each peer: apply delta via fast-rsync → persist to vault
Conflict: compare Lamport ts → LWW or 3-way merge
```

### Recommendation
- Use `iroh-gossip` for peer discovery and broadcast (already using iroh in sync crate).
- Use `iroh-docs` range-based set reconciliation for note set sync — avoids reinventing CRDT.
- For conflict resolution: **LWW with ULID timestamps** (already in `core`) is sufficient for note-taking (v0.3); defer vector clocks to v0.4+.
- No need for standalone `vclock` crate — YAGNI at this stage.

---

## Summary: Recommended Crates for v0.3.0

```toml
[dependencies]
fast-rsync = "0.1"        # delta sync
zstd = "0.13"             # compression
tokio-cron-scheduler = "0.13"  # cron scheduling
notify = "6"              # file watcher triggers
iroh-gossip = "0.96"      # multi-peer broadcast
iroh-docs = "0.96"        # CRDT set reconciliation
```

---

## Unresolved Questions

1. `fast-rsync` is at 0.1.1 — last updated ~2021. Confirm still maintained or consider reimplementing rolling hash in-house if abandoned.
2. `iroh-docs` version alignment with current `iroh` version in codebase — check `Cargo.lock` for existing iroh version before adding iroh-docs.
3. Debounce window for `notify`-triggered sync: too short = thrash, too long = stale. Need empirical tuning (suggested: 500ms–2s for local, 5s for remote trigger).
4. `tokio-cron-scheduler` persistence (Postgres/NATS) is overkill for local-first — confirm in-memory mode is default and stable.
