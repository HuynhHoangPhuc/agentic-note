/// Batch LLM request collector with deduplication and concurrent execution.
///
/// Callers `add` requests to the collector and receive a `RequestId`. After
/// all requests are registered, `flush` executes the unique set concurrently,
/// returning a map from each `RequestId` to the corresponding response string.
use agentic_note_core::error::{AgenticError, Result};
use std::collections::HashMap;
use std::sync::Arc;

use super::{cache::LlmCache, ChatOpts, LlmProvider, Message};

/// Opaque handle returned by `BatchCollector::add`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(usize);

/// A single pending request stored internally.
struct PendingRequest {
    cache_key: String,
    messages: Vec<Message>,
    opts: ChatOpts,
    model: String,
}

/// Collects LLM requests, deduplicates them by cache key, and executes them
/// concurrently through the provided provider.
pub struct BatchCollector {
    requests: Vec<PendingRequest>,
    /// Maps cache_key → first request index (for deduplication).
    key_to_index: HashMap<String, usize>,
    /// Maps each request slot to the canonical index it deduplicates to.
    canonical: Vec<usize>,
}

impl BatchCollector {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            key_to_index: HashMap::new(),
            canonical: Vec::new(),
        }
    }

    /// Add a request. Returns the `RequestId` that will be present in the
    /// map returned by `flush`. Identical requests share the same underlying
    /// execution slot.
    pub fn add(&mut self, messages: Vec<Message>, opts: ChatOpts) -> RequestId {
        let messages_json = serde_json::to_string(&messages_to_value(&messages))
            .unwrap_or_default();
        let opts_json = opts_to_json(&opts);
        let model = opts.model.clone().unwrap_or_default();
        let cache_key = LlmCache::compute_key(&model, &messages_json, &opts_json);

        let id = RequestId(self.canonical.len());

        if let Some(&existing) = self.key_to_index.get(&cache_key) {
            // Duplicate — point to same slot.
            self.canonical.push(existing);
        } else {
            let slot = self.requests.len();
            self.key_to_index.insert(cache_key.clone(), slot);
            self.canonical.push(slot);
            self.requests.push(PendingRequest {
                cache_key,
                messages,
                opts,
                model,
            });
        }

        id
    }

    /// Execute all unique requests concurrently, using `cache` for read-through
    /// caching. Returns a map from each `RequestId` to the response string.
    pub async fn flush(
        self,
        provider: Arc<dyn LlmProvider>,
        cache: &LlmCache,
    ) -> Result<HashMap<RequestId, String>> {
        let Self {
            requests,
            canonical,
            ..
        } = self;

        // Resolve each slot: cache hit or future to run.
        let mut slot_responses: Vec<Option<String>> = vec![None; requests.len()];
        let mut futures_with_slots: Vec<(usize, _)> = Vec::new();

        for (slot, req) in requests.iter().enumerate() {
            if let Some(cached) = cache.get(&req.cache_key)? {
                slot_responses[slot] = Some(cached);
            } else {
                let prov = Arc::clone(&provider);
                let msgs = req.messages.clone();
                let opts = req.opts.clone();
                let fut = async move { prov.chat(&msgs, &opts).await };
                futures_with_slots.push((slot, fut));
            }
        }

        // Execute non-cached requests concurrently.
        if !futures_with_slots.is_empty() {
            let (slots, futs): (Vec<usize>, Vec<_>) =
                futures_with_slots.into_iter().unzip();

            let results = futures::future::join_all(futs).await;

            for (slot, result) in slots.into_iter().zip(results) {
                let response = result?;
                // Write back to cache.
                let req = &requests[slot];
                if let Err(e) = cache.put(&req.cache_key, &response, &req.model) {
                    tracing::warn!("llm cache write failed: {e}");
                }
                slot_responses[slot] = Some(response);
            }
        }

        // Build final map using canonical indirection.
        let mut out: HashMap<RequestId, String> = HashMap::new();
        for (idx, &slot) in canonical.iter().enumerate() {
            let response = slot_responses[slot]
                .clone()
                .ok_or_else(|| AgenticError::Agent(format!("batch slot {slot} missing response")))?;
            out.insert(RequestId(idx), response);
        }

        Ok(out)
    }
}

impl Default for BatchCollector {
    fn default() -> Self {
        Self::new()
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn messages_to_value(msgs: &[Message]) -> Vec<serde_json::Value> {
    msgs.iter()
        .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
        .collect()
}

fn opts_to_json(opts: &ChatOpts) -> String {
    serde_json::json!({
        "model": opts.model,
        "temperature": opts.temperature,
        "max_tokens": opts.max_tokens,
        "json_mode": opts.json_mode,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn make_cache() -> (LlmCache, NamedTempFile) {
        let f = NamedTempFile::new().unwrap();
        let c = LlmCache::new(f.path()).unwrap();
        (c, f)
    }

    fn user_msg(s: &str) -> Message {
        Message::user(s)
    }

    #[test]
    fn test_add_returns_unique_ids() {
        let mut bc = BatchCollector::new();
        let id1 = bc.add(vec![user_msg("hello")], ChatOpts::default());
        let id2 = bc.add(vec![user_msg("world")], ChatOpts::default());
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_deduplication_same_request() {
        let mut bc = BatchCollector::new();
        let opts = ChatOpts {
            model: Some("gpt-4".into()),
            ..Default::default()
        };
        let id1 = bc.add(vec![user_msg("q")], opts.clone());
        let id2 = bc.add(vec![user_msg("q")], opts);
        // Both ids map to the same canonical slot — confirmed by only 1 unique request.
        assert_eq!(bc.requests.len(), 1);
        assert_ne!(id1, id2); // ids are still distinct handles
    }

    #[tokio::test]
    async fn test_flush_uses_cache() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Provider that counts calls.
        struct CountingProvider(Arc<AtomicUsize>);
        #[async_trait::async_trait]
        impl LlmProvider for CountingProvider {
            fn name(&self) -> &str {
                "counting"
            }
            async fn chat(&self, _msgs: &[Message], _opts: &ChatOpts) -> Result<String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok("response".into())
            }
        }

        let (cache, _f) = make_cache();
        let counter = Arc::new(AtomicUsize::new(0));
        let provider: Arc<dyn LlmProvider> = Arc::new(CountingProvider(counter.clone()));

        // Pre-populate cache for key matching opts=default, model="".
        let key = LlmCache::compute_key("", r#"[{"content":"cached","role":"user"}]"#, &opts_to_json(&ChatOpts::default()));
        cache.put(&key, "cached-response", "").unwrap();

        let mut bc = BatchCollector::new();
        let _id_cached = bc.add(vec![user_msg("cached")], ChatOpts::default());
        let _id_fresh  = bc.add(vec![user_msg("fresh")], ChatOpts::default());

        let results = bc.flush(provider, &cache).await.unwrap();
        assert_eq!(results.len(), 2);
        // Only 1 network call — the cached request was served from cache.
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
