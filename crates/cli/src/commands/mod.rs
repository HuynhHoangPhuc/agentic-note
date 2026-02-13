pub mod config;
pub mod init;
pub mod mcp_cmd;
pub mod note;

use clap::Subcommand;
use std::path::PathBuf;

pub use mcp_cmd::McpCmd;

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new vault
    Init {
        /// Path to create vault at (default: current directory)
        path: Option<PathBuf>,
    },
    /// Note operations (create, read, update, delete, list)
    Note {
        #[command(subcommand)]
        cmd: note::NoteCmd,
    },
    /// Show vault configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// MCP server operations
    Mcp {
        #[command(subcommand)]
        cmd: McpCmd,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Show current configuration
    Show,
}
