use zenon_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::engine::{AgentHandler, StageContext};
use crate::llm::{ChatOpts, LlmProvider, Message};

/// Classifies a note into a PARA category and suggests tags via LLM.
///
/// Output JSON: `{ "para": "projects", "tags": ["rust", "llm"], "confidence": 0.9 }`
pub struct ParaClassifier {
    llm: Arc<dyn LlmProvider>,
}

impl ParaClassifier {
    pub fn new(llm: Arc<dyn LlmProvider>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl AgentHandler for ParaClassifier {
    fn agent_id(&self) -> &str {
        "para-classifier"
    }

    async fn execute(&self, ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
        let system = Message::system(
            "You are a personal knowledge management assistant. \
             Classify the note into one PARA category: \
             projects, areas, resources, archives, inbox, or zettelkasten. \
             Also suggest relevant lowercase tags. \
             Respond ONLY with valid JSON: \
             {\"para\": \"<category>\", \"tags\": [\"tag1\", \"tag2\"], \"confidence\": <0-1>}",
        );

        let user = Message::user(format!(
            "Title: {}\n\n{}",
            ctx.frontmatter.title, ctx.note_content
        ));

        let opts = ChatOpts {
            json_mode: true,
            max_tokens: Some(256),
            ..Default::default()
        };

        let raw = self.llm.chat(&[system, user], &opts).await?;

        serde_json::from_str::<Value>(&raw)
            .map_err(|e| AgenticError::Parse(format!("para-classifier bad JSON: {e} — raw: {raw}")))
    }
}
