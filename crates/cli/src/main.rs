mod commands;
mod mcp;
mod output;

use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use commands::{Commands, ConfigCmd, McpCmd, PluginCmd};
use mcp::McpServer;
use output::OutputFormat;

#[derive(Parser)]
#[command(
    name = "agentic-note",
    version,
    about = "Local-first agentic note-taking"
)]
struct Cli {
    /// Path to vault (default: AGENTIC_NOTE_VAULT env or cwd)
    #[arg(long, global = true)]
    vault: Option<PathBuf>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Init tracing to stderr (stdout reserved for JSON-RPC / user output)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("AGENTIC_LOG"))
        .with_writer(std::io::stderr)
        .init();

    let fmt = if cli.json {
        OutputFormat::Json
    } else {
        OutputFormat::Human
    };

    let vault_path = match agentic_note_core::config::AppConfig::resolve_vault_path(cli.vault) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error resolving vault path: {e}");
            std::process::exit(1);
        }
    };

    let result: anyhow::Result<()> = match cli.command {
        Commands::Init { path } => commands::init::run(path, fmt),

        Commands::Note { cmd } => commands::note::run(cmd, &vault_path, fmt),

        Commands::Config { cmd } => match cmd {
            ConfigCmd::Show => commands::config::show(&vault_path, fmt),
        },

        Commands::Plugin { cmd } => match cmd {
            PluginCmd::List => commands::plugin::list(&vault_path, fmt),
        },

        Commands::Device { cmd } => commands::device::run(cmd, &vault_path, fmt),

        Commands::Mcp { cmd } => match cmd {
            McpCmd::Serve => {
                if let Err(e) = McpServer::new(vault_path).serve_stdio().await {
                    eprintln!("MCP server error: {e}");
                    std::process::exit(1);
                }
                Ok(())
            }
        },

        Commands::Sync { cmd } => commands::sync_cmd::run(cmd, &vault_path, fmt).await,
    };

    if let Err(e) = result {
        match fmt {
            OutputFormat::Json => {
                output::print_json(&serde_json::json!({"error": e.to_string()}));
            }
            OutputFormat::Human => {
                eprintln!("Error: {e}");
            }
        }
        std::process::exit(1);
    }
}
