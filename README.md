# zenon

Local-first agentic note-taking app in Rust. CLI + MCP server with PARA/Zettelkasten organization, CAS versioning, and AgentSpace pipelines.

## Features

- **PARA + Zettelkasten**: Organize notes in Projects/Areas/Resources/Archives/Inbox/Zettelkasten folders
- **YAML Frontmatter**: Each `.md` note has structured metadata (ID, tags, links, status)
- **Full-Text Search**: tantivy-powered FTS across titles, bodies, and tags
- **Tag/Link Graph**: SQLite-backed graph for backlinks, tag queries, orphan detection
- **CAS Versioning**: Content-addressable storage with SHA-256 snapshots, diff, and restore
- **AgentSpace Engine**: TOML-defined pipelines with sequential stage execution
- **4 Built-in Agents**: para-classifier, zettelkasten-linker, distiller, vault-writer
- **LLM Providers**: OpenAI, Anthropic, Ollama (local) support
- **Review Queue**: Human-in-the-loop approval for agent-proposed changes
- **MCP Server**: JSON-RPC stdio server for AI assistant integration
- **Trust Levels**: Manual, Review, or Auto approval modes

## Quick Start

```bash
# Build
cargo build --release

# Initialize a vault
zenon init ~/notes

# Create a note
zenon --vault ~/notes note create --title "My First Note" --para inbox --tags rust,cli

# List notes
zenon --vault ~/notes note list

# Search
zenon --vault ~/notes note list --para inbox

# JSON output (for scripts/agents)
zenon --vault ~/notes note list --json
```

## Configuration

Config lives at `<vault>/.zenon/config.toml`:

```toml
[vault]
path = "."

[llm]
default_provider = "openai"

[llm.providers.openai]
api_key = "sk-..."
model = "gpt-4o"

[agent]
default_trust = "review"
max_concurrent_pipelines = 1
```

## Architecture

Cargo workspace with 7 crates:

| Crate | Purpose |
|-------|---------|
| `core` | Shared types, errors, config, ULID IDs |
| `vault` | Note CRUD, frontmatter, PARA folders |
| `search` | tantivy FTS, SQLite graph |
| `cas` | Content-addressable storage, snapshots |
| `agent` | AgentSpace engine, LLM providers, built-in agents |
| `review` | SQLite review queue, approval gate |
| `cli` | Binary: CLI commands + MCP server |

## MCP Server

Start the MCP server for AI assistant integration:

```bash
zenon --vault ~/notes mcp serve
```

Available tools: `note/create`, `note/read`, `note/list`, `note/search`, `vault/init`, `vault/status`

## Pipelines

Define agent pipelines in `pipelines/*.toml`:

```toml
name = "auto-process-inbox"
enabled = true

[trigger]
trigger_type = "file_created"
path_filter = "inbox/"

[[stages]]
name = "classify"
agent = "para-classifier"
output = "classification"
```

## License

MIT
