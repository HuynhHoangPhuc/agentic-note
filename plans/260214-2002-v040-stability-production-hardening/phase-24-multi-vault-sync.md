# Phase 24: Multi-Vault Sync

## Context Links
- [plan.md](plan.md)
- [crates/sync/src/lib.rs](/Users/phuc/Developer/agentic-note/crates/sync/src/lib.rs) — SyncEngine
- [crates/cli/src/commands/sync_cmd.rs](/Users/phuc/Developer/agentic-note/crates/cli/src/commands/sync_cmd.rs) — CLI sync
- [crates/core/src/config.rs](/Users/phuc/Developer/agentic-note/crates/core/src/config.rs) — AppConfig

## Overview
- **Priority:** P2
- **Status:** Complete
- **Implementation Status:** complete
- **Review Status:** complete
- **Effort:** 2h
- **Description:** Support multiple vaults in a single CLI session. Vault registry manifest lists vault paths + per-vault configs. Sequential sync across vaults (simpler, avoids resource contention).

## Key Insights
- Current SyncEngine is bound to single vault_path
- Multi-vault = iterate over vault registry, create SyncEngine per vault
- TOML manifest at `~/.agentic-note/vaults.toml` (global, not per-vault)
- Sequential sync preferred: simpler, no concurrent file/DB contention
- Each vault retains its own `.agentic/` directory (identity, config, index)

## Requirements

### Functional
- `VaultRegistry` struct: load/save vault manifest from `~/.agentic-note/vaults.toml`
- Manifest format: list of vault entries with path, name, sync_enabled, default_peers
- CLI: `vault register <path> [--name <name>]`, `vault unregister <path>`, `vault list`
- CLI: `sync all` syncs all registered vaults sequentially
- Per-vault sync results aggregated and displayed
- Each vault maintains independent identity + device registry

### Non-Functional
- Sequential sync (no parallel vault sync) for simplicity
- Support up to 50 registered vaults
- <100ms overhead for vault registry operations

## Architecture

```
~/.agentic-note/vaults.toml
    |
    +-- VaultEntry { path: "/path/a", name: "work", sync_enabled: true }
    +-- VaultEntry { path: "/path/b", name: "personal", sync_enabled: true }
    |
    v
CLI: `sync all`
    |
    +-- For vault A: SyncEngine::new_with_iroh(path_a) -> sync all peers -> result_a
    +-- For vault B: SyncEngine::new_with_iroh(path_b) -> sync all peers -> result_b
    |
    v
MultiVaultSyncResult { results: [(vault_name, SyncResult)] }
```

## Related Code Files

### Modify
- `crates/core/src/config.rs` — add VaultEntry type
- `crates/core/src/error.rs` — add MultiVault(String) variant
- `crates/sync/src/lib.rs` — add multi_vault_sync function
- `crates/cli/src/commands/sync_cmd.rs` — add `sync all` subcommand
- `crates/cli/src/main.rs` — add vault register/unregister/list commands

### Create
- `crates/sync/src/vault_registry.rs` — VaultRegistry manifest load/save
- `crates/cli/src/commands/vault_registry_cmd.rs` — CLI commands for vault management

## Implementation Steps

1. Add `MultiVault(String)` error variant to `error.rs`
2. Create `VaultEntry` in config.rs:
   ```rust
   pub struct VaultEntry {
       pub path: PathBuf,
       pub name: String,
       pub sync_enabled: bool,
       pub default_peers: Vec<String>,
   }
   ```
3. Create `vault_registry.rs`:
   ```rust
   pub struct VaultRegistry {
       pub vaults: Vec<VaultEntry>,
       manifest_path: PathBuf,
   }
   impl VaultRegistry {
       pub fn load() -> Result<Self>; // from ~/.agentic-note/vaults.toml
       pub fn save(&self) -> Result<()>;
       pub fn register(&mut self, path: PathBuf, name: String) -> Result<()>;
       pub fn unregister(&mut self, path: &Path) -> Result<()>;
       pub fn list(&self) -> &[VaultEntry];
       pub fn sync_enabled(&self) -> Vec<&VaultEntry>;
   }
   ```
4. Add `sync_all_vaults` function in sync lib.rs:
   ```rust
   pub async fn sync_all_vaults(registry: &VaultRegistry) -> Result<MultiVaultSyncResult> {
       let mut results = Vec::new();
       for vault in registry.sync_enabled() {
           let engine = SyncEngine::new_with_iroh(&vault.path).await?;
           // sync each peer sequentially
           let result = /* batch sync all peers */;
           results.push((vault.name.clone(), result));
       }
       Ok(MultiVaultSyncResult { results })
   }
   ```
5. Create `vault_registry_cmd.rs` with CLI commands:
   - `vault register <path> [--name <name>]`
   - `vault unregister <path>`
   - `vault list` (show all registered vaults with sync status)
6. Extend `sync_cmd.rs`:
   - `sync all` uses VaultRegistry to sync all vaults
   - Display per-vault results
7. Add tests: register/unregister, manifest persistence, sync_all_vaults

## Todo List
- [x]Add MultiVault error variant
- [x]Add VaultEntry to config types
- [x]Create VaultRegistry with TOML persistence
- [x]Create vault register/unregister/list CLI commands
- [x]Add sync_all_vaults function
- [x]Extend sync CLI with `sync all`
- [x]Add unit tests
- [x]Add integration test (register + list)

## Success Criteria
- `vault register ~/work-notes --name work` persists to manifest
- `vault list` shows all registered vaults
- `sync all` syncs each vault sequentially, reports per-vault results
- Unregister removes vault from manifest
- Existing single-vault sync unchanged

## Risk Assessment
- **Stale manifests**: Vault path may no longer exist. Mitigate: validate path on sync, warn on missing.
- **Identity conflicts**: Each vault has independent identity. No risk of cross-vault key confusion.
- **Resource exhaustion**: Many vaults syncing. Mitigate: sequential processing, configurable limit.

## Security Considerations
- Manifest file at `~/.agentic-note/vaults.toml` stores vault paths (not secrets)
- Each vault's config (API keys, identity) stays in its own `.agentic/` directory
- No cross-vault data leakage

## Next Steps
- Depends on existing SyncEngine (already stable)
- Future: parallel vault sync with resource limits
- Future: vault groups for selective sync
