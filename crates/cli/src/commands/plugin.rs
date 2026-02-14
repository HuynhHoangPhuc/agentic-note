use crate::output::OutputFormat;
use agentic_note_agent::plugin::discover_plugins;
use anyhow::Result;
use std::path::Path;

/// List discovered plugins from the vault's plugins directory.
pub fn list(vault_path: &Path, fmt: OutputFormat) -> Result<()> {
    let plugins_dir = vault_path.join(".agentic").join("plugins");
    let plugins = discover_plugins(&plugins_dir)?;

    if plugins.is_empty() {
        match fmt {
            OutputFormat::Json => {
                println!("{}", serde_json::json!({"plugins": []}));
            }
            OutputFormat::Human => {
                println!("No plugins found in {}", plugins_dir.display());
            }
        }
        return Ok(());
    }

    match fmt {
        OutputFormat::Json => {
            let list: Vec<_> = plugins
                .iter()
                .map(|(m, dir)| {
                    serde_json::json!({
                        "name": m.name,
                        "version": m.version,
                        "description": m.description,
                        "path": dir.display().to_string(),
                    })
                })
                .collect();
            println!("{}", serde_json::json!({"plugins": list}));
        }
        OutputFormat::Human => {
            println!("Discovered {} plugin(s):", plugins.len());
            for (manifest, dir) in &plugins {
                println!(
                    "  {} v{} — {} ({})",
                    manifest.name,
                    manifest.version,
                    manifest.description,
                    dir.display()
                );
            }
        }
    }

    Ok(())
}
