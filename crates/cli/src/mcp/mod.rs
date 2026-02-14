/// MCP server: JSON-RPC 2.0 over stdio.
/// Reads one request per line from stdin, writes one response per line to stdout.
/// All tracing/log output goes exclusively to stderr.
pub mod handlers;
pub mod tools;

use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, warn};

pub struct McpServer {
    vault_path: PathBuf,
}

impl McpServer {
    pub fn new(vault_path: PathBuf) -> Self {
        Self { vault_path }
    }

    pub async fn serve_stdio(&self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        tracing::info!("MCP server started, vault={}", self.vault_path.display());

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                // EOF — client disconnected
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            debug!("rx: {trimmed}");

            let response = self.handle_request(trimmed).await;
            let mut out = serde_json::to_string(&response)
                .unwrap_or_else(|e| internal_error_str(None, &e.to_string()));
            out.push('\n');

            debug!("tx: {}", out.trim());
            stdout.write_all(out.as_bytes()).await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    async fn handle_request(&self, raw: &str) -> serde_json::Value {
        // Parse JSON
        let req: serde_json::Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(e) => {
                warn!("parse error: {e}");
                return jsonrpc_error(None, -32700, "Parse error", &e.to_string());
            }
        };

        let id = req.get("id").cloned();
        let method = match req["method"].as_str() {
            Some(m) => m,
            None => {
                return jsonrpc_error(id.as_ref(), -32600, "Invalid Request", "missing method");
            }
        };
        let params = req
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        match method {
            "initialize" => self.handle_initialize(id),
            "tools/list" => self.handle_tools_list(id),
            "tools/call" => self.handle_tools_call(id, params).await,
            other => {
                warn!("unknown method: {other}");
                jsonrpc_error(id.as_ref(), -32601, "Method not found", other)
            }
        }
    }

    fn handle_initialize(&self, id: Option<serde_json::Value>) -> serde_json::Value {
        jsonrpc_ok(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "agentic-note", "version": "0.2.0" }
            }),
        )
    }

    fn handle_tools_list(&self, id: Option<serde_json::Value>) -> serde_json::Value {
        jsonrpc_ok(id, serde_json::json!({ "tools": tools::all_tools() }))
    }

    async fn handle_tools_call(
        &self,
        id: Option<serde_json::Value>,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let tool_name = match params["name"].as_str() {
            Some(n) => n.to_string(),
            None => {
                return jsonrpc_error(id.as_ref(), -32602, "Invalid params", "missing tool name");
            }
        };
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Object(Default::default()));

        match handlers::handle_tool(&tool_name, args, &self.vault_path).await {
            Ok(result) => {
                let text =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                jsonrpc_ok(
                    id,
                    serde_json::json!({
                        "content": [{ "type": "text", "text": text }]
                    }),
                )
            }
            Err(e) => {
                warn!("tool {tool_name} error: {e}");
                jsonrpc_error(id.as_ref(), -32603, "Tool error", &e.to_string())
            }
        }
    }
}

// --- JSON-RPC helpers --------------------------------------------------------

fn jsonrpc_ok(id: Option<serde_json::Value>, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn jsonrpc_error(
    id: Option<&serde_json::Value>,
    code: i32,
    message: &str,
    data: &str,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message, "data": data }
    })
}

fn internal_error_str(id: Option<&serde_json::Value>, data: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32603, "message": "Internal error", "data": data }
    })
    .to_string()
}
