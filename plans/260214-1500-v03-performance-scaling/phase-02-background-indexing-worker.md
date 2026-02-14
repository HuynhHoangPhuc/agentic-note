---
phase: 2
title: "Background Indexing Worker"
status: complete
effort: 2h
depends_on: [1]
---

## Context Links

- [Search lib.rs](../../crates/search/src/lib.rs)
- [Search fts.rs](../../crates/search/src/fts.rs)
- [Search reindex.rs](../../crates/search/src/reindex.rs)
- [CLI main.rs](../../crates/cli/src/main.rs)
- [Research: Background Workers](research/researcher-metrics-workers.md)

## Overview

Replace blocking index-on-write with async background indexer. Uses `notify-debouncer-full` for FS events, `tokio::sync::mpsc` for task queue, 200ms debounce, 50-file batch limit, and `CancellationToken` for graceful shutdown.

## Key Insights

- `notify-debouncer-full` 0.4 delivers debounced events directly into tokio mpsc -- no manual bridging
- `tokio-util` `CancellationToken` is canonical shutdown pattern, already transitive dep
- Existing `SearchEngine::index_note()` and `reindex::reindex_vault()` can be reused
- Background worker runs as `tokio::spawn` task alongside CLI main loop
- Debounce 200ms + batch max 50 prevents thrashing on rapid saves

## Requirements

**Functional:**
- FS watcher monitors vault directory for `.md` file changes (create/modify/delete)
- Changes debounced at 200ms window
- Batch up to 50 files per index cycle
- Graceful shutdown on CLI exit or SIGINT
- Manual `reindex` command still works independently

**Non-functional:**
- < 5ms overhead on main thread (just channel send)
- Indexer errors logged but never crash the CLI
- Config-driven: `[indexer] background = true` to enable

## Architecture

```
CLI main
  │ spawn BackgroundIndexer
  ▼
BackgroundIndexer (tokio::spawn)
  ├── notify-debouncer-full watches vault/**/*.md
  ├── tokio::select! { debounced_events, manual_tx, cancel_token }
  ├── collect paths into HashSet (dedup)
  ├── on 200ms timeout OR batch >= 50: flush
  │     └── SearchEngine::index_note() for each changed note
  │     └── SearchEngine::remove_note() for deleted notes
  └── on cancel: flush remaining, drop watcher
```

## Related Code Files

**Create:**
- `crates/search/src/background_indexer.rs` -- BackgroundIndexer struct + spawn logic

**Modify:**
- `crates/search/src/lib.rs` -- add `pub mod background_indexer;`, re-export
- `crates/search/Cargo.toml` -- add `notify-debouncer-full`, `tokio-util`
- `crates/cli/src/main.rs` -- spawn indexer on startup, cancel on shutdown
- Root `Cargo.toml` -- add workspace deps

## Implementation Steps

1. Add workspace deps to root `Cargo.toml`:
   ```toml
   notify-debouncer-full = "0.4"
   tokio-util = { version = "0.7", features = ["rt"] }
   ```

2. Add to `crates/search/Cargo.toml`:
   ```toml
   notify-debouncer-full = { workspace = true }
   tokio-util = { workspace = true }
   tokio = { workspace = true }
   agentic-note-vault = { workspace = true }
   ```

3. Create `crates/search/src/background_indexer.rs`:
   - Define `BackgroundIndexer` struct holding `CancellationToken`
   - `pub fn new(vault_path: PathBuf, config: IndexerConfig) -> Self`
   - `pub async fn spawn(self, search_engine: Arc<Mutex<SearchEngine>>) -> JoinHandle<()>`
   - Internal loop:
     a. Create `notify-debouncer-full` watcher on `vault_path`
     b. Receive debounced events into `mpsc::Receiver`
     c. Collect changed paths into `HashSet<PathBuf>`
     d. `tokio::select!` on: event_rx, sleep(200ms), cancel_token.cancelled()
     e. On flush: read each `.md` file via `Note::read()`, call `search_engine.index_note()`
     f. On delete events: call `search_engine.remove_note()`
   - `pub fn cancel(&self)` -- triggers the CancellationToken

4. Add `pub mod background_indexer;` to `crates/search/src/lib.rs`.

5. Modify `crates/cli/src/main.rs`:
   - After resolving vault path, load `IndexerConfig`
   - If `config.indexer.background`:
     a. Open `SearchEngine`
     b. Wrap in `Arc<Mutex<SearchEngine>>`
     c. Spawn `BackgroundIndexer`
     d. Store `CancellationToken` handle
   - On exit / error: cancel the token before process exit

6. Add `IndexTask` enum for manual index requests:
   ```rust
   pub enum IndexTask {
       FileChanged(PathBuf),
       FileDeleted(PathBuf),
       ReindexAll,
   }
   ```

7. Expose `mpsc::Sender<IndexTask>` so CLI commands can trigger immediate indexing after note create/update/delete.

8. Run `cargo check -p agentic-note-search -p agentic-note-cli`.

9. Write unit test: mock FS events, verify index_note called with correct paths.

10. Write integration test: create temp vault, spawn indexer, write .md file, assert search finds it within 500ms.

## Todo List

- [ ] Add workspace deps (notify-debouncer-full, tokio-util)
- [ ] Create `background_indexer.rs` module
- [ ] Implement `BackgroundIndexer` struct
- [ ] Implement debounce + batch flush logic
- [ ] Implement graceful shutdown with CancellationToken
- [ ] Wire into CLI main.rs
- [ ] Expose IndexTask channel for manual triggers
- [ ] Unit test: event -> index call
- [ ] Integration test: file write -> searchable within 500ms
- [ ] `cargo check` passes

## Success Criteria

- Background indexer starts on CLI launch when enabled
- New/modified `.md` files indexed within ~500ms
- Deleted files removed from index
- Graceful shutdown flushes pending batch
- No panics or crashes on watcher errors (logged + continue)

## Risk Assessment

- **FS watcher platform differences**: `notify` handles macOS/Linux/Windows; test on target OS
- **Concurrent SearchEngine access**: `Arc<Mutex<>>` may contend with CLI commands -- acceptable for local CLI (low concurrency)
- **Tantivy writer lock**: only one writer at a time; background indexer holds it during flush

## Security Considerations

- Watcher only monitors vault directory (not parent/system dirs)
- File path validated to be within vault root before indexing

## Next Steps

- Phase 6 (Pipeline Scheduling) reuses the `notify-debouncer-full` watcher pattern
- Phase 7 (Metrics) instruments indexing latency
