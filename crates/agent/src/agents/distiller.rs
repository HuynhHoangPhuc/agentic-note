use zenon_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::engine::{AgentHandler, StageContext};
use crate::llm::{ChatOpts, LlmProvider, Message};

/// Distills a note into a summary and key ideas via LLM.
///
/// Output JSON: `{ "summary": "...", "key_ideas": ["idea1", "idea2"] }`
pub struct Distiller {
    llm: Arc<dyn LlmProvider>,
}

impl Distiller {
    pub fn new(llm: Arc<dyn LlmProvider>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl AgentHandler for Distiller {
    fn agent_id(&self) -> &str {
        "distiller"
    }

    async fn execute(&self, ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
        let system = Message::system(
            "You are a knowledge distillation assistant. \
             Summarise the note in 2-3 sentences and extract 3-5 key ideas as bullet points. \
             Respond ONLY with valid JSON: \
             {\"summary\": \"<summary>\", \"key_ideas\": [\"idea1\", \"idea2\"]}",
        );

        let user = Message::user(format!(
            "Title: {}\n\n{}",
            ctx.frontmatter.title, ctx.note_content
        ));

        let opts = ChatOpts {
            json_mode: true,
            max_tokens: Some(512),
            ..Default::default()
        };

        let raw = self.llm.chat(&[system, user], &opts).await?;

        serde_json::from_str::<Value>(&raw)
            .map_err(|e| AgenticError::Parse(format!("distiller bad JSON: {e} — raw: {raw}")))
    }
}
