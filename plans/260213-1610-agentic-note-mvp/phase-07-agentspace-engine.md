# Phase 07: AgentSpace Engine

## Context
- Parent: [plan.md](plan.md)
- Deps: Phase 02 (vault)
- Research: [AgentSpace Patterns](../reports/researcher-260213-1604-agentspace-pipeline-patterns.md)

## Overview
- **Priority:** P1 (enables autonomous agent workflows)
- **Status:** pending
- **Effort:** 8h
- **Description:** TOML pipeline parser, file watcher (notify + debounce), event queue (tokio::mpsc), stage executor with StageContext passing, pipeline lifecycle.

## Key Insights
- 3 primitives: Stage, Pipeline, Trigger — keep it simple, no DAG branching for MVP
- Sequential pipeline execution (CrewAI model) — each stage receives previous output
- notify + debounce (500ms) for file watching; bridge sync→async via spawn_blocking
- Bounded mpsc channel (capacity 32) for backpressure
- Max 3 concurrent pipeline tasks (configurable)
- Agent functions are trait objects — engine doesn't know about LLM (that's Phase 08)

## Requirements

**Functional:**
- Parse pipeline config from TOML files in `pipelines/` dir
- File watcher triggers pipelines on matching events
- Manual trigger: `agentic-note pipeline run <name> [--note <id>]`
- Stage executor runs stages sequentially, passing StageContext
- Pipeline lifecycle: start/stop/status for agentspace daemon
- `agentic-note agentspace start` — activate file watcher + pipeline dispatch
- `agentic-note agentspace stop` — deactivate
- `agentic-note agentspace status` — show active pipelines, queue depth
- `agentic-note pipeline list` — show all configured pipelines
- `agentic-note pipeline run <name>` — manual trigger

**Non-functional:**
- Pipeline dispatch < 100ms from event to first stage start
- Event queue handles burst of 100 file events without dropping
- Clean shutdown: finish in-flight stages, discard queued events

## Architecture

```
crates/agent/src/
├── lib.rs              # pub mod re-exports
├── engine/
│   ├── mod.rs          # AgentSpace engine struct
│   ├── pipeline.rs     # Pipeline + Stage structs, TOML parser
│   ├── trigger.rs      # Trigger types (file, manual), matching logic
│   ├── watcher.rs      # File watcher (notify) → event queue
│   ├── executor.rs     # Stage executor, StageContext management
│   ├── context.rs      # StageContext struct (shared state between stages)
│   └── dispatcher.rs   # Event → pipeline matching, task spawning
```

Note: AgentSpace engine lives in the `agent` crate alongside LLM/agent code (Phase 08) to avoid circular deps. Engine module is self-contained, agents plug in via trait.

## Related Code Files

**Create:**
- `crates/agent/Cargo.toml` (update stub)
- `crates/agent/src/lib.rs`
- `crates/agent/src/engine/mod.rs`
- `crates/agent/src/engine/pipeline.rs`
- `crates/agent/src/engine/trigger.rs`
- `crates/agent/src/engine/watcher.rs`
- `crates/agent/src/engine/executor.rs`
- `crates/agent/src/engine/context.rs`
- `crates/agent/src/engine/dispatcher.rs`
- `pipelines/auto-process-inbox.toml` (default pipeline config)

**Modify:**
- `crates/cli/src/commands/mod.rs` — add AgentSpace, Pipeline subcommands
- `crates/cli/Cargo.toml` — add agent dep

## Cargo.toml Dependencies
```toml
[dependencies]
agentic-note-core = { path = "../core" }
agentic-note-vault = { path = "../vault" }
tokio = { workspace = true }
notify = "6.1"
notify-debouncer-mini = "0.4"
toml = "0.8"
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
async-trait = "0.1"
```

## Implementation Steps

1. **`context.rs`:** StageContext
   ```rust
   pub struct StageContext {
       pub note_id: NoteId,
       pub note_content: String,
       pub frontmatter: FrontMatter,
       pub outputs: HashMap<String, serde_json::Value>, // stage_name → output
       pub vault_path: PathBuf,
   }
   ```
   - `StageContext::from_note(note: &Note, vault_path: &Path) -> Self`
   - `StageContext::get_output<T: DeserializeOwned>(&self, stage: &str) -> Result<T>`
   - `StageContext::set_output(&mut self, stage: &str, value: serde_json::Value)`

2. **`pipeline.rs`:** TOML parsing
   ```rust
   pub struct PipelineConfig { pub name: String, pub description: String, pub enabled: bool, pub trigger: TriggerConfig, pub stages: Vec<StageConfig> }
   pub struct StageConfig { pub name: String, pub agent: String, pub config: toml::Value, pub input: Option<String>, pub output: String }
   ```
   - `PipelineConfig::load(path: &Path) -> Result<Self>` — deserialize TOML
   - `PipelineConfig::load_all(dir: &Path) -> Result<Vec<Self>>` — load all .toml files
   - Validate: no duplicate stage names, input refs exist, agent IDs valid

3. **`trigger.rs`:**
   ```rust
   pub struct TriggerConfig { pub trigger_type: TriggerType, pub path_filter: Option<String>, pub debounce_ms: Option<u64> }
   pub enum TriggerType { FileCreated, FileModified, Manual }
   ```
   - `TriggerConfig::matches(event: &FileEvent) -> bool` — glob match on path_filter
   - `FileEvent { path: PathBuf, event_type: FileEventType }`

4. **`watcher.rs`:** File watcher → async event queue
   - `VaultWatcher::start(vault_path: &Path, tx: mpsc::Sender<FileEvent>) -> Result<Self>`
   - Uses `notify-debouncer-mini` with configurable debounce (default 500ms)
   - Bridge sync notify callback → tokio mpsc via `spawn_blocking`
   - Filter: only `.md` files, ignore `.agentic/` dir
   - `VaultWatcher::stop(self)` — drop watcher handle

5. **`executor.rs`:** Stage executor
   ```rust
   #[async_trait]
   pub trait AgentHandler: Send + Sync {
       fn agent_id(&self) -> &str;
       async fn execute(&self, ctx: &mut StageContext, config: &toml::Value) -> Result<serde_json::Value>;
   }
   ```
   - `StageExecutor::new(agents: HashMap<String, Arc<dyn AgentHandler>>)`
   - `StageExecutor::run_pipeline(pipeline: &PipelineConfig, ctx: &mut StageContext) -> Result<PipelineResult>`
     - Iterate stages sequentially
     - Look up agent handler by ID
     - Call `execute()`, store output in context
     <!-- Updated: Validation Session 1 - Global skip+warn error policy -->
     - On error: log warning, skip failed stage, continue pipeline (global `on_error: skip` policy)
     - Per-stage error policy deferred to v2
   - `PipelineResult { stages_completed: usize, total: usize, outputs: HashMap<String, Value>, skipped: Vec<String>, warnings: Vec<String> }`

6. **`dispatcher.rs`:** Event → pipeline matching
   - `Dispatcher::new(pipelines: Vec<PipelineConfig>, executor: StageExecutor)`
   - `Dispatcher::dispatch(event: FileEvent) -> Vec<PipelineTask>` — match event to pipeline triggers
   - Spawns pipeline execution as tokio task
   - Semaphore for max concurrent pipelines (default 3)
   - Track running tasks for status reporting

7. **`mod.rs`:** AgentSpace engine facade
   - `AgentSpace::new(vault_path, pipelines_dir) -> Result<Self>`
   - `AgentSpace::start() -> Result<()>` — start watcher, create dispatcher
   - `AgentSpace::stop() -> Result<()>` — stop watcher, wait for in-flight tasks
   - `AgentSpace::status() -> AgentSpaceStatus` — running pipelines, queue depth
   - `AgentSpace::run_manual(pipeline_name, note_id) -> Result<PipelineResult>`
   - `AgentSpace::register_agent(handler: Arc<dyn AgentHandler>)`

8. **Default pipeline config:** `pipelines/auto-process-inbox.toml`
   - Trigger: file_created in inbox/
   - Stages: classify → auto-link → distill → write (approval_required)

9. **CLI commands:** agentspace start/stop/status, pipeline list/run

## Todo List
- [ ] Define StageContext struct
- [ ] Implement TOML pipeline parser + validation
- [ ] Implement trigger matching logic
- [ ] Implement file watcher with debounce
- [ ] Implement stage executor with AgentHandler trait
- [ ] Implement dispatcher (event → pipeline matching)
- [ ] Implement AgentSpace lifecycle (start/stop/status)
- [ ] Create default auto-process-inbox.toml
- [ ] Add CLI commands
- [ ] Write tests for pipeline parsing + trigger matching

## Success Criteria
- TOML pipeline config parses correctly
- File watcher detects new .md in inbox/
- Dispatcher matches event to correct pipeline
- Executor runs stages sequentially, passing context
- `agentspace status` shows running pipelines
- Manual pipeline run works: `pipeline run auto-process-inbox --note <id>`

## Risk Assessment
- **macOS FSEvents latency:** ~1s kernel delay — acceptable for async processing
- **Event storm:** rapid file saves could flood queue — bounded channel + debounce mitigate
- **Agent handler panic:** wrap execute() in catch_unwind or tokio task — don't crash engine

## Security Considerations
- Pipeline configs are local TOML files — no remote execution
- AgentHandler trait is internal — no user-supplied code execution in MVP
- File watcher only monitors vault dir — no path traversal risk

## Next Steps
- Phase 08 implements actual AgentHandler implementations (LLM-powered agents)
- Phase 08 adds review queue integration to executor
