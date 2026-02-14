# Research Report: Embeddings, DAG Pipelines, Plugins & Error Recovery

Date: 2026-02-13 | Context: agentic-note v0.2.0 planning

---

## 1. Embeddings-based Semantic Search

### ort (ONNX Runtime for Rust)
- **Latest:** v2.0.0-rc.11 (approaching stable 2.0)
- Fast ML inference with hardware acceleration (CUDA, TensorRT, OpenVINO, etc.)
- Wraps Microsoft ONNX Runtime
- Used by: Text Embeddings Inference (TEI), FastEmbed-rs
- API: Load `.onnx` model → create session → run inference

**all-MiniLM-L6-v2:**
- ~23MB ONNX model, 384-dim embeddings
- Good quality/speed tradeoff for semantic similarity
- First-run download from HuggingFace

### sqlite-vec
- Pure C SQLite extension, no dependencies
- KNN brute-force vector search (ANN planned)
- Supports float, int8, binary vectors via `vec0` virtual tables
- Rust integration: `sqlite3_vec_init` + rusqlite `bundled` feature
- Distance metrics: cosine, L2, inner product

**Hybrid Search (FTS + Semantic):**
- Reciprocal Rank Fusion (RRF): `score = 1/(k + rank_fts) + 1/(k + rank_semantic)`, k=60
- Linear combination: `score = α * fts_score + (1-α) * semantic_score`
- RRF recommended — rank-based, no score normalization needed

### Architecture:
```
SearchEngine (existing facade)
├── FtsIndex (tantivy) — keyword search
├── Graph (SQLite) — backlinks/tags
└── [NEW] EmbeddingIndex
    ├── ort Session — generate embeddings
    └── sqlite-vec — store/query vectors
```

**First-run download pattern:**
1. Check `~/.cache/agentic-note/models/all-MiniLM-L6-v2.onnx`
2. If missing → download from HuggingFace CDN
3. Progress bar via `indicatif` crate
4. Verify SHA-256 checksum

Sources: [ort GitHub](https://github.com/pykeio/ort), [sqlite-vec](https://github.com/asg017/sqlite-vec), [sqlite-vec Rust docs](https://alexgarcia.xyz/sqlite-vec/rust.html)

---

## 2. DAG Pipeline Execution

### petgraph
- Mature Rust graph library, actively maintained
- `DiGraph` for directed graphs, `toposort()` for topological ordering
- `rayon` feature for parallel iterators
- O(|V| + |E|) topological sort

### DAG Execution Strategy:
1. Parse TOML pipeline into `petgraph::DiGraph`
2. Topological sort → execution layers
3. Stages in same layer (no dependencies) → parallel via `tokio::join_all`
4. Stages with dependencies → sequential within their chain

### TOML v2 Schema (DAG):
```toml
schema_version = 2

[[stages]]
name = "classify"
agent = "para-classifier"
depends_on = []  # NEW: dependency list

[[stages]]
name = "extract-links"
agent = "zettelkasten-linker"
depends_on = []  # parallel with classify

[[stages]]
name = "summarize"
agent = "distiller"
depends_on = ["classify", "extract-links"]  # waits for both

[[stages]]
name = "write-synthesis"
agent = "vault-writer"
depends_on = ["summarize"]
condition = "classify.output.para == 'projects'"  # NEW: conditional
```

### Migration v1 → v2:
- v1 (sequential): stages ordered by array position, no `depends_on`
- v2 (DAG): stages have explicit `depends_on`, optional `condition`
- v1 auto-upgrade: add `depends_on = ["previous_stage"]` to each stage
- `schema_version` field distinguishes formats

Sources: [petgraph](https://github.com/petgraph/petgraph), [petgraph docs](https://docs.rs/petgraph/latest/petgraph/)

---

## 3. Custom Agent Plugin System

### Approach Comparison:

| Approach | Safety | Performance | Portability | Complexity |
|----------|--------|-------------|-------------|------------|
| WASM (wasmtime) | Sandboxed | ~3x native | Excellent | High |
| Dynamic lib | Unsafe FFI | Native | Platform-specific | Medium |
| Subprocess | Process isolation | IPC overhead | Excellent | Low |

### Recommendation: Subprocess (KISS for MVP)
- Simplest to implement, natural sandboxing via OS process isolation
- Plugin = executable that reads JSON stdin, writes JSON stdout
- Same pattern as MCP tools — consistent with existing architecture
- Users can write plugins in any language
- No WASM compilation complexity

### Plugin Discovery:
```
.agentic/plugins/
├── my-agent/
│   ├── plugin.toml       # metadata: name, version, input/output schema
│   └── run.sh            # or run.exe, or any executable
```

### plugin.toml:
```toml
name = "custom-tagger"
version = "0.1.0"
description = "Auto-tag notes based on content"
input_schema = "note"      # StageContext type
output_schema = "metadata"  # StageOutput type
executable = "run.sh"
```

### Execution:
1. Discover plugins in `.agentic/plugins/`
2. Load `plugin.toml`, validate schema
3. On pipeline stage referencing plugin → spawn subprocess
4. Pipe `StageContext` as JSON to stdin
5. Read `StageOutput` as JSON from stdout
6. Timeout after configurable duration (default 30s)

Sources: [NullDeref Rust plugins](https://nullderef.com/blog/plugin-tech/), [wasmtime docs](https://docs.rs/wasmtime)

---

## 4. Pipeline Error Recovery

### Per-stage Error Policies:
```toml
[[stages]]
name = "classify"
agent = "para-classifier"
on_error = "retry"       # skip | retry | abort | fallback
retry_max = 3
retry_backoff_ms = 1000  # exponential: 1s, 2s, 4s
fallback_agent = "rule-based-classifier"  # optional
```

### Policies:
1. **skip** — Log warning, continue pipeline (current global default)
2. **retry** — Exponential backoff, configurable max attempts
3. **abort** — Stop entire pipeline, report error
4. **fallback** — Try alternative agent, then skip if fallback also fails

### Implementation:
- Retry with `tokio::time::sleep` for backoff
- Error accumulator: `Vec<StageError>` in `PipelineResult`
- Fallback: resolve agent by `fallback_agent` name, same StageContext
- Global default still configurable: `[agent] default_on_error = "skip"`

---

## Unresolved Questions

1. sqlite-vec ANN support timeline? Current brute-force OK for <10k notes but may need ANN for larger vaults.
2. Plugin sandboxing: subprocess sufficient or need resource limits (cgroups/rlimit)?
3. DAG cycle detection: fail at load time or allow self-referencing stages?
4. Embedding model alternatives to all-MiniLM-L6-v2? (e.g., BGE-small for better multilingual)
