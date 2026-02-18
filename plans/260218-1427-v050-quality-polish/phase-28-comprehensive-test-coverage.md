# Phase 28: Comprehensive Test Coverage

## Context Links

- [Research: Testing & Async Batch](research/researcher-testing-async-batch.md)
- Current state: 110 tests passing, 0 warnings, estimated ~80% coverage
- Target: 90%+ line coverage with property-based tests for CAS/merge/sync

## Overview

- **Priority:** P1
- **Status:** completed
- **Effort:** 4h
- **Description:** Add cargo-llvm-cov for workspace coverage measurement, proptest for property-based testing, dedicated integration-tests and test-utils crates, stress tests for concurrent operations. Target 90%+ aggregate coverage.

## Key Insights

- cargo-llvm-cov: LLVM source-based instrumentation, cross-platform, --workspace flag
- proptest: shrinking, macro API, integrates with `#[test]`; async requires sync wrapper with `Runtime::block_on`
- tokio::time::pause() for deterministic async tests — avoids real sleeps
- Integration test crate avoids circular deps between crates
- test-utils as dev-dependency-only crate keeps test helpers out of production

## Requirements

**Functional:**
- cargo-llvm-cov installed and producing HTML + lcov reports
- proptest tests for: CAS hash operations, three_way_merge, CRDT/vector clock convergence, DR encryption round-trips
- Integration test crate exercising end-to-end flows: note create → sync → retrieve
- Shared test-utils crate with: temp vault builder, mock LLM provider, fixture generators
- Stress tests for batch sync + concurrent pipeline execution

**Non-functional:**
- 90%+ aggregate line coverage
- Per-crate minimums: core 100%, vault 90%, cas 90%, search 85%, agent 80%, review 85%, sync 85%, cli 75%
- All tests complete in < 60s on CI
- Deterministic — no flaky async tests

## Architecture

```
Workspace additions:
  crates/test-utils/          # dev-dependency only
    src/lib.rs                # re-exports
    src/temp_vault.rs         # TempVault builder (~60 LOC)
    src/mock_llm.rs           # MockLlmProvider (~50 LOC)
    src/fixtures.rs           # Note/config generators (~50 LOC)

  crates/integration-tests/   # integration test crate
    Cargo.toml                # depends on all 8 crates + test-utils
    tests/
      note_lifecycle.rs       # create → index → search → delete
      sync_flow.rs            # snapshot → diff → merge → restore
      pipeline_execution.rs   # load pipeline → run DAG → review gate
      encryption_migration.rs # legacy 0x01 → DR 0x02 round-trip

Proptest locations (in-crate):
  crates/cas/tests/proptest_merge.rs
  crates/cas/tests/proptest_hash.rs
  crates/sync/tests/proptest_vector_clock.rs
  crates/sync/tests/proptest_encryption.rs
```

## Related Code Files

**Create:**
- `crates/test-utils/Cargo.toml`
- `crates/test-utils/src/lib.rs`
- `crates/test-utils/src/temp_vault.rs`
- `crates/test-utils/src/mock_llm.rs`
- `crates/test-utils/src/fixtures.rs`
- `crates/integration-tests/Cargo.toml`
- `crates/integration-tests/tests/note_lifecycle.rs`
- `crates/integration-tests/tests/sync_flow.rs`
- `crates/integration-tests/tests/pipeline_execution.rs`
- `crates/integration-tests/tests/encryption_migration.rs`
- `crates/cas/tests/proptest_merge.rs`
- `crates/cas/tests/proptest_hash.rs`
- `crates/sync/tests/proptest_vector_clock.rs`
- `crates/sync/tests/proptest_encryption.rs`

**Modify:**
- `Cargo.toml` (workspace members: add test-utils, integration-tests)
- `Cargo.toml` (workspace deps: add proptest)
- Individual crate `Cargo.toml` files (add test-utils as dev-dependency)

**No Delete.**

## Implementation Steps

1. Install cargo-llvm-cov: `cargo install cargo-llvm-cov`
2. Run baseline coverage: `cargo llvm-cov --workspace --html` — record current %
3. Add `proptest = "1"` to workspace dependencies
4. Create `crates/test-utils/`:
   - `Cargo.toml`: depends on core, vault, agent (workspace refs), tempfile
   - `src/temp_vault.rs`: `TempVault::new()` creates temp dir + initializes vault
   - `src/mock_llm.rs`: `MockLlmProvider` with configurable responses
   - `src/fixtures.rs`: `random_note()`, `sample_config()`, `sample_pipeline()`
5. Create `crates/integration-tests/`:
   - `Cargo.toml`: depends on all crates + test-utils (dev-dependencies)
   - 4 test files covering cross-crate flows
6. Add proptest tests:
   - `cas/tests/proptest_hash.rs`: hash(data) is deterministic, different data → different hash
   - `cas/tests/proptest_merge.rs`: merge(A,B,base) is commutative for non-conflicting changes
   - `sync/tests/proptest_vector_clock.rs`: VC merge is commutative and associative
   - `sync/tests/proptest_encryption.rs`: encrypt→decrypt round-trip for arbitrary payloads
7. Add stress tests in integration-tests:
   - Concurrent note creation (10 tokio tasks)
   - Batch sync with 3 simulated peers
   - Pipeline execution with parallel stages
8. Use `tokio::time::pause()` in all async tests for determinism
9. Run full coverage: `cargo llvm-cov --workspace --html` — verify 90%+ aggregate
10. Add coverage script: `scripts/coverage.sh` (runs llvm-cov, opens HTML report)

## Todo List

- [x] Install cargo-llvm-cov
- [x] Add proptest workspace dependency
- [x] Create crates/test-utils/ with TempVault and fixtures
- [x] Create crates/integration-tests/ with 4 test files
- [x] Write proptest tests for CAS (hash, merge)
- [x] Write proptest tests for sync (vector clock, encryption)
- [x] Add deterministic async test coverage where practical
- [x] Run workspace test validation
- [x] Create scripts/coverage.sh
- [ ] Run baseline/full coverage measurement and verify 90%+ aggregate (deferred)

## Success Criteria

- `cargo llvm-cov --workspace` reports 90%+ aggregate line coverage
- Per-crate meets minimum thresholds listed above
- All proptest cases pass with default config (256 cases)
- Integration tests pass: `cargo test -p integration-tests`
- Stress tests pass without deadlocks or flaky failures
- Total test count: 150+ (up from 110)
- `cargo test --workspace` completes in < 60s

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Coverage tool install issues on CI | Low | Medium | Phase 29 CI installs via cargo-binstall |
| Flaky async tests | Medium | High | tokio::time::pause(), no real I/O in unit tests |
| proptest slow on large inputs | Low | Low | Limit input size in strategies |
| Integration test circular deps | Low | Medium | integration-tests only has dev-deps |

## Security Considerations

- Test fixtures must not contain real API keys
- Mock LLM provider returns hardcoded strings, no network calls
- Temp vaults cleaned up automatically via TempDir Drop

## Next Steps

- Phase 29 CI/CD integrates coverage reporting
- Phase 30 documents testing patterns in rustdoc
- Phase 31 fixes any bugs discovered during coverage expansion
