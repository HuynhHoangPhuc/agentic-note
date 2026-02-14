pub mod config;
pub mod device;
pub mod init;
pub mod mcp_cmd;
pub mod metrics_cmd;
pub mod note;
pub mod pipeline;
pub mod plugin;
pub mod sync_cmd;
pub mod vault_registry_cmd;

use clap::Subcommand;
use std::path::PathBuf;

pub use device::DeviceCmd;
pub use mcp_cmd::McpCmd;
pub use pipeline::PipelineCmd;
pub use sync_cmd::SyncCmd;

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
    /// Plugin operations
    Plugin {
        #[command(subcommand)]
        cmd: PluginCmd,
    },
    /// Device identity and pairing
    Device {
        #[command(subcommand)]
        cmd: DeviceCmd,
    },
    /// P2P sync operations
    Sync {
        #[command(subcommand)]
        cmd: SyncCmd,
    },
    /// Metrics and observability
    Metrics {
        #[command(subcommand)]
        cmd: MetricsCmd,
    },
    /// Pipeline scheduling and management
    Pipeline {
        #[command(subcommand)]
        cmd: PipelineCmd,
    },
    /// Vault registry operations (multi-vault management)
    VaultRegistry {
        #[command(subcommand)]
        cmd: vault_registry_cmd::VaultRegistryCmd,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Show current configuration
    Show,
}

#[derive(Subcommand)]
pub enum PluginCmd {
    /// List discovered plugins
    List,
}

#[derive(Subcommand)]
pub enum MetricsCmd {
    /// Show current metrics summary table
    Show,
}
