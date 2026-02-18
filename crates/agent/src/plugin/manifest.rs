//! Plugin manifest loaded from `plugin.toml`.
//!
//! Supports both subprocess and WASM runtimes.

use agentic_note_core::error::{AgenticError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Plugin runtime type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntime {
    /// WASM sandbox (default).
    Wasm,
    /// Legacy subprocess execution.
    Subprocess,
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self::Wasm
    }
}

/// Plugin metadata loaded from `plugin.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub executable: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Runtime type: "wasm" (default) or "subprocess".
    #[serde(default)]
    pub runtime: PluginRuntime,
    /// Memory limit in MB for WASM plugins (default 64).
    #[serde(default = "default_memory_limit")]
    pub memory_limit_mb: u32,
    /// Fuel limit for WASM execution (default 1M).
    #[serde(default = "default_fuel")]
    pub fuel_limit: u64,
}

fn default_timeout() -> u64 {
    30
}

fn default_memory_limit() -> u32 {
    64
}

fn default_fuel() -> u64 {
    1_000_000
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_runtime_is_wasm() {
        let toml_str = r#"
name = "test-plugin"
version = "0.1.0"
description = "A test"
executable = "plugin.wasm"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str)
            .expect("parse manifest");
        assert_eq!(manifest.runtime, PluginRuntime::Wasm);
        assert_eq!(manifest.memory_limit_mb, 64);
        assert_eq!(manifest.fuel_limit, 1_000_000);
    }

    #[test]
    fn subprocess_runtime_parsed() {
        let toml_str = r#"
name = "legacy"
version = "0.1.0"
description = "Legacy plugin"
executable = "run.sh"
runtime = "subprocess"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str)
            .expect("parse manifest");
        assert_eq!(manifest.runtime, PluginRuntime::Subprocess);
    }
}
