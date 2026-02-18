# Phase 30: Rustdoc & API Documentation

## Context Links

- Code standards: `docs/code-standards.md` (documentation comments section)
- Current state: basic doc comments exist but incomplete; no module-level docs, no ADRs
- docs.rs: requires `[package.metadata.docs.rs]` in Cargo.toml for feature flags

## Overview

- **Priority:** P2
- **Status:** completed
- **Effort:** 1.5h
- **Description:** Complete rustdoc for all public APIs with examples, add module-level `//!` docs for each crate, configure docs.rs metadata. ADRs deferred to v0.6 (Validation Session 1, Q5).

## Key Insights

- `cargo doc --no-deps --document-private-items` for full internal docs
- Module-level `//!` comments appear at top of crate/module pages — essential for discoverability
- docs.rs metadata: `all-features = true` ensures optional features documented
- ADRs capture "why" decisions were made — valuable for future contributors
- Examples in doc comments are compiled and tested by `cargo test` (doc tests)

## Requirements

**Functional:**
- Every public struct, enum, trait, function has `///` doc comments
- Every crate has `//!` module-level docs in `lib.rs` describing purpose and quick start
- Each crate's `Cargo.toml` has `[package.metadata.docs.rs]` section
- Doc examples compile and pass as doc tests
**Non-functional:**
- `cargo doc --no-deps --all-features` produces 0 warnings
- Doc tests pass: `cargo test --doc --workspace`
- ADRs follow standard format (Context, Decision, Consequences)

## Architecture

```
Documentation structure:
  crates/*/src/lib.rs         → //! crate-level docs
  crates/*/src/*.rs           → /// on all pub items
  crates/*/Cargo.toml         → [package.metadata.docs.rs]

ADRs: deferred to v0.6 (Validation Session 1)
```

## Related Code Files

**Modify (add/expand doc comments):**
- `crates/core/src/lib.rs` — add //! crate docs
- `crates/vault/src/lib.rs` — add //! crate docs
- `crates/cas/src/lib.rs` — add //! crate docs
- `crates/search/src/lib.rs` — add //! crate docs
- `crates/agent/src/lib.rs` — add //! crate docs
- `crates/agent/src/llm/mod.rs` — expand trait docs with examples
- `crates/review/src/lib.rs` — add //! crate docs
- `crates/sync/src/lib.rs` — add //! crate docs
- `crates/cli/src/lib.rs` — add //! crate docs
- All 8 `crates/*/Cargo.toml` — add docs.rs metadata

**No Create. No Delete.** <!-- Updated: Validation Session 1 - ADRs deferred -->

## Implementation Steps

1. Add docs.rs metadata to each crate's Cargo.toml:
   ```toml
   [package.metadata.docs.rs]
   all-features = true
   rustdoc-args = ["--cfg", "docsrs"]
   ```
2. Add `//!` module docs to each `lib.rs`:
   - core: "Shared foundation types, error handling, and configuration for agentic-note"
   - vault: "Note CRUD, PARA organization, and YAML frontmatter management"
   - cas: "Content-addressable storage with SHA-256 hashing, snapshots, and 3-way merge"
   - search: "Full-text search (tantivy), graph indexing (SQLite), optional embeddings"
   - agent: "DAG pipeline engine, LLM providers, built-in agents, plugin system"
   - review: "Human-in-the-loop review queue with configurable trust levels"
   - sync: "P2P sync via iroh QUIC, device identity, E2EE with Double Ratchet"
   - cli: "Command-line interface and MCP JSON-RPC server"
3. Audit all public items across 8 crates; add missing `///` docs:
   - Focus on: trait methods, struct fields, enum variants, function params
   - Add `# Examples` sections for key functions (Note::create, SearchEngine::search_fts, etc.)
   - Add `# Errors` sections documenting when each error variant is returned
4. Write doc test examples that compile:
   ```rust
   /// Creates a new note.
   ///
   /// # Examples
   ///
   /// ```no_run
   /// use agentic_note_vault::Note;
   /// let note = Note::create(path, "Title", para, "Body", vec![]).unwrap();
   /// ```
   ```
5. Run `cargo doc --no-deps --all-features` — fix any warnings
7. Run `cargo test --doc --workspace` — fix any failing doc tests

## Todo List

- [x] Add docs.rs metadata to all 8 Cargo.toml files
- [x] Add //! module-level docs to all lib.rs files
- [x] Audit + complete /// docs for key public items (core)
- [x] Audit + complete /// docs for key public items (vault, cas)
- [x] Audit + complete /// docs for key public items (search, agent)
- [x] Audit + complete /// docs for key public items (review, sync, cli)
- [x] Add doc examples for key functions
- [x] cargo doc runs successfully
- [x] cargo test --doc passes

## Success Criteria

- `cargo doc --no-deps --all-features` completes with 0 warnings
- `cargo test --doc --workspace` all doc tests pass
- Every `lib.rs` has `//!` module docs
- Every public item has `///` doc comment
- docs.rs metadata configured in all crates

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Doc test compilation failures | Medium | Low | Use `no_run` for tests needing runtime setup |
| Stale docs after future changes | Medium | Medium | CI checks `cargo doc` warnings |
| Missing items discovered late | Low | Low | Clippy `missing_docs` lint can be enabled |

## Security Considerations

- Doc examples must not include real API keys — use placeholder strings
- ADRs should not expose security implementation details that aid attackers

## Next Steps

- Phase 29 CI runs `cargo doc --no-deps --all-features` as quality check
- Future: enable `#![warn(missing_docs)]` in all crates
- Future: publish to docs.rs when ready for public release
