/// Device management CLI commands: init, show, pair.
use agentic_note_sync::{DeviceIdentity, DeviceRegistry};
use clap::Subcommand;
use std::path::Path;

use crate::output::{print_json, OutputFormat};

#[derive(Subcommand)]
pub enum DeviceCmd {
    /// Initialize device identity (generates Ed25519 keypair if not present)
    Init,
    /// Show the current device identity (peer ID)
    Show,
    /// Add a peer device to the registry
    Pair {
        /// Peer ID of the device to pair with (base32-encoded public key)
        peer_id: String,
        /// Optional human-readable name for this device
        #[arg(long)]
        name: Option<String>,
    },
    /// List all known peer devices
    List,
    /// Remove a peer device from the registry
    Unpair {
        /// Peer ID to remove
        peer_id: String,
    },
}

pub fn run(cmd: DeviceCmd, vault_path: &Path, fmt: OutputFormat) -> anyhow::Result<()> {
    let agentic_dir = vault_path.join(".agentic");
    std::fs::create_dir_all(&agentic_dir)?;

    match cmd {
        DeviceCmd::Init => {
            let identity = DeviceIdentity::init_or_load(&agentic_dir)?;
            match fmt {
                OutputFormat::Json => {
                    print_json(&serde_json::json!({
                        "peer_id": identity.peer_id,
                        "status": "initialized"
                    }));
                }
                OutputFormat::Human => {
                    println!("Device initialized.");
                    println!("Peer ID: {}", identity.peer_id);
                }
            }
        }

        DeviceCmd::Show => {
            let identity = DeviceIdentity::init_or_load(&agentic_dir)?;
            match fmt {
                OutputFormat::Json => {
                    print_json(&serde_json::json!({
                        "peer_id": identity.peer_id
                    }));
                }
                OutputFormat::Human => {
                    println!("Peer ID: {}", identity.peer_id);
                }
            }
        }

        DeviceCmd::Pair { peer_id, name } => {
            let registry_path = agentic_dir.join("devices.json");
            let mut registry = DeviceRegistry::load(&registry_path)?;
            registry.add_device(peer_id.clone(), name.clone());
            registry.save()?;
            match fmt {
                OutputFormat::Json => {
                    print_json(&serde_json::json!({
                        "peer_id": peer_id,
                        "name": name,
                        "status": "paired"
                    }));
                }
                OutputFormat::Human => {
                    println!("Paired with device: {}", peer_id);
                    if let Some(n) = &name {
                        println!("Name: {n}");
                    }
                }
            }
        }

        DeviceCmd::List => {
            let registry_path = agentic_dir.join("devices.json");
            let registry = DeviceRegistry::load(&registry_path)?;
            let devices = registry.list();
            match fmt {
                OutputFormat::Json => {
                    let json: Vec<_> = devices
                        .iter()
                        .map(|d| {
                            serde_json::json!({
                                "peer_id": d.peer_id,
                                "name": d.name,
                                "last_sync": d.last_sync
                            })
                        })
                        .collect();
                    print_json(&json);
                }
                OutputFormat::Human => {
                    if devices.is_empty() {
                        println!("No paired devices.");
                    } else {
                        println!("{} paired device(s):", devices.len());
                        for d in devices {
                            let name = d.name.as_deref().unwrap_or("<unnamed>");
                            let last_sync = d
                                .last_sync
                                .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                                .unwrap_or_else(|| "never".to_string());
                            println!("  {} ({}) — last sync: {}", d.peer_id, name, last_sync);
                        }
                    }
                }
            }
        }

        DeviceCmd::Unpair { peer_id } => {
            let registry_path = agentic_dir.join("devices.json");
            let mut registry = DeviceRegistry::load(&registry_path)?;
            registry.remove_device(&peer_id);
            registry.save()?;
            match fmt {
                OutputFormat::Json => {
                    print_json(&serde_json::json!({
                        "peer_id": peer_id,
                        "status": "unpaired"
                    }));
                }
                OutputFormat::Human => {
                    println!("Unpaired device: {peer_id}");
                }
            }
        }
    }

    Ok(())
}
