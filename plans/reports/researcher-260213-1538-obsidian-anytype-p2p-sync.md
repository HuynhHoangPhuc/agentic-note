# Research Report: Obsidian, AnyType, Local-First P2P Sync

Date: 2026-02-13 | Slug: obsidian-anytype-p2p-sync

---

## 1. Obsidian

### Storage Architecture
- **Vault = plain folder on disk.** All notes are `.md` files; no proprietary DB.
- `.obsidian/` subfolder stores vault-level config: `app.json`, `workspace.json`, plugin data, themes, snippets.
- Plugin data stored in `.obsidian/plugins/<plugin-id>/data.json` (JSON); some plugins write additional files.
- No SQLite or binary blob — everything is human-readable and git-friendly.

### Plugin System
- Plugins are JS bundles (`main.js` + `manifest.json`). They run in the Electron renderer process with Node.js access.
- Community plugins loaded from `.obsidian/plugins/`; core plugins bundled in the app.
- API surface: `app.vault` (file I/O), `app.workspace` (UI), `app.metadataCache` (parsed frontmatter + links).
- `metadataCache` indexes all YAML frontmatter, wikilinks, and tags into an in-memory graph — the backbone of the graph view.

### LLM Compatibility
- Plain `.md` files = trivially readable by any LLM tool.
- YAML frontmatter is structured metadata LLMs can parse.
- Dataview plugin adds SQL-like queries over frontmatter — useful for structured retrieval.
- No native vector search, but community plugins (Smart Connections, Copilot) bolt on embeddings.
- **Key strength:** files are the API. No SDK needed to read/write notes.

### Sync
- Obsidian Sync: proprietary E2E encrypted cloud sync (paid). Uses their servers.
- Alternative: iCloud, Dropbox, git (no built-in conflict resolution — last-write-wins at FS level).
- No native P2P or CRDT — sync is file-level, not op-level.

### Limitations
- No structured data model — everything is untyped markdown.
- Conflict resolution is filesystem-level (no CRDT); concurrent edits on two devices can corrupt files.
- Closed-source core (Electron app). Plugin API is stable but undocumented in parts.
- Mobile apps are functional but slower; large vaults (>10k files) feel sluggish.
- Graph view is cosmetic — no semantic reasoning built in.

---

## 2. AnyType

### Core Architecture
- **Object model:** everything is an "Object" with a Type and Relations (typed fields). Not a flat markdown editor.
- Built on **Merodon protocol** (their open protocol) using a custom CRDT engine called **Anytype's Sync Tree**.
- Data stored locally in a custom embedded store (not SQLite; uses flat file + in-memory index).
- Objects serialized as protobuf-encoded blocks stored locally, synced via their P2P layer.

### P2P & Sync Stack
| Layer | Technology |
|---|---|
| Transport | **libp2p** (Go implementation) |
| Discovery | Custom relay + DHT via libp2p |
| Sync | Custom CRDT (Sync Tree) over libp2p streams |
| Encryption | Noise protocol (via libp2p) + per-space keys |
| Storage | Custom embedded KV (Go), protobuf blocks |

- **Sync Tree:** Each object is a Merkle DAG of operations (similar to Git). Devices exchange missing ops via set reconciliation. Eventual consistency guaranteed.
- **Spaces:** the unit of sync. A Space is a shared namespace with its own key pair. Members hold a copy.
- **Backup node:** AnyType runs optional backup nodes (their servers) — acts as a always-online peer, not a central authority. Can self-host.

### Strengths
- True local-first: full offline, no degraded mode.
- Device-to-device sync without server if on same network (mDNS discovery via libp2p).
- Structured data (typed objects, relations) — better than flat markdown for querying.
- Open-source clients (TypeScript + Go); protocol is open.
- Self-hostable backup node.

### Limitations
- **Not markdown-native.** Content is stored as protobuf blocks, not `.md` files. Export to markdown is lossy.
- CRDT is custom/proprietary — not interoperable with Automerge or Yjs ecosystems.
- Complex codebase (Go middleware + TS frontend + protobuf schema) — high contribution barrier.
- Mobile sync reliability reported as inconsistent in community.
- Structured object model = less flexible for freeform writing than Obsidian.
- Still maturing: API and protocol have breaking changes.

---

## 3. Local-First P2P Sync Technologies

### CRDT Libraries

#### Automerge (v2 / automerge-repo)
- **Type:** Op-based CRDT; rich text via Peritext algorithm.
- **Storage:** operations log + compressed snapshots. `automerge-repo` adds network + storage adapters.
- **Network:** `automerge-repo` ships adapters for WebSocket, BroadcastChannel, MessageChannel.
- **Strengths:** mature, battle-tested, WASM build available, good TypeScript support, rich text support.
- **Weaknesses:** binary format (not plain text); large documents have memory overhead; no built-in discovery.

#### Yjs
- **Type:** State-based CRDT; uses Y.Doc with types (Y.Text, Y.Map, Y.Array).
- **Network:** y-webrtc (P2P via WebRTC + signaling), y-websocket, y-libp2p.
- **Strengths:** fastest CRDT in benchmarks, smallest bundle, best editor integrations (ProseMirror, CodeMirror, Quill, Lexical, TipTap). Huge ecosystem.
- **Weaknesses:** binary format; no built-in persistence (needs y-indexeddb, y-leveldb); awareness protocol separate.
- **Best for:** real-time collaborative editing in browser/Electron.

### P2P Transport

#### libp2p
- Modular P2P networking stack (originally from IPFS/Protocol Labs). Go, JS, Rust, Python implementations.
- Handles: transport (TCP, WebSockets, QUIC, WebRTC), multiplexing, encryption (Noise/TLS), peer discovery (mDNS, DHT, bootstrap).
- **Strengths:** production-proven (IPFS, Filecoin, Ethereum), handles NAT traversal, encryption built-in.
- **Weaknesses:** large dependency surface; JS implementation lags Go; complex to configure correctly.

#### Hypercore / Hyperswarm
- **Hypercore:** append-only log with cryptographic integrity (Merkle tree). Each peer owns one log.
- **Hyperswarm:** DHT-based peer discovery + hole-punching for NAT traversal (UDP).
- **Hyperbee:** B-tree on Hypercore (key-value store).
- **Strengths:** minimal, elegant, truly serverless, excellent NAT traversal, small codebase.
- **Weaknesses:** append-only model requires multi-writer extensions (Autobase) for collaboration; JS/Node.js ecosystem only (Rust port emerging); less enterprise adoption than libp2p.
- **Autobase:** multi-writer layer on top of Hypercore — each writer has own log, merged via causal ordering. Still experimental.

#### WebRTC (Direct)
- Browser-native P2P; requires signaling server for initial handshake only.
- **Strengths:** works in browser without native app, NAT traversal via ICE/STUN/TURN.
- **Weaknesses:** needs STUN/TURN infrastructure; not suitable for non-browser peers without workarounds.

### Trade-off Matrix

| Technology | Conflict Resolution | Browser | No Server | Maturity | Complexity |
|---|---|---|---|---|---|
| Automerge | CRDT (op-based) | Yes (WASM) | Needs discovery | High | Medium |
| Yjs | CRDT (state-based) | Yes | Needs discovery | High | Low |
| libp2p | None (transport only) | Partial (JS) | Yes | High | High |
| Hyperswarm | None (transport only) | No | Yes | Medium | Low |
| WebRTC | None (transport only) | Yes | No (STUN) | High | Medium |

### Recommended Combination for Markdown P2P Sync
- **Yjs** (CRDT for per-document conflict resolution) + **Hyperswarm** (discovery + transport) = minimal, no-server, works on desktop/Node.
- **Yjs** + **libp2p** = more portable, browser-compatible, but heavier.
- **Automerge-repo** with custom network adapter over Hyperswarm = solid for structured doc sync if rich text needed.

---

## Summary for agentic-note

| Concern | Obsidian | AnyType | Custom (Yjs+Hyperswarm) |
|---|---|---|---|
| Storage format | Plain `.md` (ideal) | Protobuf blocks | Plain `.md` |
| Conflict resolution | None (FS) | Custom CRDT | Yjs CRDT |
| P2P sync | None native | libp2p custom | Hyperswarm |
| LLM compatibility | Excellent | Poor (binary) | Excellent |
| Extensibility | Plugin JS | Limited API | Full control |
| Self-host | N/A | Yes | Yes |

**Key takeaway:** For an LLM-compatible agentic note-taking tool that syncs P2P, the best foundation is plain `.md` files (Obsidian-style vault) + Yjs for CRDT merge + Hyperswarm for P2P discovery. AnyType's architecture is inspiring for structured objects but its binary format breaks LLM compatibility.

---

## Unresolved Questions

1. Does agentic-note need rich-text CRDT (Yjs Y.Text) or is last-write-wins per file acceptable for MVP?
2. Mobile support required? Hyperswarm is Node.js-only; mobile would need libp2p or WebRTC instead.
3. Self-hosted relay/backup node in scope, or pure P2P only?
4. Is structured data (typed objects like AnyType) needed, or is YAML frontmatter sufficient for metadata?
5. Automerge vs Yjs: Automerge has better TypeScript types and a cleaner API for non-realtime sync; Yjs wins on performance and editor integration — which matters more?
