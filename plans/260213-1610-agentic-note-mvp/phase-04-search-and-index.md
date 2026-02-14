# Phase 04: Search & Index

## Context
- Parent: [plan.md](plan.md)
- Deps: Phase 02 (vault), Phase 03 (CLI)
- Research: [Rust Crates API](research/researcher-rust-crates-api.md), [Embeddings](research/researcher-embeddings-llm-crypto.md)

## Overview
- **Priority:** P1 (agents need search for linking)
- **Status:** pending
- **Effort:** 10h
- **Description:** tantivy FTS index, SQLite tag/link graph, sqlite-vec for embeddings, pluggable embedding provider trait, search CLI commands.

## Key Insights
- tantivy: commit is expensive — batch writes, don't commit per-document
- tantivy has no async API — use `tokio::task::spawn_blocking`
- sqlite-vec: KNN via `MATCH` + `k` parameter; 384-dim for all-MiniLM-L6
- ort + ONNX for local embeddings; model downloaded on first use (~23MB)
- SQLite for tag/link graph — simple, no external service, queryable

## Requirements

**Functional:**
- Full-text search across title, body, tags — returns ranked results
- Semantic search via embeddings — KNN nearest neighbors
- Tag graph: list all tags, notes by tag, tag co-occurrence
- Link graph: outgoing/incoming links per note, orphan detection
- Reindex command: rebuild index from vault scan
- Incremental index: update single note on create/update/delete
- `agentic-note search <query>` — FTS, returns top N matches
- `agentic-note search --semantic <query>` — embedding similarity

**Non-functional:**
- FTS query < 50ms for 10k notes
- Embedding generation < 500ms per note (local model)
- Index size < 100MB for 10k notes

## Architecture

```
crates/search/src/
├── lib.rs              # pub mod re-exports, SearchEngine struct
├── fts.rs              # tantivy FTS index management
├── embeddings.rs       # Embedding provider trait + sqlite-vec storage
├── embedding_local.rs  # ort + all-MiniLM-L6 local provider
├── graph.rs            # SQLite tag/link graph
└── reindex.rs          # Full vault reindex logic

.agentic/
├── index.db            # SQLite (tags, links, embeddings via sqlite-vec)
└── tantivy/            # tantivy index directory
```

## Related Code Files

**Create:**
- `crates/search/Cargo.toml` (update stub)
- `crates/search/src/lib.rs`
- `crates/search/src/fts.rs`
- `crates/search/src/embeddings.rs`
- `crates/search/src/embedding_local.rs`
- `crates/search/src/graph.rs`
- `crates/search/src/reindex.rs`

**Modify:**
- `crates/cli/src/commands/mod.rs` — add Search subcommand
- `crates/cli/src/commands/search.rs` — new file
- `crates/cli/Cargo.toml` — add search dep

## Cargo.toml Dependencies
```toml
[dependencies]
agentic-note-core = { path = "../core" }
agentic-note-vault = { path = "../vault" }
tantivy = "0.22"
rusqlite = { version = "0.31", features = ["bundled"] }
sqlite-vec = "0.1"
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }

[dependencies.ort]
version = "2.0"
features = ["load-dynamic"]
optional = true

[dependencies.tokenizers]
version = "0.19"
optional = true

[features]
default = ["local-embeddings"]
local-embeddings = ["ort", "tokenizers"]
```

## Implementation Steps

1. **`fts.rs`:** tantivy FTS index
   - Schema: `id` (STRING|STORED), `title` (TEXT|STORED), `body` (TEXT), `tags` (TEXT|STORED)
   - `FtsIndex::open(index_dir: &Path) -> Result<Self>` — open or create
   - `FtsIndex::index_note(note: &Note) -> Result<()>` — add/update doc (delete old by id first)
   - `FtsIndex::remove_note(id: &NoteId) -> Result<()>`
   - `FtsIndex::search(query: &str, limit: usize) -> Result<Vec<SearchResult>>`
   - `FtsIndex::commit() -> Result<()>` — explicit commit, batch caller
   - `SearchResult { id: NoteId, title: String, score: f32, snippet: String }`

2. **`embeddings.rs`:** Embedding provider trait + storage
   - `trait EmbeddingProvider: Send + Sync { fn embed(&self, text: &str) -> Result<Vec<f32>>; fn dim(&self) -> usize; }`
   - `EmbeddingStore::open(db: &Connection) -> Result<Self>` — create sqlite-vec virtual table
   - `EmbeddingStore::upsert(id: NoteId, embedding: &[f32]) -> Result<()>`
   - `EmbeddingStore::search(query_vec: &[f32], k: usize) -> Result<Vec<(NoteId, f32)>>`
   - `EmbeddingStore::delete(id: NoteId) -> Result<()>`

3. **`embedding_local.rs`:** (behind `local-embeddings` feature)
   - `LocalEmbeddingProvider::new(model_dir: &Path) -> Result<Self>` — load ONNX model + tokenizer
   - Implement `EmbeddingProvider` trait
   - Mean pooling + L2 normalization
   <!-- Updated: Validation Session 1 - First-run download for embedding model -->
   - Model path: `~/.local/share/agentic-note/models/all-MiniLM-L6-v2/`
   - First-run download: check if model exists → if missing, download from Hugging Face Hub via reqwest
   - Show download progress bar (size ~23MB)
   - Graceful fallback: if download fails (no internet), skip embeddings, warn user, FTS still works
   - `download_model(model_dir: &Path) -> Result<()>` — fetch model.onnx + tokenizer.json

4. **`graph.rs`:** SQLite tag/link graph
   - Tables: `note_tags(note_id TEXT, tag TEXT)`, `note_links(source_id TEXT, target_id TEXT)`
   - `Graph::open(db: &Connection) -> Result<Self>` — create tables
   - `Graph::update_note(id: NoteId, tags: &[String], links: &[String]) -> Result<()>`
   - `Graph::tags() -> Vec<(String, usize)>` — all tags with count
   - `Graph::notes_by_tag(tag: &str) -> Vec<NoteId>`
   - `Graph::outgoing_links(id: NoteId) -> Vec<NoteId>`
   - `Graph::incoming_links(id: NoteId) -> Vec<NoteId>` (backlinks)
   - `Graph::orphans() -> Vec<NoteId>` — notes with no incoming links

5. **`reindex.rs`:** Full reindex
   - Walk vault, parse each note, update FTS + graph + embeddings
   - Progress bar via `tracing::info` (count/total)
   - Batch tantivy commits (every 100 docs)

6. **`lib.rs`:** `SearchEngine` facade
   - Holds FtsIndex, EmbeddingStore, Graph, optional EmbeddingProvider
   - `SearchEngine::open(vault_path: &Path) -> Result<Self>`
   - `search_fts(query, limit)`, `search_semantic(query, limit)`, `reindex()`
   - `index_note(note)`, `remove_note(id)` — incremental

7. **CLI `search` command:**
   - `agentic-note search <query> [--semantic] [--limit N] [--json]`
   - `agentic-note graph tags` — list all tags
   - `agentic-note graph backlinks <id>` — show backlinks
   - `agentic-note graph orphans` — show unlinked notes
   - `agentic-note reindex` — full reindex

## Todo List
- [ ] Implement tantivy FTS index
- [ ] Implement SQLite tag/link graph
- [ ] Implement EmbeddingProvider trait + sqlite-vec store
- [ ] Implement local embedding provider (ort)
- [ ] Implement full reindex
- [ ] Implement SearchEngine facade
- [ ] Add search/graph/reindex CLI commands
- [ ] Write tests for FTS and graph queries

## Success Criteria
- `agentic-note search "test"` returns matching notes ranked by relevance
- `agentic-note search --semantic "machine learning"` returns semantically similar notes
- `agentic-note graph backlinks <id>` shows correct incoming links
- `agentic-note reindex` rebuilds all indexes without error
- Incremental index: create note -> immediately searchable

## Risk Assessment
- **ort dynamic loading:** may fail on some systems if ONNX runtime not found — provide clear error message + fallback instructions
- **sqlite-vec v0.1.x:** API may change — pin exact version
- **tantivy writer lock:** only one writer at a time — use Arc<Mutex<IndexWriter>>

## Security Considerations
- Embeddings are generated locally — no data sent to external services (with local-embeddings feature)
- SQLite db stored in `.agentic/` — inherits vault permissions

## Next Steps
- Phase 08 (Agents) uses semantic search for zettelkasten-linker
- Phase 09 (MCP) exposes search tools
