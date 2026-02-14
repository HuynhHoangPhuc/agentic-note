# Research Report: iroh P2P, CRDT Sync & Conflict Resolution

Date: 2026-02-13 | Context: agentic-note v0.2.0 planning

---

## 1. iroh Current State (2026)

**Latest:** iroh continues active development by n0-computer. Still pre-1.0 with frequent breaking changes in CHANGELOG. API evolving — not yet stable.

**Key changes since MVP deferral (0.29):**
- iroh split into modular crates: `iroh` (core networking), `iroh-blobs` (content-addressed blob transfer), `iroh-gossip` (pub/sub)
- `iroh-docs` deprecated — migrating to Willow protocol. DO NOT use iroh-docs.
- Focus on stability improvements, especially gossip layer
- Mobile roadmap: refactoring iroh-embed into configurable library with C bindings, Swift/Kotlin wrappers
- QUIC-based transport with relay-assisted NAT traversal

**Risk Assessment:**
- API still breaks between minor versions — thin adapter layer ESSENTIAL
- No 1.0 date announced — plan for breakage
- iroh-blobs aligns with our CAS (content-addressed) — good fit
- Relay servers from n0 are free but rate-limited

**Recommendation:** Use iroh with pinned exact version + thin adapter trait. Accept ~2h maintenance per iroh update. iroh-blobs for content transfer, custom sync protocol on top (not iroh-docs).

Sources: [iroh GitHub](https://github.com/n0-computer/iroh), [crates.io/iroh](https://crates.io/crates/iroh), [iroh.computer](https://www.iroh.computer/)

---

## 2. CRDT Libraries for Rust

### automerge-rs
- Op-based CRDT, Peritext rich text algorithm
- Binary format (not plain text) — would need serialize/deserialize layer
- WASM build available, good TS support
- Mature, battle-tested

### yrs (Yjs Rust port)
- State-based CRDT, Y.Doc with typed structures
- Fastest CRDT in benchmarks, smallest footprint
- Binary format — same limitation as automerge
- Best editor integration ecosystem

### diamond-types
- Experimental, high-performance text CRDT by Joseph Gentle
- Focused on text editing specifically
- Less mature than automerge/yrs

### Recommendation for agentic-note
Since notes are `.md` files with YAML frontmatter, full CRDT is overkill for MVP sync. Better approach:
1. **File-level CAS sync** (already have CAS with SHA-256)
2. **Conflict detection** via CAS three_way_merge (already implemented)
3. **Auto-resolution policies** for simple cases (newest-wins, etc.)
4. Defer character-level CRDT to v3 if real-time collab needed

This avoids binary format complexity and keeps `.md` files human-readable.

---

## 3. Conflict Auto-Resolution Policies

### How others handle it:
| App | Strategy |
|-----|----------|
| Obsidian Sync | Last-write-wins (file-level), no CRDT |
| Logseq | Git-based merge, creates conflict files |
| Joplin | Metadata timestamp comparison, newest-wins |
| AnyType | Custom CRDT (Sync Tree), eventual consistency |

### Proposed policies for agentic-note:
1. **newest-wins** — Compare `modified` timestamp in frontmatter, keep newer
2. **longest-wins** — Keep version with more content (body length)
3. **merge-both** — Concatenate both bodies under conflict headers
4. **manual** — Existing pick-A-or-B (current MVP behavior)

### Configuration:
```toml
[sync]
default_conflict_policy = "newest-wins"  # newest-wins | longest-wins | merge-both | manual

# Per-PARA overrides
[sync.conflict_overrides]
zettelkasten = "manual"  # Always review zettelkasten conflicts
inbox = "newest-wins"    # Inbox notes auto-resolve
```

### Integration with CAS:
- `three_way_merge()` already detects conflicts → add policy parameter
- If policy resolves automatically → apply + create CAS snapshot
- If policy = manual → existing review queue path

---

## Unresolved Questions

1. Should iroh relay be self-hostable in v0.2 or defer to v0.3?
2. CRDT for character-level merge — defer to v3 or never? (file-level sync may be sufficient)
3. Mobile sync priority? iroh mobile bindings still WIP.
