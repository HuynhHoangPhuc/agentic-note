use clap::Subcommand;

#[derive(Subcommand)]
pub enum McpCmd {
    /// Start MCP server over stdio (JSON-RPC 2.0)
    Serve,
}
