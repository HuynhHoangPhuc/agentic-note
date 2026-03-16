# Phase 1: Cargo.toml Files

**Status:** DONE | **Priority:** Critical | **Effort:** Small

## Overview

Rename all package names and dependency references in Cargo.toml files across the workspace.

## Files to Modify

1. `Cargo.toml` (workspace root) — workspace deps `agentic-note-*` → `zenon-*`
2. `crates/core/Cargo.toml` — package name
3. `crates/vault/Cargo.toml` — package name + deps
4. `crates/cas/Cargo.toml` — package name + deps
5. `crates/search/Cargo.toml` — package name + deps
6. `crates/agent/Cargo.toml` — package name + deps
7. `crates/review/Cargo.toml` — package name + deps
8. `crates/sync/Cargo.toml` — package name + deps
9. `crates/cli/Cargo.toml` — package name + deps + `[[bin]]` name
10. `crates/test-utils/Cargo.toml` — package name + deps
11. `crates/integration-tests/Cargo.toml` — package name + deps

## Replacements

- `name = "agentic-note-*"` → `name = "zenon-*"`
- `name = "agentic-note"` → `name = "zenon"` (CLI binary)
- `agentic-note-core = { path = ...}` → `zenon-core = { path = ...}`
- Same pattern for all crate references

## Implementation Steps

1. Replace in root `Cargo.toml`: all `agentic-note-` → `zenon-` in workspace deps
2. Replace in each crate's `Cargo.toml`: package name and dependency references
3. Run `cargo check --workspace` to validate

## Todo

- [x] Root Cargo.toml
- [x] All 10 crate Cargo.toml files
- [x] Verify `cargo check` passes

## Completion Notes

- All 11 Cargo.toml files updated successfully
- All package names: agentic-note-* → zenon-*
- All workspace dependencies updated
- All internal crate references updated
- cargo metadata confirms 10 packages now named zenon-*
