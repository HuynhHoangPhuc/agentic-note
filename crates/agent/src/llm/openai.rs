use agentic_note_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

#[cfg(feature = "batch-api")]
use super::batch_api::{build_batch_jsonl, BatchId, BatchStatus};
use super::{ChatOpts, LlmProvider, Message};
use std::sync::Arc;

const OPENAI_API_BASE_URL: &str = "https://api.openai.com/v1";

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
            base_url: OPENAI_API_BASE_URL.into(),
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
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to build OpenAI client; using default");
                reqwest::Client::new()
            })
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

    /// Concurrent batch execution using `futures::join_all`.
    async fn batch_chat(&self, requests: &[(Vec<Message>, ChatOpts)]) -> Result<Vec<String>> {
        let provider = Arc::new(Self {
            base_url: self.base_url.clone(),
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

    #[cfg(feature = "batch-api")]
    async fn batch_submit(&self, requests: &[(Vec<Message>, ChatOpts)]) -> Result<BatchId> {
        use async_openai::config::OpenAIConfig;
        use async_openai::types::{
            BatchCompletionWindow, BatchEndpoint, BatchRequest, CreateFileRequestArgs, FileInput,
            FilePurpose, InputSource,
        };
        use async_openai::Client;

        let jsonl = build_batch_jsonl(requests, &self.default_model)?;

        let mut config = OpenAIConfig::new().with_api_key(self.api_key.clone());
        if self.base_url != OPENAI_API_BASE_URL {
            config = config.with_api_base(self.base_url.clone());
        }
        let client = Client::with_config(config);

        let request = CreateFileRequestArgs::default()
            .file(FileInput {
                source: InputSource::VecU8 {
                    filename: "batch.jsonl".to_string(),
                    vec: jsonl.into_bytes(),
                },
            })
            .purpose(FilePurpose::Batch)
            .build()
            .map_err(|e| AgenticError::Agent(format!("openai batch file request: {e}")))?;

        let upload = client
            .files()
            .create(request)
            .await
            .map_err(|e| AgenticError::Agent(format!("openai batch upload: {e}")))?;

        let batch = client
            .batches()
            .create(BatchRequest {
                input_file_id: upload.id,
                endpoint: BatchEndpoint::V1ChatCompletions,
                completion_window: BatchCompletionWindow::W24H,
                metadata: None,
            })
            .await
            .map_err(|e| AgenticError::Agent(format!("openai batch create: {e}")))?;

        Ok(BatchId(batch.id))
    }

    #[cfg(feature = "batch-api")]
    async fn batch_poll(&self, batch_id: &BatchId) -> Result<BatchStatus> {
        use async_openai::config::OpenAIConfig;
        use async_openai::Client;
        use tokio_retry::strategy::ExponentialBackoff;
        use tokio_retry::RetryIf;

        let mut config = OpenAIConfig::new().with_api_key(self.api_key.clone());
        if self.base_url != OPENAI_API_BASE_URL {
            config = config.with_api_base(self.base_url.clone());
        }
        let client = Client::with_config(config);

        let strategy = ExponentialBackoff::from_millis(1000)
            .max_delay(std::time::Duration::from_secs(60))
            .take(20);

        let status = RetryIf::spawn(
            strategy,
            || async {
                client
                    .batches()
                    .retrieve(&batch_id.0)
                    .await
                    .map_err(|e| AgenticError::Agent(format!("openai batch status: {e}")))
            },
            |err: &AgenticError| matches!(err, AgenticError::Agent(_)),
        )
        .await?;

        let mapped = match status.status {
            async_openai::types::BatchStatus::Validating => BatchStatus::Validating,
            async_openai::types::BatchStatus::InProgress => BatchStatus::InProgress,
            async_openai::types::BatchStatus::Finalizing => BatchStatus::InProgress,
            async_openai::types::BatchStatus::Completed => BatchStatus::Completed,
            async_openai::types::BatchStatus::Failed => BatchStatus::Failed(
                status
                    .errors
                    .and_then(|errs| errs.data.first().map(|err| err.message.clone()))
                    .unwrap_or_else(|| "batch failed".to_string()),
            ),
            async_openai::types::BatchStatus::Expired => BatchStatus::Expired,
            async_openai::types::BatchStatus::Cancelled => BatchStatus::Cancelled,
            async_openai::types::BatchStatus::Cancelling => BatchStatus::Cancelled,
        };

        Ok(mapped)
    }

    #[cfg(feature = "batch-api")]
    async fn batch_results(&self, batch_id: &BatchId) -> Result<Vec<String>> {
        use async_openai::config::OpenAIConfig;
        use async_openai::Client;
        use serde_json::Value;
        use std::collections::HashMap;

        let mut config = OpenAIConfig::new().with_api_key(self.api_key.clone());
        if self.base_url != OPENAI_API_BASE_URL {
            config = config.with_api_base(self.base_url.clone());
        }
        let client = Client::with_config(config);

        let batch = client
            .batches()
            .retrieve(&batch_id.0)
            .await
            .map_err(|e| AgenticError::Agent(format!("openai batch retrieve: {e}")))?;

        let output_id = batch
            .output_file_id
            .ok_or_else(|| AgenticError::Agent("openai batch output file not ready".into()))?;

        let content = client
            .files()
            .content(&output_id)
            .await
            .map_err(|e| AgenticError::Agent(format!("openai batch content: {e}")))?;

        let content_str = String::from_utf8(content.to_vec())
            .map_err(|e| AgenticError::Parse(format!("batch output utf8: {e}")))?;

        let mut map: HashMap<String, String> = HashMap::new();
        for line in content_str.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)
                .map_err(|e| AgenticError::Parse(format!("batch output json: {e}")))?;
            let custom_id = value["custom_id"].as_str().unwrap_or_default().to_string();
            let content_value = value["response"]["body"]["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            if !custom_id.is_empty() {
                map.insert(custom_id, content_value);
            }
        }

        let mut results = Vec::with_capacity(map.len());
        let mut index = 0usize;
        loop {
            let key = format!("req-{index}");
            if let Some(value) = map.get(&key) {
                results.push(value.clone());
                index += 1;
            } else {
                break;
            }
        }

        Ok(results)
    }
}
