/// MCP tool definitions returned by the `tools/list` method.
pub fn all_tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "note/create",
            "description": "Create a new note in the vault",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Note title" },
                    "para": { "type": "string", "description": "PARA category (inbox, projects, areas, resources, archives, zettelkasten)", "default": "inbox" },
                    "body": { "type": "string", "description": "Note body content (markdown)" },
                    "tags": { "type": "array", "items": { "type": "string" }, "description": "List of tags" }
                },
                "required": ["title"]
            }
        }),
        serde_json::json!({
            "name": "note/read",
            "description": "Read a note by ULID or file path",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Note ULID or file path" }
                },
                "required": ["target"]
            }
        }),
        serde_json::json!({
            "name": "note/list",
            "description": "List notes with optional filters",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "para": { "type": "string", "description": "Filter by PARA category" },
                    "tag":  { "type": "string", "description": "Filter by tag" },
                    "status": { "type": "string", "description": "Filter by status (seed, budding, evergreen)" },
                    "limit": { "type": "integer", "description": "Max results (default 50)", "default": 50 }
                }
            }
        }),
        serde_json::json!({
            "name": "note/search",
            "description": "Full-text search across notes",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Max results (default 10)", "default": 10 }
                },
                "required": ["query"]
            }
        }),
        serde_json::json!({
            "name": "vault/init",
            "description": "Initialize a new vault at the given path",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Vault path (default: current vault path)" }
                }
            }
        }),
        serde_json::json!({
            "name": "vault/status",
            "description": "Get vault status (note count, path, config)",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}
