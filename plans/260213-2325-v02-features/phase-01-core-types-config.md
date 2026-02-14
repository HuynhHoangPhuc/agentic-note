# Phase 1: Core Types & Config Extensions

## Context Links
- [Codebase Summary](/Users/phuc/Developer/agentic-note/docs/codebase-summary.md)
- [System Architecture](/Users/phuc/Developer/agentic-note/docs/system-architecture.md)
- [Plan Overview](plan.md)

## Overview
- **Priority:** P1 (all other phases depend on this)
- **Status:** completed
- **Effort:** 2h
- **Description:** Add new error variants, config sections, and workspace deps needed by all 6 features.

## Key Insights
- Existing `AgenticError` already has `Sync` variant (added speculatively in MVP). Need `Embedding`, `Plugin`, `Pipeline` variants.
- `AppConfig` needs `sync`, `embeddings`, `plugins` sections — all `#[serde(default)]` for backward compat.
- New workspace deps: `petgraph`, `ort`, `indicatif`, `iroh`, `iroh-blobs`, `ed25519-dalek`, `sqlite-vec`.

## Requirements

### Functional
- F1: New error variants: `Embedding(String)`, `Plugin(String)`, `Pipeline(String)`
- F2: `SyncConfig` struct (default_conflict_policy, conflict_overrides per PARA, device_name)
- F3: `EmbeddingsConfig` struct (enabled, model_path, cache_dir)
- F4: `PluginsConfig` struct (enabled, plugins_dir, default_timeout_secs)
- F5: `ConflictPolicy` enum: NewestWins, LongestWins, MergeBoth, Manual
- F6: `ErrorPolicy` enum: Skip, Retry, Abort, Fallback (for Phase 4)
- F7: Workspace dependency additions in root `Cargo.toml`
- F8: New `crates/sync` crate scaffold (empty lib.rs, Cargo.toml)

### Non-Functional
- All new config sections default to sensible values (features disabled by default)
- Zero breaking changes to existing config.toml files

## Architecture

```
crates/core/src/
├── error.rs        # +3 variants
├── config.rs       # +SyncConfig, EmbeddingsConfig, PluginsConfig, ConflictPolicy
├── types.rs        # +ConflictPolicy, ErrorPolicy enums
└── lib.rs          # re-export new types

Cargo.toml          # workspace dep additions
crates/sync/        # new crate scaffold
```

## Related Code Files

| File | Action | Changes |
|------|--------|---------|
| `/Users/phuc/Developer/agentic-note/Cargo.toml` | modify | Add workspace deps + sync crate member |
| `/Users/phuc/Developer/agentic-note/crates/core/src/error.rs` | modify | +3 error variants |
| `/Users/phuc/Developer/agentic-note/crates/core/src/config.rs` | modify | +SyncConfig, EmbeddingsConfig, PluginsConfig structs |
| `/Users/phuc/Developer/agentic-note/crates/core/src/types.rs` | modify | +ConflictPolicy, ErrorPolicy enums |
| `/Users/phuc/Developer/agentic-note/crates/core/src/lib.rs` | modify | re-export new types |
| `/Users/phuc/Developer/agentic-note/crates/sync/Cargo.toml` | create | New crate manifest |
| `/Users/phuc/Developer/agentic-note/crates/sync/src/lib.rs` | create | Empty scaffold with module stubs |

## Implementation Steps

1. Add workspace deps to root `Cargo.toml` `[workspace.dependencies]`:
   ```toml
   petgraph = "0.6"
   ort = "2.0.0-rc.11"  # bundled ONNX Runtime (no load-dynamic)
   indicatif = "0.17"
   iroh = "0.30"
   iroh-blobs = "0.30"
   ed25519-dalek = { version = "2", features = ["serde"] }
   agentic-note-sync = { path = "crates/sync" }
   ```
2. Add `"crates/sync"` to workspace `members` array.
3. Add error variants to `crates/core/src/error.rs`:
   ```rust
   #[error("Embedding error: {0}")]
   Embedding(String),
   #[error("Plugin error: {0}")]
   Plugin(String),
   #[error("Pipeline error: {0}")]
   Pipeline(String),
   ```
4. Add `ConflictPolicy` enum to `crates/core/src/types.rs`:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "kebab-case")]
   pub enum ConflictPolicy {
       NewestWins,
       LongestWins,
       MergeBoth,
       Manual,
   }
   impl Default for ConflictPolicy { fn default() -> Self { Self::Manual } }
   ```
5. Add `ErrorPolicy` enum to `crates/core/src/types.rs`:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "kebab-case")]
   pub enum ErrorPolicy {
       Skip,
       Retry,
       Abort,
       Fallback,
   }
   impl Default for ErrorPolicy { fn default() -> Self { Self::Skip } }
   ```
6. Add config structs to `crates/core/src/config.rs`:
   - `SyncConfig { default_conflict_policy: ConflictPolicy, conflict_overrides: HashMap<String, ConflictPolicy>, device_name: Option<String> }`
   - `EmbeddingsConfig { enabled: bool, model_name: String, cache_dir: Option<PathBuf> }`
   - `PluginsConfig { enabled: bool, plugins_dir: PathBuf, default_timeout_secs: u64 }`
   - Add fields to `AppConfig`: `sync: SyncConfig`, `embeddings: EmbeddingsConfig`, `plugins: PluginsConfig` (all `#[serde(default)]`)
7. Re-export new types from `crates/core/src/lib.rs`.
8. Create `crates/sync/Cargo.toml` with deps on `agentic-note-core`, `tokio`, `serde`, `tracing`.
9. Create `crates/sync/src/lib.rs` with empty module declarations.
10. Add `[features]` to root `Cargo.toml`:
    ```toml
    [workspace.features]
    embeddings = []
    sync = []
    ```
    And in `crates/search/Cargo.toml`:
    ```toml
    [features]
    embeddings = ["ort", "indicatif"]
    ```
    And in `crates/cli/Cargo.toml`:
    ```toml
    [features]
    embeddings = ["agentic-note-search/embeddings"]
    sync = ["agentic-note-sync"]
    ```
11. Run `cargo check` to verify compilation.
<!-- Updated: Validation Session 1 - Added cargo feature flags for embeddings and sync -->

## Todo List

- [ ] Add workspace dependencies to root Cargo.toml
- [ ] Add sync crate to workspace members
- [ ] Add 3 new error variants
- [ ] Add ConflictPolicy enum
- [ ] Add ErrorPolicy enum
- [ ] Add SyncConfig, EmbeddingsConfig, PluginsConfig structs
- [ ] Update AppConfig with new sections
- [ ] Re-export new types from core lib.rs
- [ ] Scaffold sync crate (Cargo.toml + lib.rs)
- [ ] cargo check passes
- [ ] Existing 27 tests still pass

## Success Criteria
- `cargo check` compiles with 0 errors, 0 warnings
- `cargo test` passes all 27 existing tests
- Existing config.toml files parse without changes (serde defaults)
- New types visible in `cargo doc`

## Risk Assessment
- **Low:** Adding serde(default) fields is backward-compatible
- **Medium:** iroh 0.30 may not exist yet — verify crates.io before pinning, use latest available

## Security Considerations
- No new secrets or permissions
- ConflictPolicy/ErrorPolicy are enums — no injection risk

## Next Steps
- Phase 2 (Embeddings) can start after this phase
- Phase 3 (DAG) can start after this phase
- Phase 5 (Conflict) can start after this phase
