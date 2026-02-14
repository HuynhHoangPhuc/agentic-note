# Brainstorm: Agentic-First Note-Taking Application

## Problem Statement

Build a local-first, agentic note-taking app that stores data as markdown files, syncs via P2P, and provides AI-powered PKM workflows. Must be multi-platform (CLI-first → Desktop → Mobile), with the core as a Rust library exposable as CLI + MCP server.

## Research Reports

- [Obsidian/AnyType/P2P Sync](researcher-260213-1538-obsidian-anytype-p2p-sync.md)
- [PKM Methods & AI Automation](researcher-260213-1539-pkm-methods-ai-automation.md)
- [Agentic Note & Agent Collaboration](researcher-260213-1539-agentic-note-research.md)
- [Cross-Platform Core Library](researcher-260213-1539-cross-platform-core-library.md)
- [Agentic Architecture Patterns](researcher-260213-1550-agentic-architecture-patterns.md)
- [AgentSpace Pipeline Patterns](researcher-260213-1604-agentspace-pipeline-patterns.md)

---

## Confirmed Decisions (from brainstorm Q&A)

| Decision | Choice |
|---|---|
| Core language | Rust (team comfortable) |
| CLI interface | CLI + MCP server (dual interface) |
| Desktop framework | Tauri v2 (Rust + Web UI) |
| Sync strategy | Git-like versioning (custom CAS) |
| PKM framework | PARA + Zettelkasten hybrid |
| Agent autonomy | Human-in-the-loop (propose diffs, human approves) |
| Agent scoping | Per-project workspaces with parallel agents |
| Versioning backend | Custom content-addressed store (not git) |

---

## Recommended Architecture

### Layer Diagram

```
┌─────────────────────────────────────────────────────┐
│                   UI / Frontends                     │
│  ┌─────────┐  ┌──────────┐  ┌────────┐  ┌────────┐ │
│  │   CLI   │  │ Tauri v2 │  │ Mobile │  │  MCP   │ │
│  │ (human) │  │ (desktop)│  │(future)│  │(agents)│ │
│  └────┬────┘  └────┬─────┘  └───┬────┘  └───┬────┘ │
│       └─────────┬──┴────────────┴────────────┘      │
├─────────────────┴───────────────────────────────────┤
│              Core Library (Rust)                     │
│  ┌──────────────────────────────────────────────┐   │
│  │  Commands API (unified, JSON-serializable)   │   │
│  ├──────────────────────────────────────────────┤   │
│  │  ┌─────────┐ ┌────────┐ ┌────────────────┐  │   │
│  │  │  Vault  │ │  Sync  │ │    Agent       │  │   │
│  │  │ Manager │ │ Engine │ │  Orchestrator  │  │   │
│  │  └────┬────┘ └───┬────┘ └───────┬────────┘  │   │
│  │       │          │              │            │   │
│  │  ┌────┴────┐ ┌───┴─────┐ ┌─────┴──────┐    │   │
│  │  │Markdown │ │ Custom  │ │  LLM       │    │   │
│  │  │ Parser  │ │  CAS    │ │ Provider   │    │   │
│  │  │+YAML FM │ │(merkle) │ │ Registry   │    │   │
│  │  └─────────┘ └─────────┘ └────────────┘    │   │
│  │                                              │   │
│  │  ┌──────────┐ ┌──────────┐ ┌────────────┐   │   │
│  │  │ Search / │ │  PKM     │ │ Embedding  │   │   │
│  │  │ Index    │ │ Engine   │ │ Store      │   │   │
│  │  │(tantivy) │ │(PARA+ZK) │ │(sqlite-vec)│   │   │
│  │  └──────────┘ └──────────┘ └────────────┘   │   │
│  └──────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────┤
│                   Storage Layer                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │  .md     │  │  CAS     │  │  SQLite          │  │
│  │  files   │  │  objects  │  │  (index+embeddings)│ │
│  │  (vault) │  │  (sync)  │  │                  │  │
│  └──────────┘  └──────────┘  └──────────────────┘  │
├─────────────────────────────────────────────────────┤
│                   P2P Transport                      │
│  ┌──────────────────────────────────────────────┐   │
│  │  iroh (QUIC-based) / libp2p fallback         │   │
│  │  - Device discovery (mDNS LAN + relay)       │   │
│  │  - Account-based peer auth                    │   │
│  │  - Merkle tree diff sync                      │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

### 1. Data Model — Obsidian-Style Vault

```
vault/
├── .agentic/                    # System metadata (not synced raw)
│   ├── config.toml              # Vault config
│   ├── agent-sessions/          # Per-project agent conversation logs
│   └── index.db                 # SQLite: FTS + embeddings + graph
├── projects/                    # PARA: active projects
│   ├── my-startup/
│   │   ├── _index.md            # Project overview (auto-maintained)
│   │   ├── mvp-features.md
│   │   └── meeting-2026-02-13.md
├── areas/                       # PARA: ongoing responsibilities
│   ├── health/
│   └── career/
├── resources/                   # PARA: reference material
│   ├── rust-patterns.md
│   └── pkm-methods.md
├── archives/                    # PARA: completed/inactive
├── zettelkasten/                # Atomic notes (Zettelkasten)
│   ├── 202602131530-crdt-vs-ot.md
│   └── 202602131535-local-first-principles.md
└── inbox/                       # Capture inbox (CODE: Capture step)
    └── quick-thought-2026-02-13.md
```

**Note format:**
```markdown
---
id: 01HQXYZ...           # ULID (stable, sortable)
title: CRDT vs OT
created: 2026-02-13T15:30:00+07:00
modified: 2026-02-13T15:35:00+07:00
tags: [sync, distributed-systems]
para: resources           # PARA category
links: [202602131535]     # Zettelkasten links
status: evergreen         # seed → budding → evergreen
---

# CRDT vs OT

One atomic idea per note...
```

### 2. Core Library Modules (Rust)

| Module | Responsibility | Key Crate |
|---|---|---|
| `vault` | File I/O, YAML frontmatter parse, vault structure | `serde_yaml`, `pulldown-cmark` |
| `cas` | Content-addressed store, merkle tree, snapshots | Custom (SHA-256 blocks) |
| `sync` | P2P transport, device discovery, auth, diff/merge | `iroh` (QUIC) |
| `search` | Full-text search + semantic search | `tantivy`, `sqlite-vec` |
| `pkm` | PARA classification, Zettelkasten linking, CODE pipeline | Custom |
| `agent` | Orchestrator, LLM provider registry, tool execution | Custom + `reqwest` |
| `cli` | CLI commands + MCP server (stdio transport) | `clap`, `rmcp` |

### 3. CLI + MCP Dual Interface

Every command works for humans AND agents:

```bash
# Human use
agentic-note note create --title "My idea" --para inbox
agentic-note note search "distributed systems"
agentic-note note link 01HQX... 01HQY...
agentic-note agent run classify-inbox
agentic-note sync status
agentic-note sync now

# All commands support --json for agent consumption
agentic-note note search "CRDT" --json

# MCP server mode (for AI agents like Claude, Gemini)
agentic-note mcp serve --stdio
```

**MCP Tools exposed:**
- `note/create`, `note/read`, `note/update`, `note/delete`
- `note/search` (full-text + semantic)
- `note/link`, `note/unlink`
- `graph/query` (traverse knowledge graph)
- `agent/propose` (propose changes, returns diff)
- `agent/approve` (human confirms proposed changes)
- `vault/status`, `sync/status`

### 4. Dual-Mode Agent System

Two interaction modes — **Interactive** (default) and **AgentSpace** (opt-in autonomous):

```
┌─────────────────────────────────────────────────────────────┐
│                    Agent Orchestrator                        │
│                                                             │
│  ┌───────────────────┐     ┌────────────────────────────┐   │
│  │  MODE 1:          │     │  MODE 2:                   │   │
│  │  Interactive       │     │  AgentSpace                │   │
│  │  (default, HITL)  │     │  (opt-in, autonomous)      │   │
│  │                   │     │                            │   │
│  │  User asks →      │     │  File watcher triggers →   │   │
│  │  Agent proposes → │     │  Pipeline runs stages →    │   │
│  │  Show diff →      │     │  Changes queue in buffer → │   │
│  │  Human approves   │     │  Human batch-reviews       │   │
│  └────────┬──────────┘     └──────────┬─────────────────┘   │
│           └──────────┬───────────────┘                      │
│                      ▼                                      │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Shared Infrastructure                               │   │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐  │   │
│  │  │ LLM Provider │ │ Tool         │ │ Session      │  │   │
│  │  │ Registry     │ │ Registry     │ │ Manager      │  │   │
│  │  │ (OpenAI,     │ │ (note/*,     │ │ (per-project │  │   │
│  │  │  Anthropic,  │ │  graph/*,    │ │  contexts,   │  │   │
│  │  │  Ollama...)  │ │  pkm/*,      │ │  parallel    │  │   │
│  │  │             │ │  search/*)   │ │  execution)  │  │   │
│  │  └──────────────┘ └──────────────┘ └──────────────┘  │   │
│  │                                                      │   │
│  │  ┌──────────────────────────────────────────────┐    │   │
│  │  │  Approval Gate (shared by both modes)        │    │   │
│  │  │  ┌────────────────────────────────────────┐  │    │   │
│  │  │  │ Trust Levels (per pipeline/stage):     │  │    │   │
│  │  │  │  manual  → every action needs approval │  │    │   │
│  │  │  │  review  → execute, queue for batch OK │  │    │   │
│  │  │  │  auto    → commit directly (CAS undo)  │  │    │   │
│  │  │  └────────────────────────────────────────┘  │    │   │
│  │  └──────────────────────────────────────────────┘    │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**Mode 1: Interactive (default)** — Human-in-the-loop
- User explicitly asks agent to do something via CLI or MCP
- Agent proposes changes, shows diff
- Human approves/rejects each action before execution
- Trust level: `manual` (always)

```bash
# Interactive mode examples
agentic-note agent classify my-note.md
# → "Suggest: move to projects/startup/, tags: [mvp, planning]"
# → [approve/reject]

agentic-note agent link-suggest my-note.md
# → "Found 3 related notes: ..."
# → [approve selected / reject]
```

**Mode 2: AgentSpace (opt-in)** — Autonomous pipelines
- User activates: `agentic-note agentspace start`
- File watcher active, pipelines trigger automatically
- Stages execute without blocking for per-step approval
- Changes queue in review buffer based on trust level
- User reviews when ready: `agentic-note review list`

```bash
# AgentSpace lifecycle
agentic-note agentspace start          # activate watcher + pipelines
agentic-note agentspace status         # show active pipelines, queue depth
agentic-note agentspace stop           # deactivate

# Review queue (shared by both modes)
agentic-note review list               # pending changes from agents
agentic-note review show <id>          # show diff for specific change
agentic-note review approve <id>       # approve single change
agentic-note review approve --all      # batch approve all
agentic-note review reject <id>        # reject change
```

**Trust levels (configurable per pipeline & per stage):**

| Level | Behavior | Use case |
|---|---|---|
| `manual` | Block until human approves (Interactive mode default) | Delete, restructure, merge |
| `review` | Execute all stages, queue results for batch review (AgentSpace default) | Classify, link, summarize |
| `auto` | Execute and commit immediately, versioned in CAS (reversible) | Update tags, refresh embeddings, index |

**Key patterns applied:**
- **Orchestrator + parallel workers** (Claude Code): complex tasks spawn sub-tasks
- **Thread isolation** (AmpCode): each project session = isolated context
- **HITL gates on mutations** (research): tiered trust levels
- **Persistent memory** (Windsurf): cross-session agent memory per project
- **Dual-mode** (new): same engine, different autonomy — user controls the knob

### 5. AgentSpace — Pipeline Engine Details

**Architecture: 3 primitives**
- **Stage** — single agent task (classify, link, summarize, etc.)
- **Pipeline** — ordered sequence of stages with shared context
- **Trigger** — event that starts a pipeline (file created, modified, cron, manual)

**Default pipeline (`auto-process-inbox`):**
```
inbox/ file created
    → [1] para-classifier    (suggest PARA category + tags)
    → [2] zettelkasten-linker (find related notes via embeddings, suggest links)
    → [3] distiller           (extract key ideas, progressive summary)
    → [4] vault-writer        (move file to correct PARA folder, update links)
    → HITL review queue       (human approves/rejects all proposed changes)
```

**Pipeline config (TOML):**
```toml
[pipeline]
name = "auto-process-inbox"
description = "Classify, link, and distill new captures"

[trigger]
type = "file_created"
path_filter = "inbox/**"

[[stages]]
name = "classify"
agent = "para-classifier"
description = "Classify note into PARA category"

[[stages]]
name = "auto-link"
agent = "zettelkasten-linker"
input = "classify"        # receives output of previous stage
description = "Find and suggest related note links"

[[stages]]
name = "distill"
agent = "distiller"
input = "auto-link"
description = "Extract key ideas, create summary layer"

[[stages]]
name = "write"
agent = "vault-writer"
input = "distill"
requires_approval = true  # HITL gate
description = "Move to PARA folder, update frontmatter + links"
```

**Built-in agents (MVP):**

| Agent | Input | Output |
|---|---|---|
| `para-classifier` | Raw note | PARA category + tags suggestion |
| `zettelkasten-linker` | Note + embeddings index | List of related note IDs + link rationale |
| `distiller` | Note content | Key ideas, summary, potential atomic notes |
| `vault-writer` | All stage outputs | File moves, frontmatter updates, link insertions |

**Future pipelines (post-MVP):**
- `weekly-review` — stale detection, orphan rescue, project status digest
- `daily-feed` — generate personalized knowledge feed (recent + serendipity)
- `spaced-repetition` — extract flashcards, schedule reviews (SM-2)
- `moc-generator` — auto-create Maps of Content when clusters reach critical mass

**Event system (Rust):**
- `notify` crate for file watching (500ms debounce)
- `tokio::mpsc` bounded channel for event queue
- Max 3 concurrent pipeline tasks
- Each stage passes `StageContext` struct (shared typed output)

**Review Queue:**
- All HITL-gated changes queue in SQLite
- CLI: `agentic-note review list` / `agentic-note review approve <id>`
- Batch approval mode for bulk operations
- MCP: `review/list`, `review/approve`, `review/reject`

### 6. PKM Engine — PARA + Zettelkasten Hybrid

**Automated workflows via AgentSpace pipelines:**

| Workflow | Trigger | Pipeline |
|---|---|---|
| **Capture → Classify** | New note in inbox/ | `auto-process-inbox` |
| **Auto-link** | Note saved | `auto-process-inbox` stage 2 |
| **Progressive Summarize** | Note status change | `auto-process-inbox` stage 3 |
| **Orphan Rescue** | Weekly cron | `weekly-review` (future) |
| **Stale Detection** | Weekly cron | `weekly-review` (future) |
| **MOC Generation** | Cluster > 5 notes | `moc-generator` (future) |
| **Flashcard Gen** | User request | `spaced-repetition` (future) |
| **Knowledge Graph** | On every link change | Built-in (not agent, direct index update) |

### 6. Sync Engine — Custom CAS + P2P

```
Content-Addressed Store (like git but simpler):

  blob(sha256) → raw file content
  tree(sha256) → directory listing (name → blob/tree hash)
  snapshot     → root tree hash + timestamp + device_id + signature

Sync protocol:
  1. Device A sends latest snapshot hash
  2. Device B compares merkle trees
  3. Exchange only differing blobs
  4. Three-way merge for conflicts (common ancestor + both versions)
  5. Conflict → create .conflict file, human resolves

Transport: iroh (Rust-native QUIC, built-in NAT traversal, relay fallback)
Auth: Ed25519 keypair per device, account = set of authorized device keys
```

**Why iroh over libp2p:**
- Pure Rust, no C dependencies
- QUIC-native (faster, better NAT traversal)
- Simpler API (iroh is purpose-built for content sync)
- Used by Fission, Number0 — proven for exactly this use case

### 8. MVP Scope

**MVP delivers:**
1. Rust core library with vault management
2. CLI with all note CRUD + search commands
3. MCP server mode (stdio)
4. PARA folder structure + Zettelkasten linking
5. AgentSpace engine — TOML pipeline config, file watcher, stage executor
6. Default pipeline: `auto-process-inbox` (4 built-in agents)
7. Review queue (HITL approval gate via CLI + MCP)
8. Custom CAS for versioning (local only)
9. P2P sync between 2+ devices (same account, keypair auth)
10. SQLite index (FTS via tantivy, embeddings via sqlite-vec, pluggable local default)

**MVP defers:**
- TUI interface
- Desktop app (Tauri)
- Mobile app / Web app
- Advanced pipelines (weekly-review, daily-feed, spaced-repetition, moc-generator)
- Backup/relay server
- Custom agent plugins (user-written agents)
- Visual pipeline editor
- Conditional/branching pipeline stages
- Real-time collaborative editing

### 9. MVP Milestone Breakdown

| # | Milestone | Description |
|---|---|---|
| M1 | **Vault & Notes** | File I/O, markdown parser, YAML frontmatter, ULID generation, PARA folder structure |
| M2 | **CLI Interface** | clap-based CLI, all CRUD commands, --json output, config management |
| M3 | **Search & Index** | Tantivy FTS, tag/link graph in SQLite, embedding store (sqlite-vec), pluggable embedding model |
| M4 | **CAS & Versioning** | Content-addressed store, merkle tree, snapshot/restore, conflict detection |
| M5 | **P2P Sync** | iroh transport, device auth (Ed25519), merkle diff sync, conflict files |
| M6 | **AgentSpace Engine** | TOML pipeline parser, file watcher (notify), stage executor, StageContext, event queue |
| M7 | **Agent Core + Built-in Agents** | LLM provider registry, tool registry, 4 built-in agents (classifier/linker/distiller/writer) |
| M8 | **Review Queue + HITL** | SQLite review queue, CLI approve/reject, batch mode, approval gate integration |
| M9 | **MCP Server** | stdio MCP transport, expose all tools + review queue, agent-consumable output |

---

## Approaches Evaluated

### Approach A: Rust Core + CLI + MCP (Recommended)

**Pros:**
- Single language for core logic, CLI, and future Tauri desktop
- Best ecosystem for P2P (iroh), search (tantivy), CRDT, content-addressing
- CLI IS the interface — no separate API layer needed
- MCP makes it instantly usable by any AI agent (Claude, Gemini, etc.)
- Smallest binary, fastest execution
- UniFFI for future Swift/Kotlin mobile bindings

**Cons:**
- Steeper learning curve for contributors
- Slower iteration vs TypeScript for UI work
- Ecosystem smaller than Node.js for some libraries

### Approach B: Go Core + CLI

**Pros:** Simpler concurrency, faster compile, good CLI ecosystem (cobra)
**Cons:** gomobile under-maintained, GC pauses during sync, no Tauri equivalent, P2P ecosystem thinner

### Approach C: TypeScript Core + Electron

**Pros:** Fastest iteration, largest ecosystem, web-ready
**Cons:** Electron bundle 150MB+, no real mobile core sharing, performance ceiling for CAS/sync, not a good CLI language

**Verdict:** Approach A is clearly superior given your Rust proficiency and the local-first/P2P/agentic requirements. The Rust ecosystem (iroh, tantivy, sqlite-vec, Tauri) maps 1:1 to every requirement.

---

## Risk Assessment

| Risk | Severity | Mitigation |
|---|---|---|
| P2P NAT traversal failures | High | iroh has built-in relay fallback; plan backup relay server early |
| Merge conflicts in sync | Medium | Three-way merge + .conflict files; human resolution UI in CLI |
| LLM API costs for agent features | Medium | Support local LLMs (Ollama); cache embeddings aggressively |
| iOS background sync restrictions | High (future) | Foreground-first design; APNs-triggered sync when app opens |
| Scope creep beyond MVP | High | Strict milestone gating; defer TUI/desktop/mobile/plugins |
| Agent hallucination on note edits | Medium | HITL approval gates; diff-based proposals; undo via CAS snapshots |

---

## Unique Value Proposition

What makes this different from Obsidian + AI plugins:

1. **Agent-native from day one** — not bolted-on AI. MCP server = any AI can interact
2. **CLI-first** — terminal users AND agents use same interface. No GUI required
3. **P2P sync without cloud** — your data never touches a server (MVP)
4. **Git-like versioning built-in** — every change tracked, reversible, mergeable
5. **Per-project agent workspaces** — parallel AI conversations scoped to context
6. **PKM methodology as code** — PARA/Zettelkasten rules enforced by the system, not discipline

---

## Resolved Questions

| Question | Decision |
|---|---|
| Account system | Keypair-only (Ed25519 per device, pair via key exchange) |
| Embedding model | Pluggable, local default (all-MiniLM-L6 ~80MB shipped) |
| Agent autonomy | Human-in-the-loop via review queue |
| Agent scoping | Per-project workspaces (PARA project = agent context) |
| Desktop framework | Tauri v2 (future, shares Rust core) |
| Workflow engine | AgentSpace — TOML-defined pipelines with stage executor |

## Unresolved Questions

1. **Conflict resolution UX**: CLI-based three-way merge or simpler "pick A or B"?
2. **Plugin system**: Defer entirely, or design hook points in AgentSpace now for future custom agents?
3. **License model**: Open-source core + commercial sync/backup, or fully open?
4. **LLM calls per pipeline**: One call per stage (granular retry) or batched (cheaper)?
5. **Pipeline error policy**: Skip failed stage and continue, or abort entire pipeline?
6. **Embedding refresh**: Per-write (real-time) or nightly batch (cheaper)?
