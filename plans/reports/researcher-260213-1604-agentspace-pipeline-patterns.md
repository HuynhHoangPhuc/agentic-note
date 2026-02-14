# AgentSpace Pipeline Patterns — Research Report
Date: 2026-02-13 | Slug: agentspace-pipeline-patterns

---

## 1. DAG Workflow Engines — Minimal Abstraction

### What Airflow/Prefect/Temporal actually do

| Concept | Airflow | Prefect | Temporal | Minimal equivalent |
|---|---|---|---|---|
| Unit of work | Task | Task | Activity | `Stage` (fn + config) |
| Orchestration unit | DAG | Flow | Workflow | `Pipeline` (ordered stages) |
| Execution model | Scheduled, dependency-ordered | Event or scheduled | Event-driven durable | Event or scheduled |
| State persistence | Postgres/MySQL | API server / SQLite | Temporal server | SQLite (local-first) |
| Config format | Python DSL | Python decorators | Go/Java/TypeScript SDK | **TOML** (simplest for users) |
| Retry/backoff | Built-in | Built-in | Built-in | Per-stage `retry` field |

**Key insight:** All three reduce to the same primitives:
```
pipeline = [stage_A, stage_B, stage_C]
stage.input  = previous_stage.output | trigger_event
stage.output = typed artifact (JSON, text, file path)
```

Airflow's mistake: Python-only DSL = developer-only config.
Prefect's improvement: decorators are cleaner but still Python.
**For AgentSpace: TOML config is the right call** — users aren't Python devs.

### Minimal DAG abstraction for AgentSpace

Three concepts only:
1. **Stage** — named processing unit with an agent function + config
2. **Pipeline** — ordered list of stages with named I/O wiring
3. **Trigger** — what starts the pipeline (file event, schedule, manual)

No branching needed for MVP (sequential pipeline = enough for 90% of PKM workflows).
Branching = v2 feature (add `when` conditions to stages).

---

## 2. Event-Driven Note Processing — Rust Implementation

### Triggers taxonomy

| Trigger type | Event | Rust mechanism |
|---|---|---|
| File created | New `.md` in inbox/ | `notify` crate (`RecommendedWatcher`) |
| File modified | Any vault `.md` saved | `notify` + debounce (ignore within 1s) |
| Scheduled | Daily/weekly digest | `tokio::time::interval` or cron-style |
| Manual | User runs `pipeline run` | CLI command → direct dispatch |
| Tag-based | Note gains specific tag | Post-write hook in vault module |

### notify crate pattern (Rust)

```rust
// notify = "6.x" with tokio feature
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event};
use tokio::sync::mpsc;

async fn watch_vault(vault_path: PathBuf, tx: mpsc::Sender<Event>) {
    let (sync_tx, mut sync_rx) = std::sync::mpsc::channel();
    let mut watcher = RecommendedWatcher::new(sync_tx, Config::default()).unwrap();
    watcher.watch(&vault_path, RecursiveMode::Recursive).unwrap();

    // Bridge sync → async
    tokio::task::spawn_blocking(move || {
        while let Ok(event) = sync_rx.recv() {
            if let Ok(e) = event { tx.blocking_send(e).ok(); }
        }
    });
}
```

Key decisions:
- **Debounce at 500ms**: editors (vim, VSCode) write multiple events per save; debounce to one
- **Filter to inbox/ only** for auto-triggers: avoid processing every vault edit
- **Event queue = `tokio::sync::mpsc` channel**: pipeline worker reads from channel, processes async
- **Backpressure**: bounded channel (capacity=32) drops events if pipeline is behind; log dropped events

### Event queue architecture

```
FileWatcher ─→ debounce ─→ EventQueue (mpsc) ─→ PipelineDispatcher
                                                      │
                                                      ├─→ Pipeline "auto-classify"
                                                      ├─→ Pipeline "auto-link"
                                                      └─→ Pipeline "summarize"
```

PipelineDispatcher matches event (file path, event type) to registered pipeline triggers.
Each pipeline runs as a separate `tokio::task`. Max concurrent pipelines = configurable (default: 3).

---

## 3. Agent Pipeline Patterns — LangGraph/CrewAI/AutoGen

### How they chain outputs

**LangGraph** (graph-based, most relevant):
- Nodes = agent functions; edges = transitions
- State = typed dict passed between nodes (each node reads + writes state)
- `state["output_A"]` → next node reads it as `state["input_B"]`
- Key pattern: **shared state object** flows through the graph, each stage mutates it

**CrewAI** (role-based sequential):
- Tasks have `context` param = list of previous tasks whose output feeds this task
- Simple: `Task(description="...", context=[task_A, task_B])` = task_B's output injected into prompt
- Key pattern: **output as string** — each stage produces text that gets injected into next stage's prompt

**AutoGen** (conversation-based):
- Agents take turns in a conversation; each message = one agent's output = next agent's input
- Key pattern: **message passing** — structured JSON or plain text between agents

### Simplest chaining pattern for AgentSpace

Don't copy LangGraph's full state graph. **Use CrewAI's simpler model:**

```
StageOutput { note_id, content, metadata, agent_notes }
```

Each stage receives previous stage's `StageOutput`. Stages can:
1. Transform content (summarize, classify, extract)
2. Mutate metadata (add tags, para category, links)
3. Write back to vault (via approval gate)

No graph needed for MVP: stages are a **linear chain**. The "DAG" is implicit in the stage ordering.

---

## 4. Feed Generation from Notes

### What a "knowledge feed" is

Four feed types, ordered by implementation complexity:

**Type 1 — Daily Digest (easiest)**
- Cron trigger (morning, e.g., 08:00)
- Collect: notes modified in last 24h + inbox items + overdue tasks
- Format: markdown summary with links, ~1 page
- Output: `inbox/daily-digest-YYYY-MM-DD.md` (auto-created note)

**Type 2 — Weekly Review (Tiago Forte style)**
- Weekly trigger (Friday evening or Monday morning)
- Collect: completed projects, PARA inbox backlog, stale notes (30d+ unedited)
- Agent generates: "What went well", "What to archive", "What to promote"
- Output: weekly review note + suggested actions queue

**Type 3 — Spaced Repetition Feed**
- SM-2 algorithm or simpler: review at +1d, +3d, +7d, +14d, +30d intervals
- Track `last_reviewed` + `interval` in note frontmatter
- Feed = list of notes due for review today
- Output: `inbox/review-queue-YYYY-MM-DD.md` with spaced items

**Type 4 — Serendipity Feed (most novel)**
- Pick 3-5 random notes from archives + resurface by semantic similarity to recent notes
- "On this day" (same date, prior years)
- "You wrote X 3 months ago; here's something related you wrote last week"
- Output: pushed as daily digest section OR separate feed note

### Feed generation algorithm

```
feed_generator:
  1. gather(sources: [modified_today, inbox, due_for_review, random_archive])
  2. rank(by: recency + relevance_to_current_projects + spaced_repetition_score)
  3. deduplicate(by: note_id)
  4. format(template: "daily-digest" | "weekly-review" | "review-queue")
  5. write(to: inbox/{type}-{date}.md, approval: auto)  # feeds don't need HITL
```

Personalization signal (no ML needed for MVP):
- Weight notes linked to active Projects higher
- Weight notes tagged with user's `focus_areas` config field
- Weight notes not visited in 7+ days (forgotten notes)

---

## 5. User-Definable Pipeline Config

### TOML vs YAML for users

| Aspect | TOML | YAML |
|---|---|---|
| Readability | Cleaner for flat config | Cleaner for nested structures |
| Error messages | Stricter parser, better errors | Notoriously footgun-y (indentation) |
| Rust ecosystem | `toml` crate (serde) — first-class | `serde_yaml` — fine but YAML ambiguities |
| User familiarity | Cargo.toml teaches this | Already common in CI/CD |
| Multiline strings | `"""` triple quotes | `|` block scalar |

**Verdict: TOML for MVP.** Users already know Cargo.toml format. Fewer footguns than YAML.

### Proposed config format

```toml
# .agentic/pipelines/auto-process.toml

[pipeline]
name = "auto-process-inbox"
description = "Capture → Classify → Link → Summarize"
enabled = true

[trigger]
type = "file_created"        # file_created | file_modified | schedule | manual
path_filter = "inbox/**"     # only inbox files
debounce_ms = 500

# Alternative schedule trigger:
# type = "schedule"
# cron = "0 8 * * *"         # 08:00 daily

[[stages]]
name = "classify"
agent = "para-classifier"    # built-in agent ID
config.model = "default"     # inherit from vault config
config.confidence_threshold = 0.8
output = "classification"

[[stages]]
name = "auto-link"
agent = "zettelkasten-linker"
config.max_links = 5
config.min_similarity = 0.75
input = "classification"     # use previous stage output as context
output = "links"

[[stages]]
name = "summarize"
agent = "distiller"
config.max_tokens = 200
config.style = "bullet"      # bullet | paragraph | tldr
output = "summary"

[[stages]]
name = "write-back"
agent = "vault-writer"
config.approval_required = true   # HITL gate
config.apply = ["classification", "links", "summary"]
```

### Built-in agents (stage types)

| Agent ID | What it does | Input | Output |
|---|---|---|---|
| `para-classifier` | Suggests PARA bucket + tags | Note content | `{para, tags, confidence}` |
| `zettelkasten-linker` | Finds related notes via embeddings | Note + classification | `{links: [{id, similarity, reason}]}` |
| `distiller` | Summarizes note content | Note content | `{summary, bullets, tldr}` |
| `vault-writer` | Applies changes to note frontmatter | All stage outputs | Writes to disk (w/ HITL) |
| `feed-generator` | Builds a digest note | Configurable sources | New note in inbox/ |
| `orphan-rescuer` | Finds unlinked notes | Full vault scan | Suggested links |

Custom agents (v2): shell script or WASM plugin that reads/writes JSON on stdin/stdout.

---

## 6. Concrete AgentSpace MVP Design

### Core struct (Rust)

```rust
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub trigger: Trigger,
    pub stages: Vec<Stage>,
}

pub struct Stage {
    pub name: String,
    pub agent: AgentId,          // enum of built-in agents
    pub config: toml::Value,     // pass-through config
    pub input: Option<String>,   // stage name to read from, None = trigger event
    pub output: String,          // key in StageContext
}

pub struct StageContext {
    pub note_id: String,
    pub note_content: String,
    pub frontmatter: FrontMatter,
    pub outputs: HashMap<String, serde_json::Value>,  // stage_name → output
}
```

### Execution model

```
Trigger fires → load pipeline config → create StageContext(note) →
  for each stage in pipeline:
    agent = registry.get(stage.agent)
    input = context.outputs[stage.input] OR context.note_content
    output = agent.run(input, stage.config, llm_provider).await?
    context.outputs[stage.name] = output
  →
  if any stage has approval_required:
    enqueue to review_queue (SQLite)
    notify user ("1 pipeline result awaiting approval")
  else:
    vault_writer.apply(context).await?
```

### CLI commands

```bash
agentic-note pipeline list                    # show all pipelines
agentic-note pipeline run auto-process-inbox  # manual trigger
agentic-note pipeline run --note <id>         # run on specific note
agentic-note pipeline status                  # show running/queued jobs
agentic-note review list                      # show pending approvals
agentic-note review approve <job-id>          # approve all changes
agentic-note review diff <job-id>             # show proposed diff
agentic-note review reject <job-id>           # discard changes
```

### What MVP ships vs defers

**Ships:**
- TOML pipeline config parser + validator
- notify-based file watcher → event queue
- 4 built-in agents: classifier, linker, distiller, vault-writer
- Review queue (SQLite) + CLI approval flow
- 1 built-in pipeline: `auto-process-inbox`
- Schedule trigger (tokio interval, not full cron)

**Defers:**
- Visual pipeline editor
- Custom WASM/script agents
- Conditional stage execution (`when:` clauses)
- Pipeline branching / parallel stages
- Feed generator pipeline (after core pipeline works)
- Full cron expression support

---

## Unresolved Questions

1. **Agent isolation**: Should each pipeline stage call LLM independently, or share one LLM call? (shared = cheaper, isolated = cleaner, better for retries)
2. **Pipeline versioning**: If user edits pipeline TOML mid-run, what happens to in-flight jobs? Snapshot config at dispatch time?
3. **Error handling strategy**: Stage fails → abort whole pipeline or skip stage + continue? User config per stage (`on_error: skip | abort`)?
4. **Review queue UI**: CLI diff is sufficient for MVP? Or does TUI/desktop editor take priority for usability?
5. **Feed trigger cadence**: Daily digest at fixed time conflicts with mobile background restrictions — push to "manual + on-open" trigger for mobile?
6. **Embedding freshness**: When to re-embed notes for linker? On every write (expensive) or batch nightly (stale)?
