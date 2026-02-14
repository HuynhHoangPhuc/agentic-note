# Agentic-Note: System Architecture

## Overview

Agentic-note is a modular, local-first Rust application designed as a Cargo workspace with 8 specialized crates. Each crate has a single responsibility and communicates through well-defined interfaces. This document describes the architecture, crate relationships, and data flow for v0.2.0 and beyond.

---

## Crate Architecture

### Dependency Graph

```
                        ┌─────────────────────────────────────┐
                        │         CLI (Binary)                 │
                        │  • Commands (init, note, device)    │
                        │  • MCP Server / RPC                 │
                        │  • JSON/Human output                │
                        └──────────────┬──────────────────────┘
                                       │
         ┌─────────────────────────────┼─────────────────────────────┐
         │                             │                             │
         ▼                             ▼                             ▼
    ┌──────────────┐          ┌─────────────────┐         ┌──────────────────┐
    │    Vault     │          │     Search      │         │      Agent       │
    │              │          │                 │         │                  │
    │ • Note CRUD  │          │ • tantivy FTS   │         │ • DAG Executor   │
    │ • Frontmatter│          │ • Embeddings*   │         │ • Plugin system  │
    │ • PARA       │          │ • Semantic*     │         │ • Error recovery │
    │ • Markdown   │          │ • SQLite graph  │         │ • LLM providers  │
    └──────┬───────┘          └────┬────────────┘         └──────┬───────────┘
           │                       │                              │
           │               ┌───────┴────────┐                    │
           │               │                │                    │
           └───────────────┼────────────────┼────────────────────┘
                           │                │
                ┌──────────┼──────────┐     │
                │          │          │     │
                ▼          ▼          ▼     ▼
            ┌────────┐ ┌──────┐ ┌─────────┐┌──────────┐
            │  CAS   │ │Core  │ │ Review  ││  Sync    │
            │        │ │      │ │         ││          │
            │ • Hash │ │Types │ │ • Queue ││ • Iroh   │
            │ • Blob │ │Errors│ │ • Gate  ││ • Device │
            │ • Tree │ │Config││         ││ • Merge  │
            │ • Snap │ │Ulids │ │         ││ • Protocol
            └────────┘ └──────┘ └─────────┘└──────────┘

Legend: * = optional with embeddings feature
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

#### 4. **search** — Full-Text & Semantic Search (v0.2.0)
**Purpose:** Index notes for FTS, semantic search, and maintain tag/link relationships in a graph database.

**Key Modules:**
- `fts.rs` — `FtsIndex`: tantivy full-text search
- `graph.rs` — `Graph`: SQLite-backed tag and backlink indexing
- `embedding.rs` — (optional, behind `embeddings` feature) ONNX Runtime embeddings
- `hybrid.rs` — (optional) Combined FTS + semantic scoring
- `model_download.rs` — (optional) Auto-download all-MiniLM-L6-v2 ONNX model
- `reindex.rs` — Incremental reindexing logic

**Public API:**
```rust
pub struct SearchResult { id, title, score }
pub struct SearchEngine { fts, db, embedding_index? }

impl SearchEngine {
    pub fn open(vault_path: &Path) -> Result<Self>;

    // FTS search
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;

    // Semantic search (requires embeddings feature)
    pub async fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;

    // Hybrid: FTS + semantic with combined scoring
    pub async fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;

    pub fn index_note(&self, note: &Note) -> Result<()>;
    pub fn reindex_all(&self, vault: &Vault) -> Result<()>;
    pub fn get_backlinks(&self, note_id: NoteId) -> Result<Vec<NoteId>>;
    pub fn get_orphaned_notes(&self) -> Result<Vec<NoteId>>;
}
```

**Storage:**
- **FTS Index:** tantivy inverted index in `.agentic/tantivy/`
- **Embeddings:** ONNX model in `~/.cache/agentic-note/models/` (auto-downloaded)
- **Graph DB:** SQLite tables in `.agentic/index.db` for tags, links, embeddings

**Search Modes:**
- **FTS:** Keyword-based (default, always available)
- **Semantic:** Vector similarity (requires `embeddings` feature)
- **Hybrid:** Combined FTS + semantic scoring (requires `embeddings` feature)

**Indexing Strategy:**
- Create index on note creation
- Increment index on note update
- Full reindex available for repair
- Embeddings computed lazily on first semantic search

---

#### 5. **agent** — AgentSpace Engine, DAG Pipelines & LLM Integration
**Purpose:** Execute DAG pipelines with parallel stages, LLM-powered transformations, error recovery, and plugin system.

**Key Modules:**
- `engine/` — Core types and validators
- `engine/dag_executor.rs` — Topological sort + parallel execution
- `engine/error_policy.rs` — Retry/skip/abort/fallback strategies
- `engine/condition.rs` — Conditional stage execution
- `llm/` — Provider implementations (OpenAI, Anthropic, Ollama)
- `agents/` — 4 built-in agents (para-classifier, zettelkasten-linker, distiller, vault-writer)
- `plugin/` — Plugin discovery, manifest, and subprocess execution

**Core Types (v2.0 Pipeline Schema):**
```rust
pub struct PipelineConfig {
    name: String,
    enabled: bool,
    trigger: TriggerConfig,
    stages: Vec<StageConfig>,
    schema_version: u32,      // 1 = sequential, 2 = DAG
    default_on_error: ErrorPolicy,
}

pub struct StageConfig {
    name: String,
    agent: String,
    config: toml::Value,       // Agent-specific config
    output: String,            // Output key in context
    depends_on: Vec<String>,   // DAG edges (stage names)
    condition: Option<String>, // Expression evaluation
    on_error: ErrorPolicy,     // Retry/skip/abort/fallback
    retry_max: u32,
    retry_backoff_ms: u64,
    fallback_agent: Option<String>,
}

pub enum ErrorPolicy {
    Skip,                      // Continue to next stage
    Retry,                     // Retry with backoff
    Abort,                     // Abort entire pipeline
    Fallback,                  // Use fallback_agent
}

pub trait AgentHandler: Send + Sync {
    async fn execute(&self, context: &StageContext) -> Result<StageOutput>;
    fn agent_id(&self) -> &str;
}

pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String>;
    async fn complete_with_schema(&self, prompt: &str, schema: &str) -> Result<String>;
}
```

**Built-in Agents:**

| Agent | Module | Input | Output |
|-------|--------|-------|--------|
| **para-classifier** | `agents/para_classifier.rs` | Inbox note body | Suggested PARA category |
| **zettelkasten-linker** | `agents/zettelkasten_linker.rs` | Note body | Extracted atomic concepts + links |
| **distiller** | `agents/distiller.rs` | Full note | Concise summary |
| **vault-writer** | `agents/vault_writer.rs` | Search results | New synthesis note |

**Plugin System:**
- Manifest-driven: `plugin.toml` with name, version, executable, timeout
- Subprocess execution: JSON-RPC over stdio
- Discovery: auto-scan `~/.agentic/plugins/` or specified directories

**DAG Pipeline Execution:**
```
Load TOML → Build DAG (toposort) → For each Layer (parallel):
    ├─ Check conditions
    ├─ Spawn agents in parallel
    ├─ Wait for completion
    ├─ Apply error policies (retry/skip/abort/fallback)
    ├─ Merge outputs into context
    └─ Continue to next layer
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

#### 7. **sync** — P2P Sync via iroh (NEW)
**Purpose:** Peer-to-peer vault synchronization with device identity, conflict resolution, and merge orchestration.

**Key Modules:**
- `identity.rs` — Ed25519 device keypair generation and peer ID derivation
- `device_registry.rs` — Known devices TOML/JSON persistence
- `transport.rs` — Abstract sync protocol trait
- `iroh_transport.rs` — QUIC-based iroh binding (endpoint + node)
- `protocol.rs` — Sync request/response messages
- `merge_driver.rs` — CAS-aware three-way merge orchestration

**Core Types:**
```rust
pub struct DeviceIdentity {
    pub secret_key: SigningKey,
    pub peer_id: PeerId,
    pub created: DateTime<Utc>,
}

pub struct KnownDevice {
    pub peer_id: PeerId,
    pub name: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
}

pub enum ConflictPolicy {
    NewestWins,                // Latest modified wins
    LongestWins,               // Longest note wins
    MergeBoth,                 // Merge both versions
    Manual,                    // User selects A or B
}

pub trait SyncTransport: Send + Sync {
    async fn connect(&self, peer: PeerId) -> Result<SyncConnection>;
    async fn listen(&self) -> Result<SyncConnection>;
}

pub struct SyncEngine {
    pub identity: DeviceIdentity,
    pub registry: DeviceRegistry,
    pub transport: Box<dyn SyncTransport>,
    pub cas: Cas,
    vault_path: PathBuf,
}
```

**Sync Flow:**
```
Peer A                         Peer B
├─ Create snapshot A          ├─ Create snapshot B
├─ Query snapshots via iroh   ├─ Receive sync request
├─ Receive snapshot B         ├─ Apply conflict policy
├─ Determine common base      ├─ Return merged snapshot
├─ Three-way merge via CAS
└─ Apply & persist
```

**Security:**
- Ed25519 per-device keypair
- Peer ID = public key hash
- iroh QUIC encryption (TLS 1.3)
- Device registry for trusted peers only

---

#### 8. **cli** — Command-Line Interface & MCP Server
**Purpose:** User-facing CLI commands and MCP server for AI assistant integration.

**Key Modules:**
- `main.rs` — Entry point, clap CLI parsing
- `commands/` — Command implementations (init, note, config, agent, device, sync, plugin, mcp)
- `mcp/` — MCP JSON-RPC server implementation
- `output.rs` — JSON/human output formatting

**CLI Commands:**
```bash
# Vault & Notes
agentic-note init ~/vault              # Initialize vault
agentic-note note create --title "My Note" --para inbox --tags rust,cli
agentic-note note list [--para <PARA>] [--tags <TAGS>]
agentic-note note search <QUERY> [--mode hybrid]
agentic-note note read <NOTE_ID>

# Device & Sync (NEW)
agentic-note device init               # Generate Ed25519 identity
agentic-note device show               # Display peer ID
agentic-note device pair <PEER_ID> [--name "Device Name"]
agentic-note device list               # Show known devices
agentic-note device unpair <PEER_ID>   # Remove peer
agentic-note sync now [--peer <PEER_ID>] [--policy newest-wins]
agentic-note sync status               # Check sync state

# Plugins (NEW)
agentic-note plugin list               # Show installed plugins
agentic-note plugin run <PLUGIN> [--config <TOML>]

# Configuration
agentic-note config show               # Display config
agentic-note mcp serve                 # Start MCP server
```

**MCP Server Tools:**
```
note/create      - Create a new note
note/read        - Read a specific note
note/list        - List notes with optional filtering
note/search      - Full-text search (with mode param: fts/semantic/hybrid)
vault/init       - Initialize a vault
vault/status     - Get vault statistics
plugin/list      - List installed plugins
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

## Known Limitations (v0.2.0)

- **Sequential conflict resolution** — Manual merge only (auto-policies upcoming)
- **Single vault per sync session** — Multi-vault sync deferred to v0.3
- **No compression** — iroh transfer uncompressed
- **Embeddings optional** — Not all deployments need semantic search
- **Plugin security** — No sandboxing (trust plugin authors)
- **No partial snapshots** — Restore entire vault or nothing

---

## Future Architecture Improvements (v0.3+)

1. **Batch sync** — Multi-peer simultaneous sync
2. **Compression** — Delta-based iroh sync
3. **Auto-merge policies** — Semantic-aware conflict resolution
4. **Plugin sandboxing** — WebAssembly or lightweight container isolation
5. **PostgreSQL optional** — For large deployments (10k+ notes)
6. **Event bus** — Publish/subscribe for pipeline coordination and webhooks

