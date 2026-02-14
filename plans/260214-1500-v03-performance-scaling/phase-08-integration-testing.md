---
phase: 8
title: "Integration & Testing"
status: complete
effort: 1h
depends_on: [2, 3, 4, 5, 6, 7]
---

## Context Links

- [All phase files in this plan](.)
- [Code standards: Testing](../../docs/code-standards.md)
- [System architecture](../../docs/system-architecture.md)

## Overview

Cross-feature integration testing, feature-flag matrix validation, CLI end-to-end tests, and documentation updates. Ensures all v0.3.0 features work together and individually.

## Key Insights

- Features interact: background indexer + pipeline scheduler both use notify-debouncer-full
- Delta sync + batch sync + semantic merge must compose correctly
- Feature flags (prometheus, tui, embeddings) must not break compilation in any combination
- Existing 30+ tests must continue passing

## Requirements

**Functional:**
- All existing tests pass
- Cross-feature integration tests cover key interaction points
- Feature-flag matrix compiles: `default`, `prometheus`, `tui`, `embeddings`, `all`
- CLI end-to-end tests for new commands

**Non-functional:**
- Test suite completes in < 60s
- No flaky tests (use timeouts and deterministic ordering)

## Architecture

Test matrix:

| Test | Features Involved | Type |
|------|------------------|------|
| Delta sync round-trip | sync, cas | unit |
| Batch sync convergence | sync, cas | integration |
| Semantic merge clean | cas, diffy | unit |
| Semantic merge + LLM fallback | cas, agent | integration |
| Background indexer file create | search, notify | integration |
| Pipeline cron trigger | agent, scheduler | integration |
| Pipeline file-watch trigger | agent, notify | integration |
| Metrics instrumentation | vault, agent, sync | integration |
| CLI `sync now --all` | cli, sync | e2e |
| CLI `pipeline schedule` | cli, agent | e2e |
| CLI `metrics watch` | cli, ratatui | e2e (manual) |

## Related Code Files

**Create:**
- `crates/sync/tests/delta_sync_integration.rs`
- `crates/sync/tests/batch_sync_integration.rs`
- `crates/cas/tests/semantic_merge_integration.rs`
- `crates/search/tests/background_indexer_integration.rs`
- `crates/agent/tests/scheduler_integration.rs`
- `crates/cli/tests/cli_v030_integration.rs`

**Modify:**
- `docs/system-architecture.md` -- update for v0.3.0 features
- `docs/codebase-summary.md` -- update LOC, feature list, deps
- `docs/project-roadmap.md` -- mark v0.3.0 phases complete

## Implementation Steps

1. **Feature-flag compilation matrix**:
   ```bash
   cargo check                                          # default
   cargo check --features prometheus                    # prometheus only
   cargo check --features tui                           # tui only
   cargo check --features embeddings                    # embeddings only
   cargo check --all-features                           # everything
   cargo check --no-default-features                    # minimal
   ```
   Verify all 6 combinations compile without errors.

2. **Delta sync integration test** (`crates/sync/tests/delta_sync_integration.rs`):
   - Create two temp vaults with same base content
   - Modify a note in vault A
   - Compute delta, compress, decompress, apply to vault B
   - Verify content matches via SHA-256

3. **Batch sync integration test** (`crates/sync/tests/batch_sync_integration.rs`):
   - Create 3 temp vaults with mock transport
   - Make different changes in each
   - Run `sync_all_peers()` from vault A
   - Verify vault A has merged content from B and C
   - Verify no data loss

4. **Semantic merge test** (`crates/cas/tests/semantic_merge_integration.rs`):
   - Test: ancestor="A\n\nB", local="A-modified\n\nB", remote="A\n\nB-modified" → clean merge
   - Test: ancestor="X", local="Y", remote="Z" → conflict (needs LLM or manual)
   - Test: both same edit → dedup, no conflict

5. **Background indexer test** (`crates/search/tests/background_indexer_integration.rs`):
   - Spawn indexer on temp vault
   - Write a .md file
   - Wait 500ms
   - Search for content → expect found
   - Delete the file
   - Wait 500ms
   - Search → expect not found

6. **Scheduler test** (`crates/agent/tests/scheduler_integration.rs`):
   - Create scheduler with mock executor
   - Add file-watch trigger on temp dir
   - Create .md file in temp dir
   - Assert pipeline executed within 1s
   - Remove schedule, create another file, assert NOT executed

7. **CLI e2e test** (`crates/cli/tests/cli_v030_integration.rs`):
   - Test `pipeline schedule <path> --cron "* * * * *"` returns success
   - Test `pipeline list-schedules` shows scheduled pipeline
   - Test `sync now --all` with no peers returns empty result
   - Test new error messages for invalid cron expressions

8. **Run full test suite**:
   ```bash
   cargo test --workspace
   cargo test --workspace --all-features
   ```

9. **Update documentation**:
   - `docs/system-architecture.md`: add background indexer, scheduler, metrics, delta sync, batch sync, semantic merge sections
   - `docs/codebase-summary.md`: update LOC (~10,500), dep count, feature list
   - `docs/project-roadmap.md`: mark v0.3.0 features as complete
   - `README.md`: add new CLI commands to quick start

10. **Version bump**: update `version = "0.3.0"` in all crate `Cargo.toml` files.

## Todo List

- [ ] Feature-flag compilation matrix (6 combos)
- [ ] Delta sync integration test
- [ ] Batch sync integration test
- [ ] Semantic merge integration test
- [ ] Background indexer integration test
- [ ] Scheduler integration test
- [ ] CLI e2e tests
- [ ] Run full `cargo test --workspace`
- [ ] Run `cargo clippy -- -D warnings`
- [ ] Update system-architecture.md
- [ ] Update codebase-summary.md
- [ ] Update project-roadmap.md
- [ ] Update README.md
- [ ] Version bump to 0.3.0

## Success Criteria

- All tests pass: `cargo test --workspace --all-features`
- `cargo clippy -- -D warnings` clean
- `cargo fmt -- --check` clean
- All 6 feature-flag combinations compile
- Documentation reflects v0.3.0 state
- Version bumped to 0.3.0 in all Cargo.toml

## Risk Assessment

- **Flaky integration tests**: FS watcher timing-dependent. Use generous timeouts (1-2s) and retry
- **Feature flag conflicts**: untested combinations may fail. Mitigation: explicit matrix test
- **Doc drift**: ensure docs match implementation, not plan

## Security Considerations

- Integration tests must not leak temp files (use `tempfile` crate with auto-cleanup)
- No real API keys in tests -- mock LLM providers
- CI pipeline should run `cargo audit` before release

## Next Steps

Tag `v0.3.0` release after all tests pass and docs updated.
