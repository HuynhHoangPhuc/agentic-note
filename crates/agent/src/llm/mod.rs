pub mod anthropic;
pub mod batch_collector;
pub mod cache;
pub mod openai;

use agentic_note_core::error::{AgenticError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// A single chat message (user or assistant role).
#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }
}

/// Options forwarded to the LLM on each request.
#[derive(Debug, Clone, Default)]
pub struct ChatOpts {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// Request JSON output from the model.
    pub json_mode: bool,
}

/// Trait implemented by each LLM backend.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String>;

    /// Execute multiple requests, returning one response per input in order.
    /// Default implementation runs them sequentially; backends may override
    /// with concurrent execution.
    async fn batch_chat(&self, requests: &[(Vec<Message>, ChatOpts)]) -> Result<Vec<String>> {
        let mut results = Vec::with_capacity(requests.len());
        for (msgs, opts) in requests {
            results.push(self.chat(msgs, opts).await?);
        }
        Ok(results)
    }
}

/// Registry of named providers with an active default.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    active: String,
}

impl ProviderRegistry {
    pub fn new(active: impl Into<String>) -> Self {
        Self {
            providers: HashMap::new(),
            active: active.into(),
        }
    }

    /// Register a provider. Replaces any previous entry with the same name.
    pub fn register(&mut self, provider: Arc<dyn LlmProvider>) {
        self.providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn LlmProvider>> {
        self.providers.get(name).cloned()
    }

    /// Get the currently active provider.
    pub fn active(&self) -> Result<Arc<dyn LlmProvider>> {
        self.providers.get(&self.active).cloned().ok_or_else(|| {
            AgenticError::NotFound(format!("LLM provider '{}' not registered", self.active))
        })
    }

    /// Change the active provider name.
    pub fn set_active(&mut self, name: impl Into<String>) {
        self.active = name.into();
    }
}
