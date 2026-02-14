pub mod condition;
pub mod context;
pub mod dag_executor;
pub mod error_policy;
pub mod executor;
pub mod migration;
pub mod pipeline;
pub mod trigger;

pub use context::StageContext;
pub use dag_executor::DagExecutor;
pub use error_policy::StageError;
pub use executor::{AgentHandler, PipelineResult, StageExecutor};
pub use pipeline::PipelineConfig;
pub use trigger::{FileEvent, FileEventType, TriggerConfig, TriggerType};

use agentic_note_core::error::{AgenticError, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Top-level facade for the AgentSpace engine.
///
/// Uses `DagExecutor` for v2 pipelines (explicit `depends_on`) and
/// automatically migrates v1 (sequential) pipelines on first run.
pub struct AgentSpace {
    pipelines: Vec<PipelineConfig>,
    executor: DagExecutor,
    vault_path: PathBuf,
}

impl AgentSpace {
    /// Create a new `AgentSpace`, loading all `*.toml` pipelines from
    /// `pipelines_dir`. Returns `Ok` even when the directory is empty.
    pub fn new(vault_path: PathBuf, pipelines_dir: PathBuf) -> Result<Self> {
        let pipelines = PipelineConfig::load_all(&pipelines_dir)?;
        tracing::info!(
            "AgentSpace loaded {} pipeline(s) from {}",
            pipelines.len(),
            pipelines_dir.display()
        );
        Ok(Self {
            pipelines,
            executor: DagExecutor::new(),
            vault_path,
        })
    }

    /// Register an agent handler so pipeline stages can dispatch to it.
    pub fn register_agent(&mut self, handler: Arc<dyn AgentHandler>) {
        self.executor.register(handler);
    }

    /// Execute the named pipeline against the provided context.
    ///
    /// Returns `Err` only when the pipeline name is unknown; individual
    /// stage failures are captured in `PipelineResult::skipped`.
    pub async fn run_pipeline(&self, name: &str, ctx: &mut StageContext) -> Result<PipelineResult> {
        let pipeline = self
            .pipelines
            .iter()
            .find(|p| p.name == name && p.enabled)
            .ok_or_else(|| {
                AgenticError::NotFound(format!("pipeline '{name}' not found or disabled"))
            })?;

        self.executor.run_pipeline(pipeline, ctx).await
    }

    /// All loaded pipeline configs (enabled and disabled).
    pub fn list_pipelines(&self) -> &[PipelineConfig] {
        &self.pipelines
    }

    pub fn vault_path(&self) -> &PathBuf {
        &self.vault_path
    }
}
