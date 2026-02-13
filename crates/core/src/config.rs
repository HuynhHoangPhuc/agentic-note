use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{AgenticError, Result};

/// Top-level application configuration from `.agentic/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub vault: VaultConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub agent: AgentConfig,
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
pub enum TrustLevel {
    Manual,
    Review,
    Auto,
}

impl Default for TrustLevel {
    fn default() -> Self {
        Self::Review
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub default_trust: TrustLevel,
    #[serde(default = "default_max_pipelines")]
    pub max_concurrent_pipelines: usize,
}

fn default_max_pipelines() -> usize {
    1
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default_trust: TrustLevel::default(),
            max_concurrent_pipelines: default_max_pipelines(),
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
