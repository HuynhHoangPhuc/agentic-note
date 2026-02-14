/// Sync CLI commands: now, status.
///
/// Named sync_cmd.rs to avoid collision with Rust's `sync` built-in module.
use agentic_note_core::types::ConflictPolicy;
use agentic_note_sync::{DeviceRegistry, SyncEngine};
use clap::Subcommand;
use std::path::Path;

use crate::output::{print_json, OutputFormat};

#[derive(Subcommand)]
pub enum SyncCmd {
    /// Sync with a peer device now
    Now {
        /// Peer ID to sync with (base32-encoded public key)
        #[arg(long)]
        peer: String,
        /// Conflict resolution policy
        #[arg(long, default_value = "newest-wins")]
        policy: String,
    },
    /// Show sync status (last sync times, pending conflicts)
    Status,
}

pub async fn run(cmd: SyncCmd, vault_path: &Path, fmt: OutputFormat) -> anyhow::Result<()> {
    match cmd {
        SyncCmd::Now { peer, policy } => {
            let conflict_policy = parse_policy(&policy)?;

            let mut engine = SyncEngine::new_with_iroh(vault_path).await?;
            let result = engine.sync_with_peer(&peer, &conflict_policy).await?;

            match fmt {
                OutputFormat::Json => {
                    print_json(&serde_json::json!({
                        "peer": peer,
                        "merged": result.merged,
                        "auto_resolved": result.auto_resolved,
                        "conflicts": result.conflicts,
                        "snapshot_id": result.snapshot_id,
                    }));
                }
                OutputFormat::Human => {
                    println!("Sync complete with peer: {peer}");
                    println!("  Merged:        {}", result.merged);
                    println!("  Auto-resolved: {}", result.auto_resolved);
                    if result.conflicts > 0 {
                        println!(
                            "  Conflicts:     {} (see .agentic/conflicts/)",
                            result.conflicts
                        );
                    } else {
                        println!("  Conflicts:     0");
                    }
                    println!("  Snapshot:      {}", result.snapshot_id);
                }
            }
        }

        SyncCmd::Status => {
            let agentic_dir = vault_path.join(".agentic");
            let registry_path = agentic_dir.join("devices.json");
            let registry = DeviceRegistry::load(&registry_path)?;

            // Check for pending conflict files
            let conflict_dir = vault_path.join(".agentic").join("conflicts");
            let pending_conflicts: Vec<String> = if conflict_dir.exists() {
                std::fs::read_dir(&conflict_dir)
                    .map(|entries| {
                        entries
                            .filter_map(|e| {
                                e.ok()
                                    .and_then(|e| e.file_name().to_str().map(|s| s.to_string()))
                            })
                            .filter(|name| name.ends_with(".conflict"))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                vec![]
            };

            let devices = registry.list();

            match fmt {
                OutputFormat::Json => {
                    let device_list: Vec<_> = devices
                        .iter()
                        .map(|d| {
                            serde_json::json!({
                                "peer_id": d.peer_id,
                                "name": d.name,
                                "last_sync": d.last_sync
                            })
                        })
                        .collect();
                    print_json(&serde_json::json!({
                        "devices": device_list,
                        "pending_conflicts": pending_conflicts,
                    }));
                }
                OutputFormat::Human => {
                    println!("Sync Status");
                    println!("===========");
                    if devices.is_empty() {
                        println!("No paired devices. Use `device pair <PEER_ID>` to add one.");
                    } else {
                        println!("Paired devices ({}):", devices.len());
                        for d in devices {
                            let name = d.name.as_deref().unwrap_or("<unnamed>");
                            let last_sync = d
                                .last_sync
                                .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                                .unwrap_or_else(|| "never".to_string());
                            println!("  {} ({}) — last sync: {}", d.peer_id, name, last_sync);
                        }
                    }
                    if pending_conflicts.is_empty() {
                        println!("No pending conflicts.");
                    } else {
                        println!("Pending conflicts ({}):", pending_conflicts.len());
                        for f in &pending_conflicts {
                            println!("  .agentic/conflicts/{f}");
                        }
                        println!("Resolve conflicts then delete the .conflict files.");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Parse a policy string into ConflictPolicy.
fn parse_policy(s: &str) -> anyhow::Result<ConflictPolicy> {
    match s {
        "newest-wins" | "newest_wins" => Ok(ConflictPolicy::NewestWins),
        "longest-wins" | "longest_wins" => Ok(ConflictPolicy::LongestWins),
        "merge-both" | "merge_both" => Ok(ConflictPolicy::MergeBoth),
        "manual" => Ok(ConflictPolicy::Manual),
        other => Err(anyhow::anyhow!(
            "unknown conflict policy '{other}'. Valid: newest-wins, longest-wins, merge-both, manual"
        )),
    }
}
