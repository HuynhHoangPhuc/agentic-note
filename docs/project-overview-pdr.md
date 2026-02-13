# Agentic-Note: Project Overview & Product Development Requirements

## Overview

**agentic-note** is a local-first, agentic note-taking application written in Rust. It combines personal knowledge management (PKM) with AI-powered agents to organize, link, and enhance notes using PARA/Zettelkasten methods. The system is built as a CLI tool with a built-in Model Context Protocol (MCP) server for AI assistant integration.

**Core Value Proposition:**
- Organize notes with PARA (Projects/Areas/Resources/Archives) + Zettelkasten atomic notes
- Full-text search powered by tantivy FTS
- Content-addressable storage (CAS) for version control and snapshot management
- AgentSpace pipeline engine with 4 built-in agents for automated note processing
- Human-in-the-loop review queue for agent-proposed changes
- Local execution with support for OpenAI, Anthropic, and Ollama LLM providers
- MCP server for seamless AI assistant integration

---

## Product Requirements

### Functional Requirements

#### 1. Vault Management
- **FR-1.1**: Initialize a vault with PARA structure (projects, areas, resources, archives, inbox)
- **FR-1.2**: Support Zettelkasten folder for atomic notes
- **FR-1.3**: Load and validate vault configuration from `.agentic/config.toml`
- **FR-1.4**: Display vault status (total notes, indexed items, pipeline state)

#### 2. Note CRUD Operations
- **FR-2.1**: Create notes with YAML frontmatter (ID, title, tags, links, status)
- **FR-2.2**: Read/parse notes with frontmatter extraction
- **FR-2.3**: Update note metadata (title, tags, status, links)
- **FR-2.4**: Delete notes with proper cleanup
- **FR-2.5**: List notes with filtering by PARA category, tags, or status
- **FR-2.6**: Support three note maturity statuses: Seed, Budding, Evergreen

#### 3. Full-Text Search
- **FR-3.1**: Index notes using tantivy FTS
- **FR-3.2**: Search across note titles, bodies, and tags
- **FR-3.3**: Incremental indexing for new/modified notes
- **FR-3.4**: Support advanced query syntax (wildcard, phrase, boolean)

#### 4. Tag & Link Graph
- **FR-4.1**: Build tag frequency index in SQLite
- **FR-4.2**: Track backlinks (notes that reference other notes)
- **FR-4.3**: Detect orphaned notes (no tags, no links, no backlinks)
- **FR-4.4**: Query graph for related notes

#### 5. Content-Addressable Storage (CAS)
- **FR-5.1**: Store note blobs with SHA-256 hashing
- **FR-5.2**: Create snapshots (tree structures) of vault state
- **FR-5.3**: Compute diffs between snapshots
- **FR-5.4**: Restore notes from previous snapshots
- **FR-5.5**: Detect and resolve conflicts in merged snapshots

#### 6. AgentSpace Pipeline Engine
- **FR-6.1**: Load and validate TOML pipeline configurations
- **FR-6.2**: Execute sequential stages with data passing
- **FR-6.3**: Context management for agent inputs/outputs
- **FR-6.4**: Pipeline error handling with skip/warn policies

#### 7. Built-in Agents
- **FR-7.1**: para-classifier: Suggest PARA category for inbox notes
- **FR-7.2**: zettelkasten-linker: Extract atomic notes and suggest links
- **FR-7.3**: distiller: Summarize notes into concise versions
- **FR-7.4**: vault-writer: Create synthesis notes from query results

#### 8. LLM Integration
- **FR-8.1**: Support OpenAI API (gpt-4o, gpt-4-turbo, etc.)
- **FR-8.2**: Support Anthropic API (claude-3-opus, etc.)
- **FR-8.3**: Support Ollama local models
- **FR-8.4**: Configurable model selection per pipeline stage
- **FR-8.5**: JSON mode output with schema validation
- **FR-8.6**: Retry logic for LLM output parsing failures

#### 9. Review Queue (Human-in-the-Loop)
- **FR-9.1**: Queue agent-proposed changes with metadata
- **FR-9.2**: Support three approval modes: Manual, Review, Auto
- **FR-9.3**: Display pending changes with before/after diffs
- **FR-9.4**: Approve/reject/modify changes before applying
- **FR-9.5**: Track approval history and audit trail

#### 10. CLI Interface
- **FR-10.1**: `init` command to scaffold a new vault
- **FR-10.2**: `note create/read/update/delete/list` commands
- **FR-10.3**: `note search` command for full-text search
- **FR-10.4**: `config show` command to display current configuration
- **FR-10.5**: JSON output mode for script integration
- **FR-10.6**: Global `--vault` flag to specify vault location

#### 11. MCP Server
- **FR-11.1**: Implement JSON-RPC 2.0 stdio server for MCP protocol
- **FR-11.2**: Expose tools: note/create, note/read, note/list, note/search
- **FR-11.3**: Expose tools: vault/init, vault/status
- **FR-11.4**: Handle async MCP method calls
- **FR-11.5**: Error handling with proper JSON-RPC error responses

### Non-Functional Requirements

#### Performance
- **NFR-1.1**: Note indexing: <100ms per note for FTS
- **NFR-1.2**: Note listing: <500ms for <10k notes
- **NFR-1.3**: Full-text search: <1s for typical queries
- **NFR-1.4**: CAS snapshot creation: <2s for <5k notes
- **NFR-1.5**: Pipeline execution: <5s per stage (excluding LLM latency)

#### Security
- **NFR-2.1**: API keys stored with 0600 file permissions
- **NFR-2.2**: No credentials logged or printed to stdout
- **NFR-2.3**: Sensitive data isolated to `.agentic/` directory
- **NFR-2.4**: MCP server runs in isolation without network access

#### Scalability
- **NFR-3.1**: Support vaults with 10k+ notes
- **NFR-3.2**: Incremental indexing for fast updates
- **NFR-3.3**: Streaming search results for large result sets
- **NFR-3.4**: SQLite connection pooling for concurrent queries

#### Maintainability
- **NFR-4.1**: Modular crate structure (7 crates, <200 LOC per file)
- **NFR-4.2**: Comprehensive error handling with context
- **NFR-4.3**: Structured logging via tracing
- **NFR-4.4**: ~200 unit + integration tests covering critical paths

#### Reliability
- **NFR-5.1**: Graceful degradation if LLM provider unavailable
- **NFR-5.2**: Atomic writes to note files (no partial updates)
- **NFR-5.3**: SQLite transaction support for consistency
- **NFR-5.4**: CAS snapshot validation before apply

---

## Architecture Decisions

### Technology Stack
| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language | Rust | Type safety, performance, zero-cost abstractions |
| CLI | clap | Industry standard, derive macros, minimal overhead |
| Async Runtime | tokio | Mature, well-tested, MCP server support |
| FTS | tantivy | Pure Rust, no external dependencies, excellent performance |
| SQLite | rusqlite | Bundled support, no separate DB setup |
| Serialization | serde + YAML | Human-readable frontmatter, JSON-RPC compatibility |
| Logging | tracing | Structured, contextual, stderr isolation |
| HTTP Client | reqwest | Modern, async, TLS support |

### Key Design Decisions

1. **No P2P Sync in MVP** — Deferred to v2 because iroh API is unstable. MVP remains local-only.

2. **Plain Markdown + YAML Frontmatter** — Human-editable, version-control friendly, no custom binary format.

3. **Local CAS Versioning** — SHA-256 blobs/trees/snapshots. Essential for agent change reversibility and safety.

4. **TOML Pipelines** — User-friendly, Cargo-familiar, avoids JSON nesting.

5. **Sequential Pipelines** — No DAG branching in MVP. Simple linear execution, each stage passes data to next.

6. **JSON Mode LLM Output** — More reliable than regex extraction. Schema validation + retry on parse failure.

7. **Three Trust Levels** — Manual (all reviewed), Review (selected stages reviewed), Auto (all auto-approved).

8. **Review Queue in SQLite** — Persisted, queryable, no external service.

9. **Config in .agentic/config.toml** — Follows Unix conventions. API keys with 0600 permissions.

10. **MCP Server via stdio** — Simplest integration pattern for AI assistants. No port management.

---

## Success Metrics

### MVP Completion Criteria
- [ ] All 8 phases implemented and tested
- [ ] 29+ tests passing, 0 compiler warnings
- [ ] README + quick start guide complete
- [ ] Example vault with 5+ sample notes
- [ ] All 4 built-in agents functional
- [ ] MCP server passes validation tests
- [ ] Performance targets met (see NFR-1.x)

### Quality Gates
- [ ] No unsafe code except where unavoidable
- [ ] All public APIs documented with examples
- [ ] Error messages actionable (include recovery steps)
- [ ] Circular dependencies eliminated
- [ ] External crate count minimized

---

## Scope & Phases

### Completed (All Phases 01-08)
| Phase | Name | Status |
|-------|------|--------|
| 01 | Project Setup & Core Types | ✅ Complete |
| 02 | Vault & Notes | ✅ Complete |
| 03 | CLI Interface | ✅ Complete |
| 04 | Search & Index | ✅ Complete |
| 05 | CAS & Versioning | ✅ Complete |
| 06 | AgentSpace Engine | ✅ Complete |
| 07 | Agents + Review Queue | ✅ Complete |
| 08 | MCP Server | ✅ Complete |

### Deferred to v2
- P2P sync (Phase 06 deferred) — Requires stable iroh API
- Embeddings-based semantic search — Reserve for v2
- DAG pipeline branching — Sequential only in MVP
- Conflict auto-resolution — Manual pick A or B for MVP

---

## File Structure

```
/
├── Cargo.toml                          # Workspace manifest
├── crates/
│   ├── core/                           # Shared types, errors, config, ID generation
│   ├── vault/                          # Note CRUD, frontmatter, PARA structure
│   ├── cas/                            # Content-addressable storage, snapshots
│   ├── search/                         # tantivy FTS, SQLite graph
│   ├── agent/                          # AgentSpace engine, LLM providers, 4 built-in agents
│   ├── review/                         # Review queue, approval gate
│   └── cli/                            # Binary: CLI commands + MCP server
├── pipelines/                          # Sample TOML pipeline configs
├── docs/                               # User + developer documentation
├── tests/                              # Integration tests
└── README.md                           # Quick start guide
```

---

## Dependencies & Constraints

### Critical Dependencies
- **tokio 1.x**: Async runtime (cannot upgrade major without compatibility audit)
- **tantivy 0.22**: FTS engine (pinned, breaking changes in 0.21+)
- **rusqlite 0.32+**: SQLite bindings (bundled, no external DB)
- **serde**: Serialization ecosystem (stable, widely used)

### Environmental Constraints
- Rust 1.70+ (MSRV: 1.70)
- Linux/macOS/Windows (tested on macOS, CI runs on Linux)
- No elevated permissions required
- API key providers: OpenAI, Anthropic, Ollama accessible

### Known Limitations
- **Sequential pipelines only** — No parallel stages in MVP
- **Local storage only** — No cloud sync (deferred)
- **No conflict merge markers** — Pick A or B only
- **Single LLM per stage** — No model fallback chain

---

## Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|-----------|
| LLM parsing failure | Medium | High | JSON mode + retry logic, fallback to empty output |
| Vault corruption during snapshot | Low | Critical | Atomic writes, validation before apply |
| Circular note links crash linker | Medium | Medium | Cycle detection in graph, skip circular links |
| API key leak in logs | Low | Critical | Structured logging, never log sensitive fields |
| tantivy index corruption | Low | Medium | Reindex command available, rebuild on corruption |

---

## Success Definition

The MVP is successful when:

1. **Functionality**: All 8 phases complete with documented APIs
2. **Quality**: 29+ tests pass, 0 compiler warnings, <200 LOC per file
3. **Performance**: Search <1s, indexing <100ms/note, snapshots <2s
4. **UX**: Clear error messages, intuitive CLI, working MCP server
5. **Safety**: Agent changes reversible, manual approval available, no data loss
6. **Documentation**: README, API docs, example pipelines, troubleshooting guide

---

## Version History

| Version | Date | Status | Notes |
|---------|------|--------|-------|
| 0.1.0 | 2026-02-13 | MVP | All 8 phases complete, 29 tests passing |

