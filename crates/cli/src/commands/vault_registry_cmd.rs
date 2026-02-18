/// CLI commands for managing the multi-vault registry.
///
/// Subcommands: register, unregister, list.
use agentic_note_sync::VaultRegistry;
use clap::Subcommand;
use std::path::PathBuf;

use crate::output::{print_json, OutputFormat};

#[derive(Subcommand)]
pub enum VaultRegistryCmd {
    /// Register a vault in the multi-vault registry
    Register {
        /// Path to the vault directory
        path: PathBuf,
        /// Human-readable name for this vault
        #[arg(long, default_value = "")]
        name: String,
    },
    /// Unregister a vault from the multi-vault registry
    Unregister {
        /// Path of the vault to remove
        path: PathBuf,
    },
    /// List all registered vaults
    List,
}

pub fn run(cmd: VaultRegistryCmd, fmt: OutputFormat) -> anyhow::Result<()> {
    match cmd {
        VaultRegistryCmd::Register { path, name } => {
            let mut registry = VaultRegistry::load()?;
            // Use last path segment as default name when not provided.
            let resolved_name = if name.is_empty() {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("vault")
                    .to_string()
            } else {
                name
            };
            registry.register(path.clone(), resolved_name.clone())?;
            registry.save()?;

            match fmt {
                OutputFormat::Json => print_json(&serde_json::json!({
                    "registered": true,
                    "path": path.display().to_string(),
                    "name": resolved_name,
                })),
                OutputFormat::Human => {
                    println!("Registered vault '{}' at {}", resolved_name, path.display());
                }
            }
        }

        VaultRegistryCmd::Unregister { path } => {
            let mut registry = VaultRegistry::load()?;
            registry.unregister(&path)?;
            registry.save()?;

            match fmt {
                OutputFormat::Json => print_json(&serde_json::json!({
                    "unregistered": true,
                    "path": path.display().to_string(),
                })),
                OutputFormat::Human => {
                    println!("Unregistered vault at {}", path.display());
                }
            }
        }

        VaultRegistryCmd::List => {
            let registry = VaultRegistry::load()?;
            let vaults = registry.list();

            match fmt {
                OutputFormat::Json => {
                    let entries: Vec<_> = vaults
                        .iter()
                        .map(|v| {
                            serde_json::json!({
                                "path": v.path.display().to_string(),
                                "name": v.name,
                                "sync_enabled": v.sync_enabled,
                                "default_peers": v.default_peers,
                            })
                        })
                        .collect();
                    print_json(&serde_json::json!({ "vaults": entries }));
                }
                OutputFormat::Human => {
                    if vaults.is_empty() {
                        println!("No vaults registered. Use `vault register <path>` to add one.");
                    } else {
                        println!("Registered vaults ({}):", vaults.len());
                        println!("{:<25} {:<30} Sync", "Name", "Path");
                        println!("{}", "-".repeat(62));
                        for v in vaults {
                            println!(
                                "{:<25} {:<30} {}",
                                v.name,
                                v.path.display(),
                                if v.sync_enabled {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
