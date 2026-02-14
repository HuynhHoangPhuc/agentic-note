use agentic_note_core::error::Result;
use std::path::{Path, PathBuf};

use super::manifest::PluginManifest;

/// Scan a plugins directory for valid plugin manifests.
/// Returns (manifest, plugin_dir) pairs. Invalid manifests are skipped with a warning.
pub fn discover_plugins(plugins_dir: &Path) -> Result<Vec<(PluginManifest, PathBuf)>> {
    let mut plugins = Vec::new();

    if !plugins_dir.exists() {
        return Ok(plugins);
    }

    let entries = std::fs::read_dir(plugins_dir)?;
    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }

        let manifest_path = plugin_dir.join("plugin.toml");
        if !manifest_path.exists() {
            continue;
        }

        match PluginManifest::load(&manifest_path) {
            Ok(manifest) => {
                tracing::info!(
                    "discovered plugin '{}' v{}",
                    manifest.name,
                    manifest.version
                );
                plugins.push((manifest, plugin_dir));
            }
            Err(e) => {
                tracing::warn!("skipping plugin at {}: {e}", plugin_dir.display());
            }
        }
    }

    Ok(plugins)
}
