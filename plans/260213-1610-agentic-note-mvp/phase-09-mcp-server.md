# Phase 09: MCP Server

## Context
- Parent: [plan.md](plan.md)
- Deps: Phase 03 (CLI), Phase 08 (Agents + Review)
- Research: [Rust Crates API](research/researcher-rust-crates-api.md)

## Overview
- **Priority:** P1 (enables AI agent consumption)
- **Status:** pending
- **Effort:** 8h
- **Description:** rmcp stdio transport MCP server exposing all tools (note, graph, review, agent, sync), agent-consumable JSON output. `agentic-note mcp serve --stdio` command.

## Key Insights
- rmcp 0.1.x: API unstable — wrap in thin adapter; fallback to manual JSON-RPC if crate breaks
- MCP server uses stdio (stdin/stdout JSON-RPC) — NEVER write logs to stdout (use stderr)
- All CLI commands have JSON mode — MCP tools call same core logic, format as MCP CallToolResult
- Tool input schemas defined as JSON Schema objects
- Each tool = one MCP function: `note/create`, `note/search`, `review/approve`, etc.

## Requirements

**Functional:**
- `agentic-note mcp serve --stdio` starts MCP server
- Tools exposed:
  - `note/create`, `note/read`, `note/update`, `note/delete`, `note/list`
  - `note/search` (FTS), `note/search-semantic` (embeddings)
  - `graph/tags`, `graph/backlinks`, `graph/orphans`
  - `review/list`, `review/show`, `review/approve`, `review/reject`
  - `agent/classify`, `agent/link-suggest`, `agent/distill`
  - `agent/pipeline-run` (trigger pipeline manually)
  - `sync/status`, `sync/now`
  - `vault/status` (vault info, note count, index status)
- Each tool has JSON Schema for input parameters
- All tools return `CallToolResult` with JSON content

**Non-functional:**
- Startup time < 200ms
- Tool call latency: same as CLI equivalent + ~5ms MCP overhead
- Graceful shutdown on stdin close / SIGTERM

## Architecture

```
crates/cli/src/
├── mcp/
│   ├── mod.rs          # MCP server handler struct
│   ├── tools.rs        # Tool definitions (name, schema, description)
│   ├── handlers/
│   │   ├── mod.rs      # dispatch tool call to handler
│   │   ├── note.rs     # note/* tool handlers
│   │   ├── search.rs   # search tool handlers
│   │   ├── graph.rs    # graph/* tool handlers
│   │   ├── review.rs   # review/* tool handlers
│   │   ├── agent.rs    # agent/* tool handlers
│   │   ├── sync.rs     # sync/* tool handlers
│   │   └── vault.rs    # vault/* tool handlers
│   └── schema.rs       # JSON Schema builder helpers
```

MCP server lives in CLI crate (binary crate) since it's an interface layer.

## Related Code Files

**Create:**
- `crates/cli/src/mcp/mod.rs`
- `crates/cli/src/mcp/tools.rs`
- `crates/cli/src/mcp/schema.rs`
- `crates/cli/src/mcp/handlers/mod.rs`
- `crates/cli/src/mcp/handlers/note.rs`
- `crates/cli/src/mcp/handlers/search.rs`
- `crates/cli/src/mcp/handlers/graph.rs`
- `crates/cli/src/mcp/handlers/review.rs`
- `crates/cli/src/mcp/handlers/agent.rs`
- `crates/cli/src/mcp/handlers/sync.rs`
- `crates/cli/src/mcp/handlers/vault.rs`

**Modify:**
- `crates/cli/src/main.rs` — add `mcp serve` subcommand
- `crates/cli/src/commands/mod.rs` — add Mcp subcommand
- `crates/cli/Cargo.toml` — add rmcp dep

## Cargo.toml Dependencies (additions to cli crate)
```toml
rmcp = { version = "0.1", features = ["server", "transport-io"] }
```

## Implementation Steps

1. **`tools.rs`:** Define all MCP tools
   - Each tool: name, description, input JSON Schema
   - `fn all_tools() -> Vec<Tool>` — returns complete tool catalog
   - Example tool definition:
   ```rust
   Tool {
       name: "note/create".into(),
       description: Some("Create a new note".into()),
       input_schema: json!({
           "type": "object",
           "properties": {
               "title": {"type": "string"},
               "para": {"type": "string", "enum": ["projects","areas","resources","archives","inbox","zettelkasten"]},
               "body": {"type": "string"},
               "tags": {"type": "array", "items": {"type": "string"}}
           },
           "required": ["title"]
       }),
   }
   ```

2. **`schema.rs`:** Helper functions for JSON Schema construction
   - `fn string_prop(desc: &str) -> Value`
   - `fn enum_prop(desc: &str, values: &[&str]) -> Value`
   - `fn array_prop(desc: &str, item_type: &str) -> Value`
   - `fn object_schema(props: Vec<(&str, Value)>, required: Vec<&str>) -> Value`

3. **`handlers/mod.rs`:** Central dispatch
   ```rust
   pub async fn handle_tool(name: &str, args: Value, ctx: &AppContext) -> Result<CallToolResult> {
       match name {
           n if n.starts_with("note/") => note::handle(n, args, ctx).await,
           n if n.starts_with("graph/") => graph::handle(n, args, ctx).await,
           n if n.starts_with("review/") => review::handle(n, args, ctx).await,
           n if n.starts_with("agent/") => agent::handle(n, args, ctx).await,
           n if n.starts_with("sync/") => sync::handle(n, args, ctx).await,
           "vault/status" => vault::handle(n, args, ctx).await,
           _ => Err(anyhow!("Unknown tool: {name}")),
       }
   }
   ```

4. **`handlers/note.rs`:** Note tool implementations
   - `note/create`: parse args → Note::create() → return JSON with id + path
   - `note/read`: parse id → Note::read() → return frontmatter + body
   - `note/update`: parse args → Note::update() → return updated note
   - `note/delete`: parse id → Note::delete() → return confirmation
   - `note/list`: parse filters → Vault::list_notes() → return array
   - `note/search`: parse query → SearchEngine::search_fts() → return results
   - `note/search-semantic`: parse query → SearchEngine::search_semantic() → return results

5. **Other handlers:** Same pattern — parse args, call core logic, return JSON CallToolResult

6. **`mod.rs`:** MCP server struct implementing `ServerHandler`
   ```rust
   struct AgenticNoteMcp { ctx: AppContext }

   impl ServerHandler for AgenticNoteMcp {
       fn get_info(&self) -> ServerInfo { ... }
       fn list_tools(&self) -> Vec<Tool> { tools::all_tools() }
       async fn call_tool(&self, name: &str, args: Value) -> Result<CallToolResult> {
           handlers::handle_tool(name, args, &self.ctx).await
       }
   }
   ```
   - `AppContext` holds: Vault, SearchEngine, ReviewQueue, AgentSpace, SyncEngine
   - Initialize all components on startup

7. **CLI integration:**
   - `agentic-note mcp serve --stdio` — start MCP server
   - Init all crate components, create AppContext
   - `AgenticNoteMcp.serve(stdio()).await`
   - Tracing goes to stderr ONLY

8. **Error handling:**
   - Tool errors return `CallToolResult { is_error: true, content: [Content::text(err)] }`
   - Never panic — all errors converted to error results
   - Timeout per tool call (30s default)

9. **Test with MCP inspector:** Use `npx @modelcontextprotocol/inspector` to validate tool discovery and calls

## Todo List
- [ ] Define all tool schemas
- [ ] Implement JSON Schema helpers
- [ ] Implement central tool dispatch
- [ ] Implement note/* handlers
- [ ] Implement search handlers
- [ ] Implement graph/* handlers
- [ ] Implement review/* handlers
- [ ] Implement agent/* handlers
- [ ] Implement sync/* handlers
- [ ] Implement vault/status handler
- [ ] Implement MCP ServerHandler
- [ ] Add `mcp serve` CLI command
- [ ] Test with MCP inspector
- [ ] Test with Claude Desktop

## Success Criteria
- `agentic-note mcp serve --stdio` starts and responds to JSON-RPC
- MCP inspector discovers all tools with correct schemas
- `note/create` via MCP creates file in vault
- `note/search` via MCP returns ranked results
- `review/list` via MCP returns pending review items
- Claude Desktop can connect and use tools
- No logs leak to stdout (stderr only)

## Risk Assessment
- **rmcp instability:** 0.1.x may break — prepare manual JSON-RPC fallback (simple stdin/stdout line parser)
- **Tool schema validation:** MCP clients may reject invalid schemas — test with multiple clients
- **AppContext initialization:** loading all components on startup adds latency — lazy-init optional components

## Security Considerations
- MCP stdio transport is local-only — no network exposure
- Tool calls execute with same permissions as CLI user
- No authentication on MCP (stdio assumed trusted) — document this
- Rate-limiting not needed for local stdio

## Next Steps
- Post-MVP: SSE transport for remote MCP connections
- Post-MVP: resource subscriptions for vault change notifications
- Post-MVP: prompt templates for common agent workflows
