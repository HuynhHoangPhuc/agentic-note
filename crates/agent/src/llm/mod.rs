pub mod anthropic;
pub mod batch_api;
pub mod batch_collector;
pub mod cache;
pub mod openai;

use zenon_core::error::{AgenticError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub use batch_api::{BatchId, BatchStatus};

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
///
/// Use `json_mode` to request structured responses when supported by the provider.
#[derive(Debug, Clone, Default)]
pub struct ChatOpts {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// Request JSON output from the model.
    pub json_mode: bool,
}

/// Trait implemented by each LLM backend.
///
/// Providers receive structured chat messages and return the assistant response
/// content as a plain string.
///
/// # Examples
///
/// ```no_run
/// use zenon_agent::llm::{ChatOpts, Message, ProviderRegistry};
/// use std::sync::Arc;
///
/// # fn main() -> zenon_core::Result<()> {
/// let mut registry = ProviderRegistry::new("openai");
/// // registry.register(Arc::new(OpenAiProvider::new_openai("key", "gpt-4o")));
/// let provider = registry.active()?;
/// let messages = vec![Message::system("You are a helpful assistant."), Message::user("Hi!")];
/// let _reply = futures::executor::block_on(provider.chat(&messages, &ChatOpts::default()))?;
/// # Ok(()) }
/// ```
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name (used as registry key).
    fn name(&self) -> &str;
    /// Execute a single chat request.
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

    #[cfg(feature = "batch-api")]
    async fn batch_submit(&self, _requests: &[(Vec<Message>, ChatOpts)]) -> Result<BatchId> {
        Err(AgenticError::Agent("batch API not supported".into()))
    }

    #[cfg(feature = "batch-api")]
    async fn batch_poll(&self, _batch_id: &BatchId) -> Result<BatchStatus> {
        Err(AgenticError::Agent("batch API not supported".into()))
    }

    #[cfg(feature = "batch-api")]
    async fn batch_results(&self, _batch_id: &BatchId) -> Result<Vec<String>> {
        Err(AgenticError::Agent("batch API not supported".into()))
    }
}

/// Registry of named providers with an active default.
///
/// Use this to register providers and retrieve the active one by name.
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
