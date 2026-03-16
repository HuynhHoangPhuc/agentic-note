# Rename Project: agentic-note → zenon

**Status:** Complete (Phases 1-4: DONE | Phase 5: MANUAL)
**Priority:** High
**Complexity:** Low (mechanical find-and-replace, no logic changes)

## Scope

Rename all occurrences of `agentic-note` / `agentic_note` / `AgenticNote` to `zenon` / `zenon` / `Zenon` across the entire codebase.

## Mapping

| Old | New | Context |
|-----|-----|---------|
| `agentic-note` | `zenon` | Cargo package names, CLI binary, kebab-case refs |
| `agentic_note` | `zenon` | Rust crate names (use statements, module paths) |
| `agentic-note-core` | `zenon-core` | Workspace dependency names |
| `agentic-note-vault` | `zenon-vault` | etc for all crates |
| `.agentic` | `.zenon` | Config directory inside vaults |
| `AgenticNote` | `Zenon` | If any PascalCase exists |

## Phases

| # | Phase | Status | Files |
|---|-------|--------|-------|
| 1 | [Cargo.toml files](phase-01-cargo-toml.md) | DONE | 11 files |
| 2 | [Rust source code](phase-02-rust-source.md) | DONE | ~87 files |
| 3 | [CI/CD workflows](phase-03-ci-cd.md) | DONE | 3 files |
| 4 | [Documentation](phase-04-documentation.md) | DONE | README + docs/ |
| 5 | [Git remote + repo rename](phase-05-git-remote.md) | MANUAL | Manual |

## Exclusions

- **Plans/reports**: Historical docs in `plans/` — skip (they reference old name contextually)
- **Git history**: Not rewritten
- **GitHub repo rename**: Phase 5 is manual (user does via GitHub Settings)

## Risk

- Low. Purely mechanical rename. Cargo build after phases 1-2 validates correctness.
- Must update `.agentic` → `.zenon` in vault init/config code carefully.

## Success Criteria

- `cargo build --workspace` passes
- `cargo test --workspace` passes
- CLI binary name is `zenon`
- All docs reference `zenon`
