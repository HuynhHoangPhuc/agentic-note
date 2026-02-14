# Phase 8: Integration & Testing

## Context Links
- [Codebase Summary](/Users/phuc/Developer/agentic-note/docs/codebase-summary.md)
- [Plan Overview](plan.md)

## Overview
- **Priority:** P1
- **Status:** completed
- **Effort:** 4h
- **Depends on:** All previous phases
- **Description:** Cross-feature integration tests, CLI completion, MCP tool additions, documentation updates, version bump to 0.2.0.

## Key Insights
- Currently 27 tests — target 60+ after v0.2.0
- New CLI commands: device (init/show/pair), sync (now/status), plugin (list)
- New MCP tools: note/search with mode, sync/status, plugin/list
- Docs need update: system-architecture, codebase-summary, project-roadmap

## Requirements

### Functional
- F1: All 27 existing tests still pass
- F2: Integration tests for each feature
- F3: Cross-feature tests: sync + conflict policies, DAG + plugins, hybrid search + pipeline
- F4: CLI commands fully wired: device, sync, plugin
- F5: MCP tools updated: search mode, sync status, plugin list
- F6: Version bump to 0.2.0 in all Cargo.toml files
- F7: Update sample pipeline TOML to v2 schema format

### Non-Functional
- `cargo test` completes in <60s
- `cargo clippy -- -D warnings` passes
- `cargo fmt -- --check` passes
- `cargo doc` generates without warnings

## Architecture

No new modules — this phase wires everything together and validates.

## Related Code Files

| File | Action | Changes |
|------|--------|---------|
| `/Users/phuc/Developer/agentic-note/crates/cli/src/commands/mod.rs` | modify | Ensure Device, Sync, Plugin commands registered |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/main.rs` | modify | Dispatch new commands |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/handlers.rs` | modify | Add sync/status, plugin/list tools |
| `/Users/phuc/Developer/agentic-note/crates/cli/src/mcp/tools.rs` | modify | Register new tool definitions |
| `/Users/phuc/Developer/agentic-note/pipelines/auto-process-inbox.toml` | modify | Upgrade to v2 schema with depends_on |
| `/Users/phuc/Developer/agentic-note/Cargo.toml` | modify | Version 0.2.0 |
| `/Users/phuc/Developer/agentic-note/crates/*/Cargo.toml` | modify | Version 0.2.0 |
| `/Users/phuc/Developer/agentic-note/docs/system-architecture.md` | modify | Add sync crate, update diagrams |
| `/Users/phuc/Developer/agentic-note/docs/codebase-summary.md` | modify | Add new crate, new CLI commands |
| `/Users/phuc/Developer/agentic-note/docs/project-roadmap.md` | modify | Mark v0.2.0 phases complete |

## Implementation Steps

1. **Wire CLI commands:**
   - Ensure `device.rs`, `sync.rs`, `plugin.rs` in `commands/mod.rs`
   - Add `Device`, `Sync`, `Plugin` variants to `Commands` enum
   - Add dispatch in `main.rs` (device/sync are async — handle like MCP)

2. **Wire MCP tools:**
   - Add `note/search` mode parameter (fts/semantic/hybrid)
   - Add `sync/status` tool (last sync, peer info)
   - Add `plugin/list` tool (discovered plugins)
   - Update tool definitions in `tools.rs`

3. **Update sample pipeline:**
   ```toml
   schema_version = 2
   name = "auto-process-inbox"
   # ... add depends_on to stages
   ```

4. **Integration tests — create test files:**
   - `crates/search/tests/hybrid_search_test.rs` — create notes, search semantic + hybrid
   - `crates/agent/tests/dag_pipeline_test.rs` — v2 pipeline with parallel stages
   - `crates/agent/tests/error_recovery_test.rs` — retry, abort, fallback
   - `crates/agent/tests/plugin_test.rs` — plugin discovery + execution
   - `crates/cas/tests/conflict_policy_test.rs` — each policy resolves correctly
   - `crates/sync/tests/sync_protocol_test.rs` — mock transport sync round-trip

5. **Cross-feature integration tests:**
   - `crates/cli/tests/integration_test.rs`:
     - Create vault → add notes → run DAG pipeline with plugin → search hybrid → verify
     - Sync two vaults (mock transport) → verify conflict resolution

6. **Version bump:**
   - Update `version = "0.2.0"` in all 8 Cargo.toml files (workspace + 7 crates)

7. **Quality checks:**
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   cargo test
   cargo doc --no-deps
   ```

8. **Documentation updates:**
   - `system-architecture.md`: add sync crate to dependency graph, update known limitations
   - `codebase-summary.md`: add sync crate, new CLI commands, new MCP tools, update LOC/test counts
   - `project-roadmap.md`: mark v0.2.0 features as complete

## Todo List

- [ ] Wire device/sync/plugin CLI commands
- [ ] Wire new MCP tools
- [ ] Update sample pipeline to v2 schema
- [ ] Integration test: hybrid search
- [ ] Integration test: DAG pipeline
- [ ] Integration test: error recovery
- [ ] Integration test: plugins
- [ ] Integration test: conflict policies
- [ ] Integration test: sync protocol
- [ ] Cross-feature integration test
- [ ] Version bump to 0.2.0
- [ ] cargo fmt passes
- [ ] cargo clippy passes
- [ ] cargo test passes (target 60+ tests)
- [ ] cargo doc generates cleanly
- [ ] Update system-architecture.md
- [ ] Update codebase-summary.md
- [ ] Update project-roadmap.md

## Success Criteria
- All existing 27 tests pass
- 30+ new tests added (target 60+ total)
- `cargo clippy -- -D warnings` clean
- `cargo fmt -- --check` clean
- All 6 features accessible via CLI and MCP
- Documentation reflects v0.2.0 architecture
- Sample pipeline uses v2 schema

## Risk Assessment
- **Low:** Integration testing is straightforward with tempfile fixtures
- **Medium:** Sync integration test needs mock transport — may be complex to set up
- **Low:** Version bump is mechanical

## Security Considerations
- Integration tests must not leak temp files
- No real API keys in test fixtures
- Test plugin executables use safe echo scripts

## Next Steps
- Tag release v0.2.0
- Update README with new features
- Plan v0.3.0 features
