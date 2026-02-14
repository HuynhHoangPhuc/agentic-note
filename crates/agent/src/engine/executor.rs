use agentic_note_core::error::Result;
use agentic_note_core::types::ErrorPolicy;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use super::context::StageContext;
use super::error_policy::{self, StageError};
use super::pipeline::PipelineConfig;

/// Implement this trait to add a new agent capability to the engine.
#[async_trait]
pub trait AgentHandler: Send + Sync {
    /// Unique identifier matching `StageConfig::agent`.
    fn agent_id(&self) -> &str;

    /// Run this agent on the current context.
    /// Returns a JSON value stored under the stage's `output` key.
    async fn execute(&self, ctx: &mut StageContext, config: &toml::Value) -> Result<Value>;
}

/// Summary produced after a pipeline finishes.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub stages_completed: usize,
    pub total: usize,
    pub outputs: HashMap<String, Value>,
    /// Stage names that were skipped due to errors or missing handlers.
    pub skipped: Vec<String>,
    /// Human-readable warnings accumulated during execution.
    pub warnings: Vec<String>,
    /// Structured error records from policy-driven stage failures.
    pub errors: Vec<StageError>,
}

/// Dispatches pipeline stages to registered `AgentHandler` implementations.
pub struct StageExecutor {
    handlers: HashMap<String, Arc<dyn AgentHandler>>,
}

impl StageExecutor {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register an agent handler. Overwrites any previous handler with
    /// the same `agent_id`.
    pub fn register(&mut self, handler: Arc<dyn AgentHandler>) {
        self.handlers
            .insert(handler.agent_id().to_string(), handler);
    }

    /// Execute all stages in `pipeline` sequentially, applying per-stage error policies.
    ///
    /// - Skip (default): log warning and continue.
    /// - Retry: exponential backoff up to retry_max; on exhaustion, skip.
    /// - Abort: first failure stops the pipeline immediately (partial result).
    /// - Fallback: try fallback_agent; on both failures, skip.
    pub async fn run_pipeline(
        &self,
        pipeline: &PipelineConfig,
        ctx: &mut StageContext,
    ) -> Result<PipelineResult> {
        let total = pipeline.stages.len();
        let mut stages_completed = 0usize;
        let mut outputs: HashMap<String, Value> = HashMap::new();
        let mut skipped: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut errors: Vec<StageError> = Vec::new();

        for stage in &pipeline.stages {
            // Resolve effective error policy: stage-level overrides pipeline default.
            // If the stage has the default (Skip) but pipeline sets something different,
            // use the pipeline default.
            let effective_policy = if stage.on_error == ErrorPolicy::default() {
                pipeline.default_on_error.clone()
            } else {
                stage.on_error.clone()
            };

            // Build a stage with the effective policy for dispatch.
            let mut effective_stage = stage.clone();
            effective_stage.on_error = effective_policy;

            match self.handlers.get(&stage.agent) {
                None => {
                    let msg = format!(
                        "pipeline '{}' stage '{}': no handler for agent '{}'",
                        pipeline.name, stage.name, stage.agent
                    );
                    tracing::warn!("{msg}");
                    warnings.push(msg);
                    skipped.push(stage.name.clone());
                }
                Some(handler) => {
                    match error_policy::execute_with_policy(
                        handler.as_ref(),
                        ctx,
                        &effective_stage,
                        &self.handlers,
                    )
                    .await
                    {
                        Ok(Some(value)) => {
                            ctx.set_output(&stage.output, value.clone());
                            outputs.insert(stage.output.clone(), value);
                            stages_completed += 1;
                        }
                        Ok(None) => {
                            let msg = format!(
                                "pipeline '{}' stage '{}': skipped (policy: {:?})",
                                pipeline.name, stage.name, effective_stage.on_error
                            );
                            tracing::warn!("{msg}");
                            warnings.push(msg);
                            skipped.push(stage.name.clone());
                        }
                        Err(stage_error) => {
                            // Abort policy: stop immediately.
                            let msg = format!(
                                "pipeline '{}' stage '{}': abort triggered: {}",
                                pipeline.name, stage.name, stage_error.error
                            );
                            tracing::warn!("{msg}");
                            warnings.push(msg);
                            if errors.len() < 100 {
                                errors.push(stage_error);
                            }
                            skipped.push(stage.name.clone());
                            break;
                        }
                    }
                }
            }
        }

        Ok(PipelineResult {
            stages_completed,
            total,
            outputs,
            skipped,
            warnings,
            errors,
        })
    }
}

impl Default for StageExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::context::StageContext;
    use crate::engine::pipeline::{PipelineConfig, StageConfig};
    use crate::engine::trigger::{TriggerConfig, TriggerType};
    use agentic_note_core::error::AgenticError;
    use agentic_note_core::types::{FrontMatter, NoteId, NoteStatus, ParaCategory};
    use chrono::Utc;
    use std::path::PathBuf;

    struct EchoAgent;

    #[async_trait]
    impl AgentHandler for EchoAgent {
        fn agent_id(&self) -> &str {
            "echo"
        }
        async fn execute(&self, ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
            Ok(serde_json::json!({ "echoed": ctx.note_content }))
        }
    }

    struct FailAgent;

    #[async_trait]
    impl AgentHandler for FailAgent {
        fn agent_id(&self) -> &str {
            "fail"
        }
        async fn execute(&self, _ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
            Err(AgenticError::Parse("intentional failure".into()))
        }
    }

    fn make_pipeline(stages: Vec<StageConfig>) -> PipelineConfig {
        PipelineConfig {
            name: "test".into(),
            description: "".into(),
            enabled: true,
            schema_version: 1,
            trigger: TriggerConfig {
                trigger_type: TriggerType::Manual,
                path_filter: None,
                debounce_ms: 0,
                cron: None,
                watch_path: None,
            },
            stages,
            default_on_error: Default::default(),
        }
    }

    fn make_ctx() -> StageContext {
        let fm = FrontMatter {
            id: NoteId::new(),
            title: "Test".into(),
            created: Utc::now(),
            modified: Utc::now(),
            tags: vec![],
            para: ParaCategory::Inbox,
            links: vec![],
            status: NoteStatus::Seed,
        };
        StageContext {
            note_id: fm.id,
            note_content: "hello world".into(),
            frontmatter: fm,
            outputs: Default::default(),
            vault_path: PathBuf::from("/tmp"),
        }
    }

    #[tokio::test]
    async fn successful_stage_stores_output() {
        let mut exec = StageExecutor::new();
        exec.register(Arc::new(EchoAgent));

        let pipeline = make_pipeline(vec![StageConfig {
            name: "echo-stage".into(),
            agent: "echo".into(),
            config: toml::Value::Table(Default::default()),
            output: "echo_out".into(),
            depends_on: vec![],
            condition: None,
            on_error: Default::default(),
            retry_max: 3,
            retry_backoff_ms: 1000,
            fallback_agent: None,
        }]);

        let mut ctx = make_ctx();
        let result = exec.run_pipeline(&pipeline, &mut ctx).await.unwrap();

        assert_eq!(result.stages_completed, 1);
        assert_eq!(result.total, 1);
        assert!(result.skipped.is_empty());
        assert_eq!(ctx.get_output("echo_out").unwrap()["echoed"], "hello world");
    }

    #[tokio::test]
    async fn failing_stage_is_skipped_not_fatal() {
        let mut exec = StageExecutor::new();
        exec.register(Arc::new(FailAgent));
        exec.register(Arc::new(EchoAgent));

        let pipeline = make_pipeline(vec![
            StageConfig {
                name: "will-fail".into(),
                agent: "fail".into(),
                config: toml::Value::Table(Default::default()),
                output: "fail_out".into(),
                depends_on: vec![],
                condition: None,
                on_error: Default::default(),
                retry_max: 3,
                retry_backoff_ms: 1000,
                fallback_agent: None,
            },
            StageConfig {
                name: "will-echo".into(),
                agent: "echo".into(),
                config: toml::Value::Table(Default::default()),
                output: "echo_out".into(),
                depends_on: vec![],
                condition: None,
                on_error: Default::default(),
                retry_max: 3,
                retry_backoff_ms: 1000,
                fallback_agent: None,
            },
        ]);

        let mut ctx = make_ctx();
        let result = exec.run_pipeline(&pipeline, &mut ctx).await.unwrap();

        assert_eq!(result.stages_completed, 1);
        assert_eq!(result.total, 2);
        assert_eq!(result.skipped, vec!["will-fail"]);
        assert!(!result.warnings.is_empty());
    }
}
