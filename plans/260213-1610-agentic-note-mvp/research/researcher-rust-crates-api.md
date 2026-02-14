# Rust Crates API Reference — Agentic-Note MVP
**Date:** 2026-02-13 | **Knowledge cutoff:** Jan 2025 (versions may be newer in prod)

---

## Quick Reference Table

| Crate | Last Known Version | Purpose | Async? | Key Gotcha |
|---|---|---|---|---|
| `iroh` | 0.29.0 | P2P blobs + docs over QUIC | tokio | API changed drastically every minor; pin exact version |
| `tantivy` | 0.22.0 | Full-text search | No (thread pool) | Writer must be dropped before reader sees new docs |
| `rmcp` | 0.1.x | MCP server (official Rust SDK) | tokio | Very new; API surface is unstable |
| `notify` | 6.1.1 | File watcher (cross-platform) | Callback-based | Use `notify-debouncer-mini` for debounce |
| `ulid` | 1.1.3 | ULID generation | No | Monotonic generator needed for ordered insert |

---

## 1. iroh

**Version:** 0.29.0 (late 2024). Released by n0 (Number Zero). API undergoes **breaking changes every minor version** — pin exact.

**Key concepts:**
- `iroh::node::Node` — local node with router, accepts connections
- `iroh::blobs` — content-addressed blob store (replaces iroh-bytes)
- `iroh::docs` — CRDT document store built on blobs (key/value with sync)
- `iroh::net` — QUIC transport + NAT traversal (uses relay servers)
- `NodeAddr` = NodeId (public key) + relay URL + direct addrs

**Cargo.toml:**
```toml
iroh = "0.29"
iroh-blobs = "0.29"
iroh-docs = "0.29"
tokio = { version = "1", features = ["full"] }
```

**Minimal node + blob transfer:**
```rust
use iroh::{node::Node, blobs::util::SetTagOption};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Start node (in-memory, ephemeral)
    let node = Node::memory().spawn().await?;
    let blobs = node.blobs();

    // Add a blob
    let outcome = blobs.add_bytes(b"hello world".to_vec()).await?;
    let hash = outcome.hash;

    // Get node address for sharing with peers
    let addr = node.node_addr().await?;
    println!("NodeId: {}", addr.node_id);

    // On peer side: connect and download
    // let peer_node = Node::memory().spawn().await?;
    // peer_node.blobs().download(hash, addr).await?;

    node.shutdown().await?;
    Ok(())
}
```

**Docs (CRDT sync) pattern:**
```rust
use iroh_docs::store::fs::Store;
// Create or open a document
let doc = node.docs().create().await?;
let author = node.docs().create_author().await?;
doc.set_bytes(author, b"key".to_vec(), b"value".to_vec()).await?;

// Share doc ticket with peer
let ticket = doc.share(iroh_docs::sync::ShareMode::Write).await?;
// Peer joins: node.docs().import(ticket).await?;
```

**Gotchas:**
- `iroh-docs` is being deprecated in favor of `iroh-willow` (Willow protocol) — docs feature may move
- Relay servers needed for NAT traversal; default relays provided by n0 but rate-limited
- Node state persists only if `Node::persistent(path)` used; `Node::memory()` is ephemeral
- Blob downloads require `ConnectOptions` with the peer's `NodeAddr`

---

## 2. tantivy

**Version:** 0.22.0. Mature, stable API. Pure Rust, no external deps.

**Cargo.toml:**
```toml
tantivy = "0.22"
```

**Schema + index + query pattern:**
```rust
use tantivy::{schema::*, Index, IndexWriter, ReloadPolicy};
use tantivy::query::QueryParser;
use tantivy::collector::TopDocs;

fn main() -> tantivy::Result<()> {
    // 1. Define schema
    let mut schema_builder = Schema::builder();
    let id_field    = schema_builder.add_text_field("id",    STRING | STORED);
    let title_field = schema_builder.add_text_field("title", TEXT | STORED);
    let body_field  = schema_builder.add_text_field("body",  TEXT);
    let schema = schema_builder.build();

    // 2. Create index (in RAM for dev, use MmapDirectory for prod)
    let index = Index::create_in_ram(schema.clone());
    // Prod: Index::create_in_dir(&path, schema)?

    // 3. Write documents
    let mut writer: IndexWriter = index.writer(50_000_000)?; // 50MB heap
    writer.add_document(doc!(
        id_field    => "note-01",
        title_field => "My First Note",
        body_field  => "This is the content of the note",
    ))?;
    writer.commit()?; // Flush to index; expensive, batch commits

    // 4. Search
    let reader = index.reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()?;
    let searcher = reader.searcher();
    let query_parser = QueryParser::for_index(&index, vec![title_field, body_field]);
    let query = query_parser.parse_query("first note")?;
    let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;

    for (_score, doc_address) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_address)?;
        println!("{:?}", doc.get_first(title_field));
    }
    Ok(())
}
```

**Gotchas:**
- `IndexWriter::commit()` is expensive — batch writes, don't commit per-document
- `reader` must be refreshed to see new commits (`reader.reload()` or `ReloadPolicy::OnCommitWithDelay`)
- `STRING` = stored/indexed as-is (no tokenization); `TEXT` = tokenized; combine with `STORED` to retrieve
- `IndexWriter` is NOT `Clone`; use `Arc<Mutex<IndexWriter>>` across threads
- Default tokenizer is English; add custom tokenizer for multilingual

---

## 3. rmcp (official Rust MCP SDK)

**Version:** 0.1.x (published 2024 by the MCP team / Anthropic ecosystem). Crate name: `rmcp`.

**Cargo.toml:**
```toml
rmcp = { version = "0.1", features = ["server", "transport-io"] }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

**Stdio MCP server pattern:**
```rust
use rmcp::{
    ServerHandler, ServiceExt,
    model::{ServerCapabilities, ServerInfo, Tool, CallToolResult, Content},
    transport::stdio,
};
use serde_json::Value;

#[derive(Clone)]
struct NoteServer;

impl ServerHandler for NoteServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "agentic-note".into(),
            version: "0.1.0".into(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }

    async fn call_tool(&self, name: &str, args: Value) -> Result<CallToolResult, rmcp::Error> {
        match name {
            "search_notes" => {
                let query = args["query"].as_str().unwrap_or("");
                // run tantivy search here
                Ok(CallToolResult {
                    content: vec![Content::text(format!("Results for: {query}"))],
                    is_error: false,
                })
            }
            _ => Err(rmcp::Error::ToolNotFound(name.into())),
        }
    }

    fn list_tools(&self) -> Vec<Tool> {
        vec![Tool {
            name: "search_notes".into(),
            description: Some("Full-text search over notes vault".into()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "query": {"type": "string"} },
                "required": ["query"]
            }),
        }]
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let transport = stdio(); // reads stdin, writes stdout
    NoteServer.serve(transport).await?.waiting().await?;
    Ok(())
}
```

**Gotchas:**
- `rmcp` 0.1.x API surface is unstable; trait method signatures may change
- Stdio transport reads JSON-RPC from stdin/stdout — do NOT write logs to stdout (use stderr or a file)
- If `rmcp` is unavailable or broken, fallback: implement JSON-RPC 2.0 manually over stdio (the MCP wire protocol is simple) using `tokio::io::BufReader<tokio::io::Stdin>`
- Alternative: `mcp-rs` crate also exists but less maintained than rmcp

---

## 4. notify

**Version:** 6.1.1. Cross-platform (inotify/FSEvents/kqueue/ReadDirectoryChangesW).

**Cargo.toml:**
```toml
notify = "6.1"
notify-debouncer-mini = "0.4"   # debouncing wrapper
```

**Recommended pattern with debouncing:**
```rust
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, DebouncedEventKind};
use std::time::Duration;
use std::path::Path;

fn watch_vault(path: &Path) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    // 500ms debounce window
    let mut debouncer = new_debouncer(Duration::from_millis(500), tx)?;
    debouncer.watcher().watch(path, notify::RecursiveMode::Recursive)?;

    for events in rx {
        match events {
            Ok(events) => {
                for event in events {
                    match event.kind {
                        DebouncedEventKind::Any => {
                            println!("Changed: {:?}", event.path);
                            // trigger re-index
                        }
                    }
                }
            }
            Err(e) => eprintln!("Watch error: {e}"),
        }
    }
    Ok(())
}
```

**Gotchas:**
- `notify-debouncer-full` (alternative) gives per-event-kind granularity but is heavier
- On macOS, FSEvents has ~1s kernel latency even with debounce at 0ms
- Watch the vault root, not individual files — recursive mode handles subdirs
- Symlinks: `notify` follows symlinks by default on some platforms but not others; test explicitly
- In async contexts, use `tokio::sync::mpsc` instead of `std::sync::mpsc` and spawn a blocking thread for the watcher

---

## 5. ulid

**Version:** 1.1.3. Crate name: `ulid`.

**Cargo.toml:**
```toml
ulid = "1.1"
```

**API patterns:**
```rust
use ulid::Ulid;

// Simple generation (current timestamp)
let id = Ulid::new();
println!("{id}");  // "01ARZ3NDEKTSV4RRFFQ69G5FAV"

// Extract timestamp
let ts = id.datetime();

// Parse from string
let parsed: Ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse()?;

// Monotonic generator (guarantees sort order within same millisecond)
use ulid::Generator;
let mut gen = Generator::new();
let id1 = gen.generate()?;
let id2 = gen.generate()?;
assert!(id1 < id2); // guaranteed lexicographic order
```

**Gotchas:**
- `Ulid::new()` uses `SystemTime` — clock skew can break monotonicity; use `Generator` for DB inserts
- `ulid` implements `Display`, `FromStr`, `Ord`, `Hash`, `Serialize/Deserialize` (feature `serde`)
- Enable serde: `ulid = { version = "1.1", features = ["serde"] }`
- ULID string is 26 chars, case-insensitive Crockford base32; store as TEXT in SQLite (not UUID column)

---

## 6. Cargo Workspace Layout

**Recommended structure for Agentic-Note:**

```
agentic-note/
├── Cargo.toml          # workspace root
├── Cargo.lock          # committed (it's an app, not a lib)
├── crates/
│   ├── core/           # shared types, traits, errors (no heavy deps)
│   ├── vault/          # file I/O, markdown parsing, YAML frontmatter
│   ├── cas/            # content-addressable store (hashing, dedup)
│   ├── sync/           # iroh P2P sync layer
│   ├── search/         # tantivy index management
│   ├── agent/          # MCP server (rmcp), LLM tool definitions
│   └── pkm/            # PKM methods: linking, backlinks, graph
├── src/                # CLI binary (thin shell, calls crates)
│   └── main.rs
└── docs/
```

**Root Cargo.toml:**
```toml
[workspace]
resolver = "2"          # REQUIRED for feature unification correctness
members = [
    "crates/core",
    "crates/vault",
    "crates/cas",
    "crates/sync",
    "crates/search",
    "crates/agent",
    "crates/pkm",
]
default-members = ["."] # CLI binary at root

[workspace.dependencies]
# Pin shared versions here; crates inherit with { workspace = true }
tokio       = { version = "1", features = ["full"] }
anyhow      = "1"
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
tracing     = "0.1"
ulid        = { version = "1.1", features = ["serde"] }
iroh        = "0.29"
tantivy     = "0.22"
notify      = "6.1"
notify-debouncer-mini = "0.4"
rmcp        = { version = "0.1", features = ["server", "transport-io"] }

[profile.release]
lto = "thin"
codegen-units = 1
strip = true
```

**Per-crate Cargo.toml pattern:**
```toml
# crates/search/Cargo.toml
[package]
name    = "agentic-note-search"
version = "0.1.0"
edition = "2021"

[dependencies]
tantivy = { workspace = true }
anyhow  = { workspace = true }
serde   = { workspace = true }
agentic-note-core = { path = "../core" }
```

**Best practices:**
- `resolver = "2"` — avoids feature unification bugs (critical with tokio feature flags)
- Keep `core` crate dependency-free or minimal (only `serde`, `thiserror`, `ulid`)
- `sync` depends on `cas`; `agent` depends on `search` + `vault` — draw dep graph before coding
- Use `[workspace.dependencies]` for all shared crates — single version bump point
- Avoid circular deps: `core ← vault ← cas ← sync`, `core ← search`, `core ← agent`

---

## Unresolved Questions

1. **iroh-docs vs iroh-willow:** iroh-docs deprecation timeline unclear; for MVP use iroh-docs, plan migration to Willow when stable.
2. **rmcp stability:** 0.1.x is pre-1.0; exact trait signatures need verification against current crates.io — verify with `cargo add rmcp` and check generated docs locally.
3. **iroh relay cost:** Default n0 relay servers are free but rate-limited; self-hosting relay needed for production.
4. **tantivy async:** tantivy has no async API; all index ops run on blocking thread pool. Use `tokio::task::spawn_blocking` to avoid blocking the async runtime.
5. **notify on Linux CI:** `inotify` watch limit (`/proc/sys/fs/inotify/max_user_watches`) can be hit in containers — needs tuning for large vaults.
