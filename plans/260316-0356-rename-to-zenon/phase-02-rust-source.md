# Phase 2: Rust Source Code

**Status:** DONE | **Priority:** Critical | **Effort:** Medium

## Overview

Rename all Rust references: `use agentic_note_*`, string literals containing "agentic-note", and `.agentic` directory references.

## Replacements

| Pattern | Replacement | Context |
|---------|-------------|---------|
| `agentic_note_core` | `zenon_core` | use/extern crate statements |
| `agentic_note_vault` | `zenon_vault` | use/extern crate statements |
| `agentic_note_cas` | `zenon_cas` | use/extern crate statements |
| `agentic_note_search` | `zenon_search` | use/extern crate statements |
| `agentic_note_agent` | `zenon_agent` | use/extern crate statements |
| `agentic_note_review` | `zenon_review` | use/extern crate statements |
| `agentic_note_sync` | `zenon_sync` | use/extern crate statements |
| `agentic_note_cli` | `zenon_cli` | use/extern crate statements |
| `"agentic-note"` | `"zenon"` | String literals (CLI name, etc.) |
| `.agentic` | `.zenon` | Config dir path in vault code |

## Files (~87 .rs files)

All files in `crates/*/src/**/*.rs` and `crates/*/tests/**/*.rs` that contain any of the above patterns.

## Implementation Steps

1. Replace `agentic_note_` → `zenon_` in all .rs files (crate references)
2. Replace `"agentic-note"` → `"zenon"` in string literals
3. Replace `.agentic` → `.zenon` in path/config references
4. Run `cargo check --workspace` to validate

## Todo

- [x] Crate reference renames (use statements)
- [x] String literal renames
- [x] `.agentic` → `.zenon` directory references
- [x] Verify `cargo check` passes

## Completion Notes

- All .rs files updated across crates/*/src and crates/*/tests
- All use/extern crate statements: agentic_note_* → zenon_*
- All string literals: "agentic-note" → "zenon"
- All config paths: .agentic → .zenon
- Crate references fully migrated
