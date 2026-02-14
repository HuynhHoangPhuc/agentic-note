use agentic_note_core::types::ErrorPolicy;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

use super::context::StageContext;
use super::executor::AgentHandler;
use super::pipeline::StageConfig;

/// Structured record of a stage failure captured in `PipelineResult`.
#[derive(Debug, Clone)]
pub struct StageError {
    pub stage_name: String,
    pub agent: String,
    pub attempts: u32,
    pub error: String,
    pub policy_applied: ErrorPolicy,
}

/// Outcome returned by `execute_with_policy`.
///
/// - `Ok(Some(value))` → stage succeeded, store output.
/// - `Ok(None)` → stage was skipped (Skip or Retry exhausted or Fallback both failed).
/// - `Err(StageError)` → Abort policy triggered; caller should stop the pipeline.
pub async fn execute_with_policy(
    handler: &dyn AgentHandler,
    ctx: &mut StageContext,
    stage: &StageConfig,
    handlers: &HashMap<String, Arc<dyn AgentHandler>>,
) -> Result<Option<Value>, StageError> {
    match &stage.on_error {
        ErrorPolicy::Skip => match handler.execute(ctx, &stage.config).await {
            Ok(v) => Ok(Some(v)),
            Err(e) => {
                warn!(
                    "stage '{}' (agent '{}') failed [skip]: {e}",
                    stage.name, stage.agent
                );
                Ok(None)
            }
        },

        ErrorPolicy::Retry => match retry_with_backoff(handler, ctx, stage).await {
            Ok(v) => Ok(Some(v)),
            Err(last_error) => {
                warn!(
                    "stage '{}' (agent '{}') exhausted {} retries: {last_error}",
                    stage.name, stage.agent, stage.retry_max
                );
                Ok(None)
            }
        },

        ErrorPolicy::Abort => match handler.execute(ctx, &stage.config).await {
            Ok(v) => Ok(Some(v)),
            Err(e) => {
                let err = StageError {
                    stage_name: stage.name.clone(),
                    agent: stage.agent.clone(),
                    attempts: 1,
                    error: e.to_string(),
                    policy_applied: ErrorPolicy::Abort,
                };
                Err(err)
            }
        },

        ErrorPolicy::Fallback => match handler.execute(ctx, &stage.config).await {
            Ok(v) => Ok(Some(v)),
            Err(primary_err) => {
                warn!(
                    "stage '{}' (agent '{}') failed [fallback]: {primary_err}",
                    stage.name, stage.agent
                );
                try_fallback(stage, ctx, handlers, primary_err.to_string()).await
            }
        },
    }
}

/// Retry the handler up to `stage.retry_max` times with exponential backoff.
///
/// Backoff per attempt: `min(retry_backoff_ms * 2^attempt, 30_000)` ms.
/// Returns `Ok(Value)` on first success, `Err(last_error_string)` after exhaustion.
async fn retry_with_backoff(
    handler: &dyn AgentHandler,
    ctx: &mut StageContext,
    stage: &StageConfig,
) -> Result<Value, String> {
    let max = stage.retry_max.max(1);
    let base_ms = stage.retry_backoff_ms;
    let mut last_error = String::new();

    for attempt in 0..max {
        match handler.execute(ctx, &stage.config).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_error = e.to_string();
                if attempt + 1 < max {
                    // Exponential backoff, capped at 30 seconds.
                    let delay_ms = base_ms
                        .saturating_mul(2u64.saturating_pow(attempt))
                        .min(30_000);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    Err(last_error)
}

/// Try the fallback agent; on failure, treat as Skip (return Ok(None)).
async fn try_fallback(
    stage: &StageConfig,
    ctx: &mut StageContext,
    handlers: &HashMap<String, Arc<dyn AgentHandler>>,
    primary_error: String,
) -> Result<Option<Value>, StageError> {
    let fallback_id = match &stage.fallback_agent {
        Some(id) => id,
        None => {
            warn!(
                "stage '{}': on_error=fallback but no fallback_agent configured; skipping",
                stage.name
            );
            return Ok(None);
        }
    };

    match handlers.get(fallback_id.as_str()) {
        None => {
            warn!(
                "stage '{}': fallback agent '{}' not registered; skipping",
                stage.name, fallback_id
            );
            Ok(None)
        }
        Some(fb_handler) => match fb_handler.execute(ctx, &stage.config).await {
            Ok(v) => Ok(Some(v)),
            Err(fb_err) => {
                warn!(
                    "stage '{}': fallback agent '{}' also failed: {fb_err}; \
                     primary error was: {primary_error}",
                    stage.name, fallback_id
                );
                Ok(None)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::context::StageContext;
    use agentic_note_core::error::{AgenticError, Result as CoreResult};
    use agentic_note_core::types::{FrontMatter, NoteId, NoteStatus, ParaCategory};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    // ─── helpers ────────────────────────────────────────────────────────────

    fn make_ctx() -> StageContext {
        let fm = FrontMatter {
            id: NoteId::new(),
            title: "T".into(),
            created: Utc::now(),
            modified: Utc::now(),
            tags: vec![],
            para: ParaCategory::Inbox,
            links: vec![],
            status: NoteStatus::Seed,
        };
        StageContext {
            note_id: fm.id,
            note_content: "hi".into(),
            frontmatter: fm,
            outputs: Default::default(),
            vault_path: PathBuf::from("/tmp"),
        }
    }

    fn make_stage(on_error: ErrorPolicy) -> StageConfig {
        StageConfig {
            name: "test-stage".into(),
            agent: "primary".into(),
            config: toml::Value::Table(Default::default()),
            output: "out".into(),
            depends_on: vec![],
            condition: None,
            on_error,
            retry_max: 3,
            retry_backoff_ms: 0, // no sleep in tests
            fallback_agent: None,
        }
    }

    // ─── agent fixtures ─────────────────────────────────────────────────────

    struct OkAgent;

    #[async_trait]
    impl AgentHandler for OkAgent {
        fn agent_id(&self) -> &str {
            "primary"
        }
        async fn execute(&self, _ctx: &mut StageContext, _cfg: &toml::Value) -> CoreResult<Value> {
            Ok(serde_json::json!({ "ok": true }))
        }
    }

    struct FailAgent;

    #[async_trait]
    impl AgentHandler for FailAgent {
        fn agent_id(&self) -> &str {
            "primary"
        }
        async fn execute(&self, _ctx: &mut StageContext, _cfg: &toml::Value) -> CoreResult<Value> {
            Err(AgenticError::Parse("boom".into()))
        }
    }

    /// Fails the first `fail_times` calls, then succeeds.
    struct FlakyAgent {
        fail_times: u32,
        calls: Arc<AtomicU32>,
    }

    impl FlakyAgent {
        fn new(fail_times: u32) -> Self {
            Self {
                fail_times,
                calls: Arc::new(AtomicU32::new(0)),
            }
        }
    }

    #[async_trait]
    impl AgentHandler for FlakyAgent {
        fn agent_id(&self) -> &str {
            "primary"
        }
        async fn execute(&self, _ctx: &mut StageContext, _cfg: &toml::Value) -> CoreResult<Value> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n < self.fail_times {
                Err(AgenticError::Parse(format!("fail attempt {n}")))
            } else {
                Ok(serde_json::json!({ "attempt": n }))
            }
        }
    }

    struct FallbackAgent;

    #[async_trait]
    impl AgentHandler for FallbackAgent {
        fn agent_id(&self) -> &str {
            "fallback"
        }
        async fn execute(&self, _ctx: &mut StageContext, _cfg: &toml::Value) -> CoreResult<Value> {
            Ok(serde_json::json!({ "from": "fallback" }))
        }
    }

    // ─── tests ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn skip_policy_returns_none_on_failure() {
        let stage = make_stage(ErrorPolicy::Skip);
        let mut ctx = make_ctx();
        let handlers: HashMap<String, Arc<dyn AgentHandler>> = HashMap::new();
        let result = execute_with_policy(&FailAgent, &mut ctx, &stage, &handlers)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn skip_policy_returns_value_on_success() {
        let stage = make_stage(ErrorPolicy::Skip);
        let mut ctx = make_ctx();
        let handlers: HashMap<String, Arc<dyn AgentHandler>> = HashMap::new();
        let result = execute_with_policy(&OkAgent, &mut ctx, &stage, &handlers)
            .await
            .unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn retry_succeeds_after_failures() {
        // Fails twice, succeeds on 3rd attempt (attempt index 2).
        let flaky = FlakyAgent::new(2);
        let stage = make_stage(ErrorPolicy::Retry);
        let mut ctx = make_ctx();
        let handlers: HashMap<String, Arc<dyn AgentHandler>> = HashMap::new();
        let result = execute_with_policy(&flaky, &mut ctx, &stage, &handlers)
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["attempt"], 2);
    }

    #[tokio::test]
    async fn retry_exhausted_returns_none() {
        // Always fails; retry_max = 3.
        let stage = make_stage(ErrorPolicy::Retry);
        let mut ctx = make_ctx();
        let handlers: HashMap<String, Arc<dyn AgentHandler>> = HashMap::new();
        let result = execute_with_policy(&FailAgent, &mut ctx, &stage, &handlers)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn abort_policy_returns_err() {
        let stage = make_stage(ErrorPolicy::Abort);
        let mut ctx = make_ctx();
        let handlers: HashMap<String, Arc<dyn AgentHandler>> = HashMap::new();
        let result = execute_with_policy(&FailAgent, &mut ctx, &stage, &handlers).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.stage_name, "test-stage");
        assert_eq!(err.policy_applied, ErrorPolicy::Abort);
        assert_eq!(err.attempts, 1);
    }

    #[tokio::test]
    async fn fallback_agent_succeeds_when_primary_fails() {
        let mut stage = make_stage(ErrorPolicy::Fallback);
        stage.fallback_agent = Some("fallback".into());

        let mut ctx = make_ctx();
        let mut handlers: HashMap<String, Arc<dyn AgentHandler>> = HashMap::new();
        handlers.insert("fallback".into(), Arc::new(FallbackAgent));

        let result = execute_with_policy(&FailAgent, &mut ctx, &stage, &handlers)
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["from"], "fallback");
    }

    #[tokio::test]
    async fn fallback_both_fail_returns_none() {
        struct AlwaysFailFallback;
        #[async_trait]
        impl AgentHandler for AlwaysFailFallback {
            fn agent_id(&self) -> &str {
                "fallback"
            }
            async fn execute(
                &self,
                _ctx: &mut StageContext,
                _cfg: &toml::Value,
            ) -> CoreResult<Value> {
                Err(AgenticError::Parse("fallback also fails".into()))
            }
        }

        let mut stage = make_stage(ErrorPolicy::Fallback);
        stage.fallback_agent = Some("fallback".into());

        let mut ctx = make_ctx();
        let mut handlers: HashMap<String, Arc<dyn AgentHandler>> = HashMap::new();
        handlers.insert("fallback".into(), Arc::new(AlwaysFailFallback));

        let result = execute_with_policy(&FailAgent, &mut ctx, &stage, &handlers)
            .await
            .unwrap();
        assert!(result.is_none());
    }
}
