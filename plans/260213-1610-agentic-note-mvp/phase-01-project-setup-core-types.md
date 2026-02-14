# Phase 01: Project Setup & Core Types

## Context
- Parent: [plan.md](plan.md)
- Deps: none (foundation phase)
- Research: [Rust Crates API](research/researcher-rust-crates-api.md)

## Overview
- **Priority:** P1 (blocks all other phases)
- **Status:** pending
- **Effort:** 4h
- **Description:** Initialize Cargo workspace, define shared error types, config schema, vault path resolution, ULID-based ID generation.

## Key Insights
- Use `resolver = "2"` in workspace — required for tokio feature flag unification
- `[workspace.dependencies]` for all shared crates — single version bump point
- `core` crate must be minimal (no heavy deps) — every other crate depends on it
- ULID monotonic generator needed for ordered note IDs

## Requirements

**Functional:**
- Cargo workspace builds with `cargo build` from root
- Shared `Error` type usable across all crates via `thiserror`
- Config struct deserializable from `.agentic/config.toml`
- Vault path resolved from: explicit arg > env var `AGENTIC_NOTE_VAULT` > current dir
- ULID generation with monotonic ordering

**Non-functional:**
- Compile time: `core` crate < 5s clean build
- Zero unsafe code in core

## Architecture

```
crates/core/src/
├── lib.rs          # pub mod re-exports
├── error.rs        # thiserror Error enum
├── config.rs       # AppConfig, VaultConfig structs (serde + TOML)
├── types.rs        # NoteId (ULID wrapper), ParaCategory enum, NoteStatus enum
└── id.rs           # ULID monotonic generator
```

## Related Code Files

**Create:**
- `Cargo.toml` (workspace root)
- `crates/core/Cargo.toml`
- `crates/core/src/lib.rs`
- `crates/core/src/error.rs`
- `crates/core/src/config.rs`
- `crates/core/src/types.rs`
- `crates/core/src/id.rs`
- `crates/vault/Cargo.toml` (stub)
- `crates/cas/Cargo.toml` (stub)
- `crates/sync/Cargo.toml` (stub)
- `crates/search/Cargo.toml` (stub)
- `crates/agent/Cargo.toml` (stub)
- `crates/review/Cargo.toml` (stub)
- `crates/cli/Cargo.toml` (stub)

## Implementation Steps

1. **Init workspace root `Cargo.toml`:**
   - `[workspace]` with `resolver = "2"`, members list all 8 crates
   - `[workspace.dependencies]` pin: tokio 1, anyhow 1, serde 1 (derive), serde_json 1, thiserror 2, tracing 0.1, ulid 1.1 (serde), toml 0.8, chrono 0.4 (serde)
   - `[profile.release]` with lto=thin, codegen-units=1, strip=true

2. **Create `crates/core/Cargo.toml`:**
   - Package name: `agentic-note-core`, edition 2021
   - Deps: thiserror, serde, serde_json, ulid, toml, chrono (all `workspace = true`)

3. **`error.rs`:** Define `AgenticError` enum with variants:
   - `Io(#[from] std::io::Error)`, `Config(String)`, `Parse(String)`, `NotFound(String)`, `Conflict(String)`, `Agent(String)`, `Sync(String)`, `Search(String)`
   - `pub type Result<T> = std::result::Result<T, AgenticError>;`

4. **`config.rs`:** Define structs:
   - `AppConfig { vault: VaultConfig, llm: LlmConfig, agent: AgentConfig }`
   - `VaultConfig { path: PathBuf, para_folders: Vec<String> }`
   - `LlmConfig { default_provider: String, providers: HashMap<String, ProviderConfig> }`
   - `AgentConfig { default_trust: TrustLevel, max_concurrent_pipelines: usize }`
   - `TrustLevel` enum: Manual, Review, Auto
   - `load(path: Option<PathBuf>) -> Result<AppConfig>` — reads `.agentic/config.toml`
   - `vault_path()` resolution: explicit > env > cwd

5. **`types.rs`:** Define:
   - `NoteId(Ulid)` wrapper with Display, FromStr, Serialize, Deserialize
   - `ParaCategory` enum: Projects, Areas, Resources, Archives, Inbox, Zettelkasten
   - `NoteStatus` enum: Seed, Budding, Evergreen
   - `FrontMatter` struct: id, title, created, modified, tags, para, links, status

6. **`id.rs`:** ULID monotonic generator:
   - `IdGenerator` wrapping `ulid::Generator` with `Mutex` for thread safety
   - `pub fn next_id() -> NoteId` — global lazy-init generator

7. **Create stub Cargo.toml for remaining crates** (vault, cas, sync, search, agent, review, cli) — each with package name `agentic-note-{name}`, edition 2021, dep on `agentic-note-core = { path = "../core" }`

8. **Verify:** `cargo check` passes from workspace root

## Todo List
- [ ] Create workspace root Cargo.toml
- [ ] Create core crate with error, config, types, id modules
- [ ] Create stub Cargo.toml for all 7 other crates
- [ ] Verify `cargo check` passes
- [ ] Verify config TOML deserialization with test

## Success Criteria
- `cargo check` passes for entire workspace
- `AppConfig::load()` deserializes sample TOML
- `NoteId::new()` generates valid monotonic ULIDs
- All crate stubs compile

## Risk Assessment
- **Low risk** — straightforward Rust boilerplate
- Workspace resolver version mismatch could cause confusing feature flag bugs — ensure `resolver = "2"`

## Security Considerations
- Config file may contain LLM API keys — remind users to set 0600 permissions
- Do not log API keys in tracing output

## Next Steps
- Phase 02 (Vault & Notes) depends on core types
- All other phases depend on this
