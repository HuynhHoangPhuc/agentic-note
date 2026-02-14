# Phase 2: Embeddings & Semantic Search

## Context Links
- [Research: Embeddings](/Users/phuc/Developer/agentic-note/plans/260213-2325-v02-features/research/researcher-embeddings-dag-plugins.md)
- [Search crate](/Users/phuc/Developer/agentic-note/crates/search/src/lib.rs)
- [Plan Overview](plan.md)

## Overview
- **Priority:** P2
- **Status:** completed
- **Effort:** 5h
- **Depends on:** Phase 1
- **Description:** Add ort-based embedding generation + sqlite-vec vector storage + hybrid FTS/semantic search with RRF fusion.

## Key Insights
- all-MiniLM-L6-v2 produces 384-dim float32 vectors, ~23MB model
- sqlite-vec uses `vec0` virtual tables, KNN brute-force — fine for <10k notes
- RRF fusion: `score = 1/(k+rank_fts) + 1/(k+rank_semantic)`, k=60 — no normalization needed
- First-run model download with progress bar, cached at `~/.cache/agentic-note/models/`
- ort bundles ONNX runtime by default (no user setup). Feature-gated behind `embeddings` cargo feature.
<!-- Updated: Validation Session 1 - Bundled ONNX Runtime, feature-gated behind embeddings -->

## Requirements

### Functional
- F1: `EmbeddingIndex` struct — generate embeddings via ort, store/query via sqlite-vec
- F2: `SearchEngine::search_semantic(query, limit)` — vector similarity search
- F3: `SearchEngine::search_hybrid(query, limit)` — FTS + semantic fused via RRF
- F4: CLI flag `note search --semantic <QUERY>` and `note search --hybrid <QUERY>`
- F5: First-run model download with SHA-256 verification + progress bar
- F6: `SearchEngine::index_note()` updated to also generate+store embedding
- F7: MCP tool `note/search` gains `mode` param: "fts" | "semantic" | "hybrid"

### Non-Functional
- Embedding generation <200ms per note on CPU
- Semantic search <500ms for 5k notes (brute-force KNN)
- Model download shows progress, resumable not required for MVP

## Architecture

```
crates/search/src/
├── lib.rs           # modify: add EmbeddingIndex to SearchEngine
├── fts.rs           # unchanged
├── graph.rs         # unchanged
├── reindex.rs       # modify: also reindex embeddings
├── embedding.rs     # NEW: EmbeddingIndex (ort + sqlite-vec)
├── model_download.rs # NEW: download + verify model
└── hybrid.rs        # NEW: RRF fusion logic
```

### Data Flow
```
Note body → tokenize → ort Session → 384-dim vec → sqlite-vec INSERT
Query     → tokenize → ort Session → 384-dim vec → sqlite-vec KNN → ranked results
FTS results + Semantic results → RRF fusion → hybrid results
```

## Related Code Files

| File | Action | Changes |
|------|--------|---------|
| `/Users/phuc/Developer/agentic-note/crates/search/Cargo.toml` | modify | +ort, +indicatif deps |
| `/Users/phuc/Developer/agentic-note/crates/search/src/lib.rs` | modify | Add EmbeddingIndex field, search_semantic, search_hybrid |
| `/Users/phuc/Developer/agentic-note/crates/search/src/embedding.rs` | create | EmbeddingIndex: ort session, sqlite-vec storage |
| `/Users/phuc/Developer/agentic-note/crates/search/src/model_download.rs` | create | Download model from HuggingFace CDN |
| `/Users/phuc/Developer/agentic-note/crates/search/src/hybrid.rs` | create | RRF fusion of FTS + semantic results |
| `/Users/phuc/Developer/agentic-note/crates/search/src/reindex.rs` | modify | Add embedding reindex path |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/note.rs` | modify | Add --semantic/--hybrid flags to search |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/handlers.rs` | modify | Add mode param to note/search |

## Implementation Steps

1. Add deps to `crates/search/Cargo.toml` behind feature flag:
   ```toml
   [features]
   embeddings = ["ort", "indicatif"]

   [dependencies]
   ort = { workspace = true, optional = true }
   indicatif = { workspace = true, optional = true }
   ```
   Wrap all embedding code with `#[cfg(feature = "embeddings")]`.
2. Create `crates/search/src/model_download.rs`:
   - `ensure_model(cache_dir: &Path) -> Result<PathBuf>` — check cache, download if missing
   - URL: `https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx`
   - SHA-256 verification after download
   - `indicatif::ProgressBar` for download progress
   - Default cache: `dirs::cache_dir()/agentic-note/models/`
3. Create `crates/search/src/embedding.rs`:
   - `EmbeddingIndex` struct holding `ort::Session` + `rusqlite::Connection`
   - `open(db: &Connection, model_path: &Path) -> Result<Self>`
   - Init sqlite-vec: `db.execute("SELECT load_extension('vec0')")` or use `sqlite3_vec_init`
   - Create vec0 virtual table: `CREATE VIRTUAL TABLE IF NOT EXISTS note_embeddings USING vec0(note_id TEXT PRIMARY KEY, embedding float[384])`
   - `generate_embedding(text: &str) -> Result<Vec<f32>>` — tokenize, run ort session, mean-pool
   - `index_note(note_id: &str, text: &str) -> Result<()>` — generate + INSERT/REPLACE
   - `search(query_vec: &[f32], limit: usize) -> Result<Vec<(String, f32)>>` — KNN query
   - `remove_note(note_id: &str) -> Result<()>`
4. Create `crates/search/src/hybrid.rs`:
   - `fuse_rrf(fts_results: &[SearchResult], semantic_results: &[(String, f32)], k: usize) -> Vec<SearchResult>`
   - Default k=60
   - Return merged list sorted by combined RRF score
5. Modify `crates/search/src/lib.rs`:
   - Add `pub mod embedding; pub mod model_download; pub mod hybrid;`
   - Add `embedding_index: Option<EmbeddingIndex>` field to `SearchEngine`
   - `open()` initializes EmbeddingIndex if embeddings config enabled
   - `search_semantic(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>`
   - `search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>`
   - Update `index_note()` to also call `embedding_index.index_note()` if present
6. Modify `crates/search/src/reindex.rs`:
   - Pass `Option<&EmbeddingIndex>` and reindex embeddings alongside FTS
7. Modify `crates/cli/src/commands/note.rs`:
   - Add `--semantic` and `--hybrid` flags to `NoteCmd::Search`
   - Route to appropriate search method
8. Modify `crates/cli/src/mcp/handlers.rs`:
   - Add `mode` parameter to `note/search` tool
9. Write unit tests for embedding generation, sqlite-vec storage, RRF fusion.
10. Write integration test: create notes, search semantic, verify relevance ordering.

## Todo List

- [ ] Add ort + indicatif deps to search crate
- [ ] Implement model_download.rs (download + verify)
- [ ] Implement embedding.rs (ort session + sqlite-vec)
- [ ] Implement hybrid.rs (RRF fusion)
- [ ] Update SearchEngine facade with semantic/hybrid methods
- [ ] Update index_note to generate embeddings
- [ ] Update reindex to include embeddings
- [ ] Add CLI --semantic/--hybrid flags
- [ ] Update MCP note/search with mode param
- [ ] Unit tests for embedding, fusion
- [ ] Integration test end-to-end

## Success Criteria
- `note search --semantic "rust programming"` returns relevant results
- `note search --hybrid "rust"` merges FTS + semantic rankings
- Model auto-downloads on first run with progress bar
- Embedding index <200ms per note generation
- All existing + new tests pass

## Risk Assessment
- **High:** sqlite-vec Rust bindings may require manual `unsafe` init via `libsqlite3_sys`. Mitigation: fallback to raw SQL `SELECT vec_distance_cosine()` functions.
- **Medium:** ort rc.11 API may differ from docs. Mitigation: feature-gate `embeddings` behind cargo feature flag.
- **Low:** Model download fails. Mitigation: graceful degradation — semantic search unavailable, FTS still works.

## Security Considerations
- Model downloaded over HTTPS, verified with SHA-256
- No API keys needed (local inference)
- Model cache dir has 0755 permissions

## Next Steps
- Phase 8 (Integration) tests hybrid search with DAG pipeline outputs
