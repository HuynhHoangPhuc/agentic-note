use agentic_note_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ChatOpts, LlmProvider, Message};
use std::sync::Arc;

/// Anthropic Claude provider.
pub struct AnthropicProvider {
    api_key: String,
    default_model: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            default_model: model.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String> {
        let model = opts
            .model
            .as_deref()
            .unwrap_or(&self.default_model)
            .to_string();

        // Split system message from the rest (Anthropic uses a top-level `system` field).
        let system: Option<String> = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let user_msgs: Vec<Value> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let mut body = json!({
            "model": model,
            "messages": user_msgs,
            "max_tokens": opts.max_tokens.unwrap_or(1024),
        });

        if let Some(sys) = system {
            body["system"] = json!(sys);
        }
        if let Some(temp) = opts.temperature {
            body["temperature"] = json!(temp);
        }

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgenticError::Agent(format!("anthropic request: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AgenticError::Agent(format!(
                "anthropic HTTP {status}: {text}"
            )));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| AgenticError::Parse(format!("anthropic parse: {e}")))?;

        json["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AgenticError::Parse("anthropic: missing content[0].text".into()))
    }

    /// Concurrent batch execution using `futures::join_all`.
    async fn batch_chat(&self, requests: &[(Vec<Message>, ChatOpts)]) -> Result<Vec<String>> {
        let provider = Arc::new(Self {
            api_key: self.api_key.clone(),
            default_model: self.default_model.clone(),
            client: self.client.clone(),
        });

        let futs: Vec<_> = requests
            .iter()
            .map(|(msgs, opts)| {
                let p = Arc::clone(&provider);
                let msgs = msgs.clone();
                let opts = opts.clone();
                async move { p.chat(&msgs, &opts).await }
            })
            .collect();

        futures::future::join_all(futs)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()
    }
}
