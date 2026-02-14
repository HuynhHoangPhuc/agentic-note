use agentic_note_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ChatOpts, LlmProvider, Message};

/// OpenAI-compatible provider (also works for Ollama and other OpenAI-API clones).
pub struct OpenAiProvider {
    base_url: String,
    api_key: String,
    default_model: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// Create a provider pointing at the OpenAI API.
    pub fn new_openai(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: api_key.into(),
            default_model: model.into(),
            client: Self::build_client(),
        }
    }

    /// Create a provider pointing at a custom base URL (e.g., Ollama at localhost).
    pub fn new_custom(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            default_model: model.into(),
            client: Self::build_client(),
        }
    }

    fn build_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client")
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String> {
        let model = opts
            .model
            .as_deref()
            .unwrap_or(&self.default_model)
            .to_string();

        let msgs: Vec<Value> = messages
            .iter()
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let mut body = json!({
            "model": model,
            "messages": msgs,
        });

        if let Some(temp) = opts.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max_tok) = opts.max_tokens {
            body["max_tokens"] = json!(max_tok);
        }
        if opts.json_mode {
            body["response_format"] = json!({"type": "json_object"});
        }

        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgenticError::Agent(format!("openai request: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AgenticError::Agent(format!("openai HTTP {status}: {text}")));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| AgenticError::Parse(format!("openai parse: {e}")))?;

        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AgenticError::Parse("openai: missing content field".into()))
    }
}
