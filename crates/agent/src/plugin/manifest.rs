use agentic_note_core::error::{AgenticError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Plugin metadata loaded from `plugin.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub executable: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    30
}

impl PluginManifest {
    /// Load manifest from a `plugin.toml` file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AgenticError::Plugin(format!("read {}: {e}", path.display())))?;
        toml::from_str(&content)
            .map_err(|e| AgenticError::Plugin(format!("parse {}: {e}", path.display())))
    }
}
