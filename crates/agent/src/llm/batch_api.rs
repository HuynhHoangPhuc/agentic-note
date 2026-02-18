use agentic_note_core::error::{AgenticError, Result};
use serde::{Deserialize, Serialize};

use super::{ChatOpts, Message};

/// Identifier returned by batch submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchId(pub String);

/// Status for an async batch request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    Validating,
    InProgress,
    Completed,
    Failed(String),
    Expired,
    Cancelled,
}

/// JSONL entry for OpenAI batch API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJsonlEntry {
    pub custom_id: String,
    pub method: String,
    pub url: String,
    pub body: BatchRequestBody,
}

/// JSON body for chat completion batch requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequestBody {
    pub model: String,
    pub messages: Vec<BatchMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

/// JSONL message format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI response_format payload for JSON mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
}

/// Build JSONL content for a set of chat requests.
pub fn build_batch_jsonl(
    requests: &[(Vec<Message>, ChatOpts)],
    model_fallback: &str,
) -> Result<String> {
    let mut lines = Vec::with_capacity(requests.len());

    for (index, (messages, opts)) in requests.iter().enumerate() {
        let model = opts
            .model
            .as_deref()
            .unwrap_or(model_fallback)
            .to_string();

        let batch_messages = messages
            .iter()
            .map(|msg| BatchMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            })
            .collect::<Vec<_>>();

        let response_format = if opts.json_mode {
            Some(ResponseFormat {
                format_type: "json_object".to_string(),
            })
        } else {
            None
        };

        let entry = BatchJsonlEntry {
            custom_id: format!("req-{index}"),
            method: "POST".to_string(),
            url: "/v1/chat/completions".to_string(),
            body: BatchRequestBody {
                model,
                messages: batch_messages,
                temperature: opts.temperature,
                max_tokens: opts.max_tokens,
                response_format,
            },
        };

        let line =
            serde_json::to_string(&entry).map_err(|e| AgenticError::Parse(format!(
                "serialize batch jsonl entry: {e}"
            )))?;
        lines.push(line);
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_batch_jsonl_includes_optional_fields() -> Result<()> {
        let requests = vec![vec![Message::user("hello")]]
            .into_iter()
            .map(|msgs| (msgs, ChatOpts::default()))
            .collect::<Vec<_>>();

        let jsonl = build_batch_jsonl(&requests, "gpt-4o")?;
        assert!(jsonl.contains("\"model\":\"gpt-4o\""));
        assert!(jsonl.contains("\"custom_id\":\"req-0\""));
        Ok(())
    }

    #[test]
    fn build_batch_jsonl_sets_json_mode() -> Result<()> {
        let mut opts = ChatOpts::default();
        opts.json_mode = true;
        let requests = vec![(vec![Message::system("system")], opts)];

        let jsonl = build_batch_jsonl(&requests, "gpt-4o")?;
        assert!(jsonl.contains("\"response_format\""));
        assert!(jsonl.contains("\"type\":\"json_object\""));
        Ok(())
    }
}
