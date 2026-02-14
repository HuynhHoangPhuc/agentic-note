---
phase: 6
title: "Pipeline Scheduling"
status: complete
effort: 2h
depends_on: [1, 2]
---

## Context Links

- [Agent engine/pipeline.rs](../../crates/agent/src/engine/pipeline.rs)
- [Agent engine/dag_executor.rs](../../crates/agent/src/engine/dag_executor.rs)
- [Agent engine/trigger.rs](../../crates/agent/src/engine/trigger.rs)
- [CLI main.rs](../../crates/cli/src/main.rs)
- [Research: Cron Scheduling](research/researcher-sync-scheduling.md)

## Overview

<!-- Updated: Validation Session 1 - Use pipeline TOML triggers (persistent), not in-memory schedules -->
Add cron-based and file-watch-based pipeline triggers. Schedules defined in pipeline TOML `[trigger]` section (persistent by design). Uses `tokio-cron-scheduler` for cron expressions and `notify-debouncer-full` (from Phase 2) for file-change triggers. On startup, scheduler reads all enabled pipeline TOMLs and activates their triggers.

## Key Insights

- `tokio-cron-scheduler` 0.13 is tokio-native; in-memory mode (no Postgres/NATS needed)
- Reuse `notify-debouncer-full` from Phase 2 -- shared dep, similar pattern (DRY)
- Existing `TriggerConfig` in `engine/trigger.rs` already has `TriggerType` enum -- extend it
- Pipeline TOML already supports `[trigger]` section -- add `cron` and `watch_path` fields
- Scheduler runs as background tokio task alongside CLI

## Requirements

**Functional:**
- Triggers defined in pipeline TOML `[trigger]` section (persistent):
  - `trigger.type = "cron"`, `trigger.cron = "*/5 * * * *"`
  - `trigger.type = "watch"`, `trigger.watch_path = "inbox/"`
- On `pipeline start-daemon`: scan all enabled pipeline TOMLs, activate triggers
- `pipeline status`: show active triggers and last execution time
- No `schedule/unschedule` commands needed -- edit TOML to change triggers

**Non-functional:**
- Scheduler overhead < 1MB memory
- File watch debounce configurable (default 500ms from config)
- Graceful shutdown stops all scheduled jobs

## Architecture

```
CLI: pipeline schedule <file> --cron "*/5 * * * *"
  └── PipelineScheduler::add_cron(pipeline_config, cron_expr)
        ├── tokio-cron-scheduler::Job::new_async(cron, callback)
        └── callback: DagExecutor::run_pipeline()

CLI: pipeline schedule <file> --watch inbox/
  └── PipelineScheduler::add_watch(pipeline_config, watch_path)
        ├── notify-debouncer-full watcher on watch_path
        ├── on .md change: send to mpsc channel
        └── receiver: DagExecutor::run_pipeline(changed_note)

PipelineScheduler (tokio::spawn)
  ├── JobScheduler (cron jobs)
  ├── FileWatchers (notify watchers)
  └── CancellationToken for shutdown
```

## Related Code Files

**Create:**
- `crates/agent/src/engine/scheduler.rs` -- `PipelineScheduler` struct
- `crates/cli/src/commands/pipeline.rs` -- `pipeline schedule/list-schedules/unschedule` commands

**Modify:**
- `crates/agent/src/engine/trigger.rs` -- extend `TriggerType` with `Cron`, `Watch`
- `crates/agent/src/engine/pipeline.rs` -- extend `TriggerConfig` with `cron` and `watch_path` fields
- `crates/agent/src/engine/mod.rs` -- add `pub mod scheduler;`
- `crates/agent/Cargo.toml` -- add `tokio-cron-scheduler`, `notify-debouncer-full`
- `crates/cli/src/commands/mod.rs` -- add `Pipeline` command variant
- `crates/cli/src/main.rs` -- dispatch pipeline commands
- Root `Cargo.toml` -- add `tokio-cron-scheduler` workspace dep

## Implementation Steps

1. Add workspace dep to root `Cargo.toml`:
   ```toml
   tokio-cron-scheduler = "0.13"
   ```

2. Add to `crates/agent/Cargo.toml`:
   ```toml
   tokio-cron-scheduler = { workspace = true }
   notify-debouncer-full = { workspace = true }
   tokio-util = { workspace = true }
   ```

3. Extend `TriggerType` in `crates/agent/src/engine/trigger.rs`:
   ```rust
   pub enum TriggerType {
       Manual,
       OnCreate,
       Cron,     // NEW
       Watch,    // NEW
   }
   ```

4. Extend `TriggerConfig` in `pipeline.rs`:
   ```rust
   pub struct TriggerConfig {
       pub trigger_type: TriggerType,
       pub path_filter: Option<String>,
       pub debounce_ms: u64,
       pub cron: Option<String>,       // NEW: cron expression
       pub watch_path: Option<String>, // NEW: directory to watch
   }
   ```

5. Create `crates/agent/src/engine/scheduler.rs`:
   - `PipelineScheduler` struct:
     ```rust
     pub struct PipelineScheduler {
         job_scheduler: JobScheduler,
         watchers: Vec<(String, CancellationToken)>,
         cancel: CancellationToken,
     }
     ```
   - `pub async fn new() -> Result<Self>`
   - `pub async fn add_cron(&mut self, name: &str, config: PipelineConfig, cron: &str, executor: Arc<DagExecutor>) -> Result<()>`
     - Create `Job::new_async(cron, callback)` that invokes `executor.run_pipeline()`
   - `pub async fn add_watch(&mut self, name: &str, config: PipelineConfig, watch_path: &Path, executor: Arc<DagExecutor>) -> Result<()>`
     - Create `notify-debouncer-full` watcher on `watch_path`
     - On `.md` events: load note, build `StageContext`, run pipeline
   - `pub fn list_schedules(&self) -> Vec<ScheduleInfo>`
   - `pub async fn remove(&mut self, name: &str) -> Result<()>`
   - `pub async fn shutdown(&mut self)` -- cancel all jobs and watchers

6. Create `crates/cli/src/commands/pipeline.rs`:
   ```rust
   #[derive(Subcommand)]
   pub enum PipelineCmd {
       StartDaemon,       // Scan pipelines/, activate all triggers
       Status,            // Show active triggers and last run
   }
   ```

7. Add `Pipeline` to `Commands` enum in `commands/mod.rs`.

8. In `start-daemon`: scan `pipelines/*.toml`, for each enabled pipeline with cron/watch trigger, add to scheduler. Keep running until SIGINT.

9. Run `cargo check -p agentic-note-agent -p agentic-note-cli`.

10. Unit test: mock executor, verify cron callback fires at scheduled time.

11. Integration test: watch a temp dir, create .md file, verify pipeline executes.

## Todo List

- [ ] Add `tokio-cron-scheduler` workspace dep
- [ ] Extend `TriggerType` and `TriggerConfig`
- [ ] Create `scheduler.rs` module
- [ ] Implement cron scheduling
- [ ] Implement file-watch scheduling (reuse notify-debouncer-full)
- [ ] Implement list/remove schedule management
- [ ] Create CLI pipeline commands
- [ ] Wire into main.rs
- [ ] Unit test: cron trigger
- [ ] Integration test: file-watch trigger
- [ ] `cargo check` passes

## Success Criteria

- Cron-scheduled pipeline executes on time
- File-watch pipeline triggers within debounce window of file change
- Multiple schedules can coexist
- Schedules can be listed and removed
- Graceful shutdown cancels all scheduled jobs

## Risk Assessment

- **TOML-based triggers**: schedules are persistent by design (in pipeline.toml). No state loss on restart
- **Multiple watchers**: Phase 2 (indexer) and Phase 6 (scheduler) both watch vault. Ensure no FS event contention -- each has own watcher instance
- **Pipeline execution during another run**: use `Semaphore` or config `max_concurrent_pipelines` to prevent overlap

## Security Considerations

- Cron expressions validated before scheduling (reject invalid syntax)
- Watch paths validated to be within vault directory
- Pipeline execution respects existing trust levels (review gate)

## Next Steps

Phase 8 (Integration) tests scheduling with background indexer.
