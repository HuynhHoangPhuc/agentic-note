use agentic_note_core::error::{AgenticError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::trigger::TriggerConfig;

/// Top-level pipeline configuration loaded from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub name: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub trigger: TriggerConfig,
    pub stages: Vec<StageConfig>,
}

/// Configuration for a single stage within a pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageConfig {
    pub name: String,
    /// Identifies the `AgentHandler` to dispatch to.
    pub agent: String,
    /// Arbitrary agent-specific config forwarded verbatim.
    #[serde(default = "default_toml_table")]
    pub config: toml::Value,
    /// Key under which this stage's output is stored in `StageContext`.
    pub output: String,
}

fn default_true() -> bool {
    true
}

fn default_toml_table() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

impl PipelineConfig {
    /// Load a single pipeline from a TOML file.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            AgenticError::NotFound(format!("{}: {e}", path.display()))
        })?;
        toml::from_str(&raw)
            .map_err(|e| AgenticError::Parse(format!("pipeline {}: {e}", path.display())))
    }

    /// Load all `*.toml` pipelines from a directory (non-recursive).
    /// Files that fail to parse are skipped with a warning log.
    pub fn load_all(dir: &Path) -> Result<Vec<Self>> {
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut configs = Vec::new();
        let entries = std::fs::read_dir(dir)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match Self::load(&path) {
                Ok(cfg) => configs.push(cfg),
                Err(e) => {
                    tracing::warn!("skipping pipeline {:?}: {e}", path);
                }
            }
        }
        Ok(configs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trigger::TriggerType;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const SAMPLE_TOML: &str = r#"
name = "summarise"
description = "Summarise notes on creation"
enabled = true

[trigger]
trigger_type = "file_created"
path_filter = "projects/**"
debounce_ms = 300

[[stages]]
name = "extract"
agent = "keyword-extractor"
output = "keywords"

[[stages]]
name = "summarise"
agent = "summariser"
output = "summary"
"#;

    #[test]
    fn parse_pipeline_toml() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(SAMPLE_TOML.as_bytes()).unwrap();

        let cfg = PipelineConfig::load(f.path()).unwrap();
        assert_eq!(cfg.name, "summarise");
        assert!(cfg.enabled);
        assert_eq!(cfg.trigger.trigger_type, TriggerType::FileCreated);
        assert_eq!(cfg.trigger.path_filter.as_deref(), Some("projects/**"));
        assert_eq!(cfg.trigger.debounce_ms, 300);
        assert_eq!(cfg.stages.len(), 2);
        assert_eq!(cfg.stages[0].agent, "keyword-extractor");
        assert_eq!(cfg.stages[1].output, "summary");
    }

    #[test]
    fn load_all_returns_empty_for_missing_dir() {
        let result = PipelineConfig::load_all(Path::new("/nonexistent/dir")).unwrap();
        assert!(result.is_empty());
    }
}
