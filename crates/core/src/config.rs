use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{AgenticError, Result};
use crate::types::{ConflictPolicy, ErrorPolicy};

/// Top-level application configuration from `.agentic/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub vault: VaultConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub embeddings: EmbeddingsConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub path: PathBuf,
    #[serde(default = "default_para_folders")]
    pub para_folders: Vec<String>,
}

fn default_para_folders() -> Vec<String> {
    vec![
        "inbox".into(),
        "projects".into(),
        "areas".into(),
        "resources".into(),
        "archives".into(),
        "zettelkasten".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

fn default_provider() -> String {
    "openai".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
}

/// Trust level for agent actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TrustLevel {
    Manual,
    #[default]
    Review,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub default_trust: TrustLevel,
    #[serde(default = "default_max_pipelines")]
    pub max_concurrent_pipelines: usize,
    #[serde(default)]
    pub default_on_error: ErrorPolicy,
}

fn default_max_pipelines() -> usize {
    1
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default_trust: TrustLevel::default(),
            max_concurrent_pipelines: default_max_pipelines(),
            default_on_error: ErrorPolicy::default(),
        }
    }
}

/// Sync configuration for P2P device synchronization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    #[serde(default)]
    pub default_conflict_policy: ConflictPolicy,
    #[serde(default)]
    pub conflict_overrides: HashMap<String, ConflictPolicy>,
    #[serde(default)]
    pub device_name: Option<String>,
}

/// Embeddings configuration for semantic search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_model_name")]
    pub model_name: String,
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,
}

fn default_model_name() -> String {
    "all-MiniLM-L6-v2".into()
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_name: default_model_name(),
            cache_dir: None,
        }
    }
}

/// Plugins configuration for custom agent plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_plugins_dir")]
    pub plugins_dir: PathBuf,
    #[serde(default = "default_timeout_secs")]
    pub default_timeout_secs: u64,
}

fn default_plugins_dir() -> PathBuf {
    PathBuf::from(".agentic/plugins")
}

fn default_timeout_secs() -> u64 {
    30
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            plugins_dir: default_plugins_dir(),
            default_timeout_secs: default_timeout_secs(),
        }
    }
}

impl AppConfig {
    /// Load config from `.agentic/config.toml` relative to vault path.
    /// Resolution order: explicit path > AGENTIC_NOTE_VAULT env > current dir.
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let config_path = Self::resolve_config_path(path)?;
        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            AgenticError::Config(format!("failed to read {}: {e}", config_path.display()))
        })?;
        toml::from_str(&content)
            .map_err(|e| AgenticError::Config(format!("invalid config TOML: {e}")))
    }

    /// Resolve vault root path: explicit > env > cwd.
    pub fn resolve_vault_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
        if let Some(p) = explicit {
            return Ok(p);
        }
        if let Ok(env_path) = std::env::var("AGENTIC_NOTE_VAULT") {
            return Ok(PathBuf::from(env_path));
        }
        std::env::current_dir().map_err(|e| AgenticError::Config(format!("cannot get cwd: {e}")))
    }

    fn resolve_config_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
        let vault = Self::resolve_vault_path(explicit)?;
        Ok(vault.join(".agentic").join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_config() {
        let toml_str = r#"
[vault]
path = "/home/user/notes"

[llm]
default_provider = "anthropic"

[llm.providers.anthropic]
api_key = "sk-test"
model = "claude-sonnet-4-5-20250929"

[agent]
default_trust = "review"
max_concurrent_pipelines = 2
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.vault.path, PathBuf::from("/home/user/notes"));
        assert_eq!(config.llm.default_provider, "anthropic");
        assert_eq!(config.agent.max_concurrent_pipelines, 2);
        assert_eq!(config.agent.default_trust, TrustLevel::Review);
    }
}
