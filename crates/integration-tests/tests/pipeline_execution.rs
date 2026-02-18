use agentic_note_agent::engine::pipeline::StageConfig;
use agentic_note_agent::engine::{
    AgentHandler, PipelineConfig, StageContext, StageExecutor, TriggerConfig, TriggerType,
};
use agentic_note_core::types::ErrorPolicy;
use agentic_note_test_utils::TempVault;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

struct EchoAgent;

#[async_trait]
impl AgentHandler for EchoAgent {
    fn agent_id(&self) -> &str {
        "echo"
    }

    async fn execute(
        &self,
        _ctx: &mut StageContext,
        _config: &toml::Value,
    ) -> agentic_note_core::Result<Value> {
        Ok(serde_json::json!({"summary": "ok"}))
    }
}

#[tokio::test]
async fn pipeline_execution_runs_dag() -> agentic_note_core::Result<()> {
    let vault = TempVault::new()?;
    let note = agentic_note_vault::Note::create(
        vault.path(),
        "Pipeline",
        agentic_note_core::types::ParaCategory::Inbox,
        "hello",
        vec![],
    )?;

    let pipeline = PipelineConfig {
        name: "test".to_string(),
        description: "pipeline test".to_string(),
        enabled: true,
        trigger: TriggerConfig {
            trigger_type: TriggerType::Manual,
            path_filter: None,
            debounce_ms: 0,
            cron: None,
            watch_path: None,
        },
        stages: vec![StageConfig {
            name: "summarise".to_string(),
            agent: "echo".to_string(),
            config: toml::Value::Table(Default::default()),
            output: "summary".to_string(),
            depends_on: vec![],
            condition: None,
            on_error: ErrorPolicy::Skip,
            retry_max: 1,
            retry_backoff_ms: 10,
            fallback_agent: None,
        }],
        schema_version: 2,
        default_on_error: ErrorPolicy::Skip,
    };

    let mut executor = StageExecutor::new();
    executor.register(Arc::new(EchoAgent));

    let mut ctx = StageContext::from_note(&note, vault.path());
    let result = executor.run_pipeline(&pipeline, &mut ctx).await?;
    assert_eq!(result.stages_completed, 1);
    Ok(())
}
