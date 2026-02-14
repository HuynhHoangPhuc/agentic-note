# Phase 03: CLI Interface

## Context
- Parent: [plan.md](plan.md)
- Deps: Phase 02 (vault & notes)
- Research: [Architecture Brainstorm](../reports/brainstorm-260213-1552-agentic-note-app.md)

## Overview
- **Priority:** P1 (primary user interface)
- **Status:** pending
- **Effort:** 8h
- **Description:** clap-based CLI binary with all note CRUD commands, `--json` output mode, vault init, config management. Binary crate that becomes the `agentic-note` executable.

## Key Insights
- Every command supports `--json` flag for agent consumption (MCP parity)
- Use clap derive API for clean subcommand definitions
- Binary lives in `crates/cli/` (not workspace root) — cleaner separation
- `tracing-subscriber` for structured logging to stderr (stdout reserved for output)

## Requirements

**Functional:**
- `agentic-note init [path]` — create vault
- `agentic-note note create --title <t> --para <p> [--tags <t>...] [--body <b>]`
- `agentic-note note read <id-or-path>`
- `agentic-note note update <id> [--title] [--tags] [--para] [--body] [--status]`
- `agentic-note note delete <id>`
- `agentic-note note list [--para <p>] [--tag <t>] [--status <s>] [--limit <n>]`
- `agentic-note config show` / `config set <key> <value>`
- Global flags: `--vault <path>`, `--json`, `--verbose`
- Exit code 0 on success, 1 on error; errors to stderr

**Non-functional:**
- Startup time < 50ms for simple commands
- JSON output matches MCP tool response schemas

## Architecture

```
crates/cli/src/
├── main.rs           # entrypoint, clap parse, dispatch
├── commands/
│   ├── mod.rs        # subcommand enum
│   ├── init.rs       # vault init command
│   ├── note.rs       # note CRUD subcommands
│   └── config.rs     # config show/set
└── output.rs         # OutputFormat (human/json), formatting helpers
```

## Related Code Files

**Create:**
- `crates/cli/Cargo.toml` (update stub)
- `crates/cli/src/main.rs`
- `crates/cli/src/commands/mod.rs`
- `crates/cli/src/commands/init.rs`
- `crates/cli/src/commands/note.rs`
- `crates/cli/src/commands/config.rs`
- `crates/cli/src/output.rs`

## Cargo.toml Dependencies
```toml
[package]
name = "agentic-note"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "agentic-note"
path = "src/main.rs"

[dependencies]
agentic-note-core = { path = "../core" }
agentic-note-vault = { path = "../vault" }
clap = { version = "4", features = ["derive"] }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = { workspace = true }
```

## Implementation Steps

1. **`main.rs`:**
   - Parse `Cli` struct via clap derive
   - Init tracing-subscriber (stderr, env filter `AGENTIC_LOG`)
   - Resolve vault path (--vault > env > cwd)
   - Match subcommand, dispatch to handler
   - Wrap errors with anyhow, print to stderr

2. **`commands/mod.rs`:** Define clap subcommand enum:
   ```rust
   #[derive(Subcommand)]
   enum Commands {
       Init { path: Option<PathBuf> },
       Note { #[command(subcommand)] cmd: NoteCmd },
       Config { #[command(subcommand)] cmd: ConfigCmd },
   }
   ```

3. **`commands/init.rs`:**
   - Call `vault::init_vault(path)`
   - Print success message or JSON `{"status": "ok", "path": "..."}`

4. **`commands/note.rs`:**
   - `NoteCmd` enum: Create, Read, Update, Delete, List
   - Create: collect args, call `Note::create()`, output note summary
   - Read: resolve id/path, call `Note::read()`, output full note
   - Update: load note, apply changes, call `Note::update()`
   - Delete: confirm (unless --force), call `Note::delete()`
   - List: build `NoteFilter`, call `Vault::list_notes()`, output table or JSON array

5. **`commands/config.rs`:**
   - Show: read config, print as TOML or JSON
   - Set: parse dotted key path, update value, write config

6. **`output.rs`:**
   - `OutputFormat` enum: Human, Json
   - `fn print_note(note: &Note, fmt: OutputFormat)`
   - `fn print_notes(notes: &[NoteSummary], fmt: OutputFormat)` — table for human, JSON array for json
   - `fn print_error(err: &Error, fmt: OutputFormat)` — stderr always

7. **Add `agentic-note` binary to workspace root Cargo.toml** `default-members`

8. **Test CLI manually:** `cargo run -- init /tmp/test-vault`, `cargo run -- note create --title "Test"`

## Todo List
- [ ] Define Cli struct with clap derive
- [ ] Implement init command
- [ ] Implement note create/read/update/delete/list
- [ ] Implement config show/set
- [ ] Implement --json output mode
- [ ] Implement --vault global flag
- [ ] Manual smoke test all commands

## Success Criteria
- `agentic-note init /tmp/v` creates vault with PARA structure
- `agentic-note note create --title "Hello" --para inbox` creates file
- `agentic-note note list --json` returns JSON array
- `agentic-note note read <id>` shows frontmatter + body
- All commands return exit code 0 on success, 1 on error

## Risk Assessment
- **ID resolution ambiguity:** user may pass full ULID, partial prefix, or file path — need smart resolver
- **Config set:** dotted key path parsing for nested TOML values is non-trivial — keep simple for MVP (flat keys only or use `toml_edit`)

## Security Considerations
- `--force` flag for delete to prevent accidental data loss
- Do not echo API keys in `config show` (mask sensitive fields)

## Next Steps
- Phase 04 (Search) adds `search` subcommand
- Phase 08 (Agents) adds `agent`, `review`, `agentspace` subcommands
- Phase 09 (MCP) adds `mcp serve` subcommand
