//! Merge assistant agent for LLM-powered conflict resolution.
//!
//! Tier 2 in the semantic merge pipeline: sends conflicting hunks to
//! the configured LLM provider for intelligent merge resolution.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, warn};

use crate::engine::{AgentHandler, StageContext};
use crate::llm::{ChatOpts, LlmProvider, Message};
use agentic_note_core::Result;
use std::sync::Arc;

/// Agent that resolves merge conflicts using LLM.
pub struct MergeAssistant {
    llm: Arc<dyn LlmProvider>,
}

impl MergeAssistant {
    pub fn new(llm: Arc<dyn LlmProvider>) -> Self {
        Self { llm }
    }

    /// Resolve conflicting hunks by sending them to the LLM.
    pub async fn resolve_conflict(
        &self,
        ancestor: &str,
        local: &str,
        remote: &str,
    ) -> Result<String> {
        let system = Message::system(
            "You are merging two versions of a markdown note. \
             Preserve the intent of both edits. \
             Output ONLY the merged text. Do not include any explanation or conflict markers.",
        );

        let user = Message::user(format!(
            "## Ancestor version:\n{ancestor}\n\n\
             ## Version A (local):\n{local}\n\n\
             ## Version B (remote):\n{remote}"
        ));

        let opts = ChatOpts {
            max_tokens: Some(1024),
            ..Default::default()
        };

        debug!("Sending merge conflict to LLM");
        self.llm.chat(&[system, user], &opts).await
    }
}

#[async_trait]
impl AgentHandler for MergeAssistant {
    fn agent_id(&self) -> &str {
        "merge-assistant"
    }

    async fn execute(&self, ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
        let ancestor = ctx
            .get_output("merge_ancestor")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let local = ctx
            .get_output("merge_local")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let remote = ctx
            .get_output("merge_remote")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        match self.resolve_conflict(&ancestor, &local, &remote).await {
            Ok(merged) => Ok(json!({ "merged_text": merged })),
            Err(e) => {
                warn!("LLM merge failed, falling back to manual: {e}");
                Ok(json!({ "error": e.to_string(), "fallback": "manual" }))
            }
        }
    }
}
