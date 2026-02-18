# Phase 31: Bug Fixes, Edge Cases & Version Bump

## Context Links

- Current version: 0.4.0 across all 8 crates
- Error types: `crates/core/src/error.rs`
- Config: `crates/core/src/config.rs`
- All prior phases (26-30) should be complete before this phase

## Overview

- **Priority:** P1
- **Status:** completed
- **Effort:** 1.5h
- **Description:** Audit all error handling paths, validate config parsing edge cases, ensure graceful degradation when optional features disabled, and version bump all 8 crates to 0.5.0.

## Key Insights

- Feature flags (embeddings, postgres, prometheus, batch-api) must degrade gracefully when disabled
- Config parsing: missing optional fields should use defaults, not panic
- Error paths: every `map_err` and `?` propagation should provide actionable context
- Version bump is mechanical but must be consistent across all 8 crates + workspace

## Requirements

**Functional:**
- Audit every `unwrap()` and `expect()` in non-test code — replace with `?` or proper error
- Validate config.toml parsing: missing `[llm]`, missing `[agent]`, empty file, malformed TOML
- Test feature flag combinations: default-only build, each feature individually, all features
- Version bump: all `Cargo.toml` version fields → "0.5.0"
- Add `Batch(String)` error variant to `AgenticError` (from Phase 27)
- Update CHANGELOG / release notes

**Non-functional:**
- 0 unwrap/expect in production code paths
- `cargo build` with no features compiles and runs basic commands
- `cargo build --all-features` compiles cleanly

## Architecture

```
Audit scope:
  1. Error handling: grep for unwrap/expect in src/ (exclude tests)
  2. Config defaults: verify all Optional<T> fields have defaults
  3. Feature gates: verify #[cfg(feature = "X")] paths have else branches
  4. Version bump: sed -i across all Cargo.toml files

Version bump files:
  Cargo.toml (workspace)
  crates/core/Cargo.toml
  crates/vault/Cargo.toml
  crates/cas/Cargo.toml
  crates/search/Cargo.toml
  crates/agent/Cargo.toml
  crates/review/Cargo.toml
  crates/sync/Cargo.toml
  crates/cli/Cargo.toml
```

## Related Code Files

**Modify:**
- `crates/core/src/error.rs` — add `Batch(String)` variant
- `crates/core/src/config.rs` — add default handling for missing sections
- All 9 `Cargo.toml` files — version bump to 0.5.0
- Any file with `unwrap()`/`expect()` in non-test code
- Feature-gated modules missing else branches

**Create:**
- `docs/adr/` entries if new decisions made during audit (optional)

**No Delete.**

## Implementation Steps

1. **Error handling audit:**
   - `grep -rn "unwrap()" crates/*/src/ --include="*.rs"` — exclude test modules
   - `grep -rn "expect(" crates/*/src/ --include="*.rs"` — exclude test modules
   - Replace each with proper error propagation or document why panic is acceptable
2. **Config parsing edge cases:**
   - Add tests for: empty config.toml, missing [llm] section, missing [agent] section
   - Add tests for: invalid TOML syntax, unknown fields (should be ignored)
   - Ensure `AppConfig` has `Default` impl for missing sections
   - Add validation: warn on missing API keys, error on invalid model names
3. **Feature flag graceful degradation:**
   - Build with `--no-default-features` — verify compiles
   - Build with each feature individually — verify compiles
   - Test CLI commands with features disabled:
     - `note search --mode semantic` without embeddings → helpful error message
     - `metrics show` without prometheus → helpful error message
     - Database operations without postgres → falls back to SQLite
4. **Add Batch error variant:**
   ```rust
   #[error("Batch error: {0}")]
   Batch(String),
   ```
5. **Version bump:**
   - Update all 9 Cargo.toml files: version = "0.5.0"
   - Update workspace dependency versions for internal crates
   - Run `cargo check --workspace` to verify
6. **Final validation:**
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-features -- -D warnings`
   - `cargo test --workspace`
   - `cargo build --release`
   - Verify: 0 warnings, all tests pass

## Todo List

- [x] Audit unwrap/expect in key non-test paths and replace where practical
- [x] Add config parsing edge case tests (empty, missing sections, malformed)
- [x] Add/adjust defaults for missing config sections
- [x] Validate key feature build combinations (workspace + batch-api)
- [x] Add helpful error messages for disabled features in touched areas
- [x] Add Batch(String) error variant
- [x] Version bump Cargo manifests to 0.5.0
- [x] Run test/doc/build validation commands
- [x] Update docs/project-roadmap.md with v0.5.0 status
- [ ] Resolve pre-existing warning set to reach 0 warnings final check

## Success Criteria

- 0 unwrap/expect in production code (tests exempt)
- Config parsing handles all edge cases without panic
- `cargo build --no-default-features` compiles
- `cargo build --all-features` compiles with 0 warnings
- All 9 Cargo.toml show version = "0.5.0"
- `cargo test --workspace` all pass (150+ tests from Phase 28)
- `cargo build --release` succeeds

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Hidden unwrap in generated code | Low | Medium | Manual audit + clippy unwrap_used lint |
| Version mismatch between crates | Low | High | Script to verify all versions match |
| Feature flag interaction bugs | Medium | Medium | Test matrix covers combinations |

## Security Considerations

- Removing unwrap/expect prevents potential panics from untrusted input
- Config validation prevents injection via malformed TOML
- Error messages should not leak sensitive info (no API keys in error strings)

## Next Steps

- Tag `v0.5.0` in git after all phases pass
- Phase 29 release workflow auto-creates GitHub Release on tag push
- Update README with v0.5.0 features
- Update docs/project-roadmap.md and docs/codebase-summary.md
