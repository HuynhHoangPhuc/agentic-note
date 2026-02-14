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
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub indexer: IndexerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub llm_cache: LlmCacheConfig,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub default_conflict_policy: ConflictPolicy,
    #[serde(default)]
    pub conflict_overrides: HashMap<String, ConflictPolicy>,
    #[serde(default)]
    pub device_name: Option<String>,
    /// Enable zstd compression for sync blob transfer.
    #[serde(default = "default_true")]
    pub compression_enabled: bool,
    /// zstd compression level 1-22.
    #[serde(default = "default_compression_level")]
    pub compression_level: i32,
    /// End-to-end encryption settings.
    #[serde(default)]
    pub encryption: EncryptionConfig,
}

fn default_true() -> bool {
    true
}

fn default_compression_level() -> i32 {
    3
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            default_conflict_policy: ConflictPolicy::default(),
            conflict_overrides: HashMap::new(),
            device_name: None,
            compression_enabled: true,
            compression_level: 3,
            encryption: EncryptionConfig::default(),
        }
    }
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
    /// WASM runtime configuration.
    #[serde(default)]
    pub wasm: WasmConfig,
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
            wasm: WasmConfig::default(),
        }
    }
}

/// Pipeline scheduler configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Enable the pipeline scheduler.
    #[serde(default)]
    pub enabled: bool,
    /// Default cron expression for scheduled pipelines.
    #[serde(default)]
    pub default_cron: Option<String>,
    /// FS watch debounce in milliseconds.
    #[serde(default = "default_watch_debounce_ms")]
    pub watch_debounce_ms: u64,
}

fn default_watch_debounce_ms() -> u64 {
    500
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_cron: None,
            watch_debounce_ms: 500,
        }
    }
}

/// Metrics and observability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection.
    #[serde(default)]
    pub enabled: bool,
    /// Prometheus exporter port (localhost only).
    #[serde(default = "default_prometheus_port")]
    pub prometheus_port: u16,
}

fn default_prometheus_port() -> u16 {
    9091
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            prometheus_port: 9091,
        }
    }
}

/// Background indexer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// Enable background indexing on file changes.
    #[serde(default = "default_true")]
    pub background: bool,
    /// Debounce window in milliseconds before indexing.
    #[serde(default = "default_indexer_debounce_ms")]
    pub debounce_ms: u64,
    /// Max files per index batch.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_indexer_debounce_ms() -> u64 {
    200
}

fn default_batch_size() -> usize {
    50
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            background: true,
            debounce_ms: 200,
            batch_size: 50,
        }
    }
}

/// Database backend configuration (SQLite default, Postgres optional).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Backend type: "sqlite" or "postgres".
    #[serde(default = "default_backend")]
    pub backend: String,
    /// Connection URL for Postgres (ignored for SQLite).
    #[serde(default)]
    pub url: Option<String>,
    /// Connection pool size (Postgres only).
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

fn default_backend() -> String {
    "sqlite".into()
}

fn default_max_connections() -> u32 {
    5
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            url: None,
            max_connections: 5,
        }
    }
}

/// LLM response cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCacheConfig {
    /// Enable LLM response caching.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Cache TTL in seconds (default 24h).
    #[serde(default = "default_cache_ttl")]
    pub ttl_secs: u64,
    /// Max cache entries before pruning.
    #[serde(default = "default_max_cache_entries")]
    pub max_entries: usize,
}

fn default_cache_ttl() -> u64 {
    86400
}

fn default_max_cache_entries() -> usize {
    10000
}

impl Default for LlmCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: 86400,
            max_entries: 10000,
        }
    }
}

/// Encryption configuration for P2P sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Enable end-to-end encryption for sync.
    #[serde(default)]
    pub enabled: bool,
    /// Reject unencrypted peers when true.
    #[serde(default)]
    pub require_encryption: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            require_encryption: false,
        }
    }
}

/// A registered vault entry for multi-vault sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub path: PathBuf,
    pub name: String,
    #[serde(default = "default_true")]
    pub sync_enabled: bool,
    #[serde(default)]
    pub default_peers: Vec<String>,
}

/// WASM plugin runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    /// Default memory limit in MB per plugin.
    #[serde(default = "default_wasm_memory_limit")]
    pub default_memory_limit_mb: u32,
    /// Default fuel limit per plugin execution.
    #[serde(default = "default_fuel_limit")]
    pub default_fuel_limit: u64,
    /// Cache compiled WASM modules.
    #[serde(default = "default_true")]
    pub cache_compiled: bool,
}

fn default_wasm_memory_limit() -> u32 {
    64
}

fn default_fuel_limit() -> u64 {
    1_000_000
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            default_memory_limit_mb: 64,
            default_fuel_limit: 1_000_000,
            cache_compiled: true,
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
        // v0.3 defaults applied when sections are absent
        assert!(config.indexer.background);
        assert_eq!(config.indexer.debounce_ms, 200);
        assert_eq!(config.indexer.batch_size, 50);
        assert!(!config.scheduler.enabled);
        assert_eq!(config.scheduler.watch_debounce_ms, 500);
        assert!(!config.metrics.enabled);
        assert_eq!(config.metrics.prometheus_port, 9091);
        assert!(config.sync.compression_enabled);
        assert_eq!(config.sync.compression_level, 3);
    }

    #[test]
    fn test_deserialize_config_with_v030_sections() {
        let toml_str = r#"
[vault]
path = "/home/user/notes"

[scheduler]
enabled = true
watch_debounce_ms = 1000

[metrics]
enabled = true
prometheus_port = 8080

[indexer]
background = false
debounce_ms = 500
batch_size = 100

[sync]
compression_enabled = false
compression_level = 6
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.scheduler.enabled);
        assert_eq!(config.scheduler.watch_debounce_ms, 1000);
        assert!(config.metrics.enabled);
        assert_eq!(config.metrics.prometheus_port, 8080);
        assert!(!config.indexer.background);
        assert_eq!(config.indexer.debounce_ms, 500);
        assert_eq!(config.indexer.batch_size, 100);
        assert!(!config.sync.compression_enabled);
        assert_eq!(config.sync.compression_level, 6);
    }
}
