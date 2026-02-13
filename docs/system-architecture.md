# Agentic-Note: System Architecture

## Overview

Agentic-note is a modular, local-first Rust application designed as a Cargo workspace with 7 specialized crates. Each crate has a single responsibility and communicates through well-defined interfaces. This document describes the architecture, crate relationships, and data flow.

---

## Crate Architecture

### Dependency Graph

```
                        ┌─────────────────────────────────────┐
                        │         CLI (Binary)                 │
                        │    • Commands                        │
                        │    • MCP Server                      │
                        │    • JSON Output                     │
                        └──────────────┬──────────────────────┘
                                       │
                    ┌──────────────────┼──────────────────────┐
                    │                  │                      │
                    ▼                  ▼                      ▼
            ┌──────────────┐  ┌────────────────┐  ┌──────────────────┐
            │    Vault     │  │     Search     │  │      Agent       │
            │              │  │                │  │                  │
            │ • Note CRUD  │  │ • tantivy FTS  │  │ • AgentSpace     │
            │ • Frontmatter│  │ • SQLite graph │  │ • LLM providers  │
            │ • PARA       │  │ • Reindex      │  │ • 4 built-in     │
            └──────┬───────┘  └────┬───────────┘  └──────┬───────────┘
                   │               │                     │
                   └───────────────┼─────────────────────┘
                                   │
                        ┌──────────┼──────────┐
                        │          │          │
                        ▼          ▼          ▼
                    ┌────────┐ ┌──────┐ ┌─────────┐
                    │  CAS   │ │Core  │ │ Review  │
                    │        │ │      │ │         │
                    │ • Hash │ │Types │ │ • Queue │
                    │ • Blob │ │Errors│ │ • Gate  │
                    │ • Tree │ │Config│ │         │
                    │ • Snap │ │Ulids │ │         │
                    └────────┘ └──────┘ └─────────┘
```

### Crate Descriptions

#### 1. **core** — Shared Foundation
**Purpose:** Central repository for types, errors, configuration, and ID generation used across all crates.

**Key Modules:**
- `types.rs` — Domain types: `NoteId`, `ParaCategory`, `NoteStatus`, `FrontMatter`
- `error.rs` — Unified error type: `AgenticError` with variants (NotFound, Parse, Config, Vault, Search, Cas, Agent, Io)
- `config.rs` — Configuration loading: `AppConfig` from `config.toml`
- `id.rs` — ULID-based ID generation: `next_id()` and `NoteId` wrapper

**Public API:**
```rust
pub struct NoteId(pub Ulid);
pub enum ParaCategory { Projects, Areas, Resources, Archives, Inbox, Zettelkasten }
pub enum NoteStatus { Seed, Budding, Evergreen }
pub struct FrontMatter { id, title, created, modified, tags, para, links, status }
pub enum AgenticError { NotFound, Parse, Config, Vault, Search, Cas, Agent, Io }
pub type Result<T> = std::result::Result<T, AgenticError>;
pub struct AppConfig { vault, llm, agent }
pub fn next_id() -> NoteId;
```

**No External Dependencies:** Only serde, ulid, chrono, serde_yaml for parsing.

---

#### 2. **vault** — Note Storage & Organization
**Purpose:** Manage note CRUD operations, PARA folder structure, YAML frontmatter parsing, and vault lifecycle.

**Key Modules:**
- `lib.rs` — `Vault` struct: the main interface for vault operations
- `note.rs` — `Note` struct with create/read/update/delete operations
- `frontmatter.rs` — Parse/serialize YAML frontmatter
- `para.rs` — PARA folder validation and path resolution
- `markdown.rs` — Markdown utilities (link extraction, body parsing)
- `init.rs` — Vault initialization with directory structure

**Public API:**
```rust
pub struct Vault { root, config }
pub struct Note { id, frontmatter, body, path }
pub struct NoteSummary { id, title, para, tags, status, modified, path }
pub struct NoteFilter { para, tags, status }

impl Vault {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn list_notes(&self, filter: &NoteFilter) -> Result<Vec<NoteSummary>>;
}

impl Note {
    pub fn create(vault: &Path, title: &str, para: ParaCategory, body: &str, tags: Vec<String>) -> Result<Note>;
    pub fn read(path: &Path) -> Result<Note>;
    pub fn update(&mut self) -> Result<()>;
    pub fn delete(path: &Path) -> Result<()>;
}
```

**Storage Format:** Plain `.md` files with YAML frontmatter:
```markdown
---
id: 01ARZ3NDEKTSV4RRFFQ69G5FAV
title: "My Note"
created: 2026-02-13T00:00:00Z
modified: 2026-02-13T00:00:00Z
para: projects
tags: [rust, cli]
links: [01ARZ3NDEKTSV4RRFFQ69G5FB0]
status: seed
---

Note body in Markdown...
```

---

#### 3. **cas** — Content-Addressable Storage
**Purpose:** Provide versioning, snapshots, diffing, and conflict resolution through SHA-256 hashing.

**Key Modules:**
- `hash.rs` — SHA-256 hashing: `hash_bytes()`, `hash_file()`
- `blob.rs` — `BlobStore`: store and retrieve content blobs
- `tree.rs` — `Tree` and `TreeEntry`: directory-like structures (vault snapshot)
- `snapshot.rs` — `Snapshot`: represents vault state at a point in time
- `diff.rs` — `diff_trees()`: compute changes between snapshots
- `merge.rs` — `three_way_merge()`: resolve conflicts
- `restore.rs` — `restore()`: revert vault to previous snapshot
- `cas.rs` — `Cas` main interface: coordinate blob, tree, snapshot operations

**Key Types:**
```rust
pub type ObjectId = String;  // SHA-256 hex hash

pub struct Blob { id, content }
pub struct TreeEntry { name, type (Blob/Tree), object_id }
pub struct Tree { id, entries }
pub struct Snapshot { id, root_tree, created, notes_count }
pub enum DiffStatus { Added, Modified, Deleted }
pub struct DiffEntry { path, status, old_id, new_id }

pub struct Cas { blob_store, snapshot_db }

impl Cas {
    pub fn create_snapshot(&self, vault_path: &Path) -> Result<Snapshot>;
    pub fn diff(&self, snap_a: &Snapshot, snap_b: &Snapshot) -> Result<Vec<DiffEntry>>;
    pub fn restore(&self, vault_path: &Path, snapshot: &Snapshot) -> Result<()>;
    pub fn three_way_merge(&self, base, mine, theirs) -> Result<MergeResult>;
}
```

**Design Decisions:**
- **No git-style merge markers** — User selects A or B for conflicts
- **Immutable snapshots** — Each snapshot is read-only reference point
- **Tree structure** — Enables efficient diffing and partial restores

---

#### 4. **search** — Full-Text Search & Graph Indexing
**Purpose:** Index notes for FTS and maintain tag/link relationships in a graph database.

**Key Modules:**
- `fts.rs` — `FtsIndex`: tantivy full-text search implementation
- `graph.rs` — `Graph`: SQLite-backed tag and backlink indexing
- `reindex.rs` — Incremental reindexing logic
- `lib.rs` — `SearchEngine` facade combining FTS and graph

**Public API:**
```rust
pub struct SearchResult { id, title, score }
pub struct SearchEngine { fts, db }

impl SearchEngine {
    pub fn open(vault_path: &Path) -> Result<Self>;
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
    pub fn index_note(&self, note: &Note) -> Result<()>;
    pub fn reindex_all(&self, vault: &Vault) -> Result<()>;
    pub fn get_backlinks(&self, note_id: NoteId) -> Result<Vec<NoteId>>;
    pub fn get_orphaned_notes(&self) -> Result<Vec<NoteId>>;
}
```

**Storage:**
- **FTS Index:** tantivy inverted index in `.agentic/tantivy/`
- **Graph DB:** SQLite tables in `.agentic/index.db` for tags and links

**Indexing Strategy:**
- Create index on note creation
- Increment index on note update
- Full reindex available for repair

---

#### 5. **agent** — AgentSpace Engine & LLM Integration
**Purpose:** Execute pipelines of sequential agents with LLM-powered transformations and review gates.

**Key Modules:**
- `engine.rs` — `AgentSpace`: pipeline loader and executor
- `llm/` — Provider implementations (OpenAI, Anthropic, Ollama)
- `agents/` — 4 built-in agents (para-classifier, zettelkasten-linker, distiller, vault-writer)

**Core Types:**
```rust
pub struct PipelineConfig {
    name: String,
    enabled: bool,
    trigger: TriggerConfig,
    stages: Vec<StageConfig>,
}

pub struct StageConfig {
    name: String,
    agent: String,
    llm_provider: String,
    trust_level: TrustLevel,  // Manual | Review | Auto
    output: String,
}

pub struct StageContext {
    stage_name: String,
    inputs: Map<String, Value>,  // Previous stage output
    vault: Arc<Vault>,
    search: Arc<SearchEngine>,
    llm: Arc<dyn LlmProvider>,
}

pub enum StageOutput {
    NoteChanges(Vec<NoteChange>),
    Suggestions(Vec<Suggestion>),
    Metadata(Map<String, Value>),
}

pub trait AgentHandler: Send + Sync {
    async fn execute(&self, context: &StageContext) -> Result<StageOutput>;
}

pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String>;
    async fn complete_with_schema(&self, prompt: &str, schema: &str) -> Result<String>;
}
```

**Built-in Agents:**

| Agent | Input | Output | Example |
|-------|-------|--------|---------|
| **para-classifier** | Inbox note body | Suggested PARA category | "This project task → projects" |
| **zettelkasten-linker** | Note body | Extracted atomic concepts + links | Detects `[[id]]` refs, suggests similar notes |
| **distiller** | Full note | Concise summary | Summarizes lengthy notes |
| **vault-writer** | Search query results | New synthesis note | Creates "daily synthesis" from 5 related notes |

**Pipeline Execution Flow:**
```
Load TOML → Validate Structure → For each Stage:
    ├─ Load LLM Provider
    ├─ Instantiate Agent
    ├─ Pass StageContext (previous output)
    ├─ Execute Agent
    ├─ Collect Output
    ├─ Enqueue for Review (based on trust_level)
    └─ Pass to next Stage
```

---

#### 6. **review** — Review Queue & Approval Gate
**Purpose:** Manage human-in-the-loop approval for agent-proposed changes with three trust levels.

**Key Modules:**
- `queue.rs` — `ReviewQueue` and `ReviewItem` storage in SQLite
- `gate.rs` — `gate()` function: approval logic based on trust level

**Public API:**
```rust
pub struct ReviewItem {
    id: String,
    pipeline_name: String,
    stage_name: String,
    proposed_change: NoteChange,
    created: DateTime<Utc>,
    status: ReviewStatus,  // Pending | Approved | Rejected | Modified
}

pub enum TrustLevel { Manual, Review, Auto }

pub enum GateAction { Approve, Review, Auto }
pub enum GateResult { Approved(Value), Pending, Rejected }

pub async fn gate(
    item: &ReviewItem,
    trust_level: TrustLevel,
    review_queue: &ReviewQueue,
) -> Result<GateResult>;

impl ReviewQueue {
    pub fn new(vault_path: &Path) -> Result<Self>;
    pub fn enqueue(&self, item: ReviewItem) -> Result<()>;
    pub fn list_pending(&self) -> Result<Vec<ReviewItem>>;
    pub fn approve(&self, item_id: &str) -> Result<()>;
    pub fn reject(&self, item_id: &str) -> Result<()>;
}
```

**Trust Levels:**
- **Manual:** All changes queued, require explicit approval
- **Review:** Selected stages queued, others auto-approved
- **Auto:** All changes auto-approved (safe agents only)

**Storage:** SQLite table in `.agentic/index.db`

---

#### 7. **cli** — Command-Line Interface & MCP Server
**Purpose:** User-facing CLI commands and MCP server for AI assistant integration.

**Key Modules:**
- `main.rs` — Entry point, clap CLI parsing
- `commands/` — Command implementations (init, note, config, agent)
- `mcp/` — MCP JSON-RPC server implementation
- `output.rs` — JSON/human output formatting

**CLI Commands:**
```bash
agentic-note init ~/vault              # Initialize vault
agentic-note note create \
  --title "My Note" \
  --para inbox \
  --tags rust,cli                      # Create note

agentic-note note list                 # List all notes
agentic-note note list --para inbox    # Filter by PARA
agentic-note note search "rust"        # Full-text search
agentic-note config show               # Display config
agentic-note mcp serve                 # Start MCP server
```

**MCP Server Tools:**
```
note/create      - Create a new note
note/read        - Read a specific note
note/list        - List notes with optional filtering
note/search      - Full-text search
vault/init       - Initialize a vault
vault/status     - Get vault statistics
```

**Output Modes:**
- **Human:** Formatted text, tables, colors
- **JSON:** `--json` flag, script-friendly format

---

## Data Flow

### Note Creation Flow
```
User Input (CLI)
    ↓
[CLI Command: note create]
    ↓
[Vault::create_note()]
    ├─ Generate ULID for ID
    ├─ Build FrontMatter
    ├─ Write .md file to disk
    └─ Return Note
    ↓
[SearchEngine::index_note()]
    ├─ Index to tantivy
    └─ Update SQLite graph
    ↓
[Pipeline Trigger Check]
    └─ If trigger matches, queue AgentSpace pipeline
    ↓
[Review Queue (if trust_level = Review/Manual)]
    └─ Queue changes for approval
    ↓
Human Review → Approve → Apply Changes
```

### Pipeline Execution Flow
```
Load Pipeline Config (.toml)
    ↓
For Each Stage:
    ├─ Load StageContext
    │  ├─ Vault reference
    │  ├─ SearchEngine reference
    │  ├─ LLM Provider (OpenAI/Anthropic/Ollama)
    │  └─ Previous stage output as inputs
    ├─ Execute AgentHandler
    ├─ Collect StageOutput
    ├─ Apply Trust Level Gate
    │  ├─ Auto → Apply immediately
    │  ├─ Review → Queue for human review
    │  └─ Manual → Queue, require explicit approval
    └─ Pass Output to Next Stage
    ↓
Pipeline Complete
    ↓
[Metrics logged via tracing]
```

### Search Flow
```
User Query
    ↓
[SearchEngine::search_fts()]
    ├─ Parse query
    ├─ Search tantivy index
    ├─ Rank results by score
    └─ Return top-k SearchResults
    ↓
[Graph queries]
    ├─ Get backlinks from SQLite
    ├─ Query tag frequency
    └─ Detect orphans
    ↓
Output (JSON or formatted table)
```

### CAS Snapshot & Restore Flow
```
[Cas::create_snapshot()]
    ├─ Walk vault tree
    ├─ Hash each file → BlobStore
    ├─ Build TreeEntry for each blob
    ├─ Create Tree (SHA-256 of entries)
    ├─ Create Snapshot (Tree + metadata)
    └─ Return Snapshot
    ↓
[Later: User wants to restore]
    ↓
[Cas::restore()]
    ├─ Load target Snapshot
    ├─ Read Blobs from BlobStore
    ├─ Write files back to vault
    └─ Return success
```

---

## Configuration

### File Structure
```
~/my-vault/
├── projects/
├── areas/
├── resources/
├── archives/
├── inbox/
├── zettelkasten/
└── .agentic/
    ├── config.toml          # Main config
    ├── index.db             # SQLite (FTS + graph + review queue)
    ├── tantivy/             # tantivy index directory
    ├── cas/
    │   ├── blobs/           # SHA-256 blob files
    │   └── snapshots/       # Snapshot metadata
    └── logs/
```

### config.toml
```toml
[vault]
path = "."

[llm]
default_provider = "openai"

[llm.providers.openai]
api_key = "sk-..."
model = "gpt-4o"
temperature = 0.7

[llm.providers.anthropic]
api_key = "sk-ant-..."
model = "claude-3-opus-20240229"
temperature = 0.7

[llm.providers.ollama]
base_url = "http://localhost:11434"
model = "llama2"

[agent]
default_trust = "review"
max_concurrent_pipelines = 1
```

---

## Error Handling Strategy

### Error Hierarchy
```
AgenticError (from core)
├── NotFound(String)        // Note/resource not found
├── Parse(String)           // YAML/JSON parsing failed
├── Config(String)          // Configuration invalid
├── Vault(String)           // Vault operation failed
├── Search(String)          // Search/indexing failed
├── Cas(String)             // CAS operation failed
├── Agent(String)           // Agent execution failed
└── Io(String)              // File I/O failed
```

### Error Propagation
- Use `?` operator to bubble up to CLI handler
- Provide context with `.map_err()` when transitioning between crates
- Never ignore errors with `.unwrap()` in production code
- Return actionable error messages to user

---

## Testing Strategy

### Test Organization
- **Unit tests:** In each module with `#[cfg(test)]` blocks
- **Integration tests:** In `tests/` directories at crate roots
- **Fixtures:** Use `tempfile` crate for isolated test vaults

### Coverage Targets
- **core:** 100% (critical shared code)
- **vault:** 85%+ (CRUD operations)
- **search:** 80%+ (indexing logic)
- **cas:** 85%+ (snapshot/restore correctness)
- **agent:** 70%+ (LLM calls mocked)
- **review:** 80%+ (approval logic)
- **cli:** 70%+ (command parsing)

---

## Performance Characteristics

### Benchmarks
| Operation | Target | Current |
|-----------|--------|---------|
| Note creation | <50ms | ~20ms |
| FTS indexing/note | <100ms | ~80ms |
| Search (1k notes) | <1s | ~500ms |
| CAS snapshot (5k notes) | <2s | ~1.8s |
| Graph query (backlinks) | <200ms | ~100ms |
| Pipeline stage execution | <5s | Varies with LLM |

### Optimization Techniques
- **Incremental indexing:** Don't reindex all notes on update
- **Batch database operations:** SQLite transactions
- **Lazy loading:** Load note bodies only when needed
- **Stream search results:** Don't load all at once

---

## Security Considerations

### Data Isolation
- Vault stored locally, no cloud sync
- API keys in config with 0600 permissions
- Logs to stderr (not stdout, reserved for JSON-RPC)
- No sensitive data in structured logs

### LLM Provider Security
- API keys validated before use
- HTTPS required for network calls
- Request/response logging disabled for secrets
- Ollama local models (no network required)

### File Permissions
| Path | Permissions | Reason |
|------|-------------|--------|
| config.toml | 0600 | API keys |
| index.db | 0600 | Sensitive metadata |
| vault root | 0755 | Readable by user |
| note files | 0644 | User readable |

---

## Extension Points

### Adding a New LLM Provider
1. Implement `LlmProvider` trait in `agent/src/llm/`
2. Add config section to `config.toml`
3. Register in `AgentSpace::load_provider()`

### Adding a New Agent
1. Implement `AgentHandler` trait in `agent/src/agents/`
2. Define input/output schema
3. Register in pipeline configuration

### Adding a CLI Command
1. Add variant to `Commands` enum in `cli/src/main.rs`
2. Implement command module in `cli/src/commands/`
3. Add help text with clap attributes

---

## Known Limitations (MVP)

- **Sequential pipelines only** — No parallel stage execution
- **No P2P sync** — Local-only, deferred to v2
- **No conflict merge markers** — Pick A or B only
- **Single LLM per stage** — No fallback chain
- **No partial snapshots** — Restore entire vault or nothing

---

## Future Architecture Improvements (v2+)

1. **P2P Sync** — When iroh API stabilizes
2. **DAG Pipelines** — Parallel stages with dependency graphs
3. **Embeddings** — Semantic search with all-MiniLM-L6-v2
4. **Plugin System** — Load custom agents from external crates
5. **Database** — Optional postgres for large deployments
6. **Event Bus** — Publish/subscribe for pipeline coordination

