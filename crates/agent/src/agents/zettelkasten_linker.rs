use zenon_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};

use crate::engine::{AgentHandler, StageContext};
use crate::llm::{ChatOpts, LlmProvider, Message};
use zenon_search::SearchEngine;

/// Suggests wikilinks to related notes using FTS search + LLM ranking.
///
/// `SearchEngine` contains a non-Sync `rusqlite::Connection`, so we wrap it
/// in a `Mutex` to satisfy the `Send + Sync` bounds required by `AgentHandler`.
///
/// Output JSON:
/// `{ "suggestions": [{ "note_id": "...", "title": "...", "reason": "..." }] }`
pub struct ZettelkastenLinker {
    llm: Arc<dyn LlmProvider>,
    /// Mutex makes the non-Sync SearchEngine safe to share across threads.
    search: Option<Arc<Mutex<SearchEngine>>>,
}

impl ZettelkastenLinker {
    pub fn new(llm: Arc<dyn LlmProvider>, search: Option<Arc<Mutex<SearchEngine>>>) -> Self {
        Self { llm, search }
    }
}

#[async_trait]
impl AgentHandler for ZettelkastenLinker {
    fn agent_id(&self) -> &str {
        "zettelkasten-linker"
    }

    async fn execute(&self, ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
        // Build a short query from title + first 200 chars of content.
        let query = format!(
            "{} {}",
            ctx.frontmatter.title,
            ctx.note_content.chars().take(200).collect::<String>()
        );

        let note_id = ctx.note_id;

        // FTS candidates (up to 10 results), executed synchronously inside the mutex.
        let candidates: Vec<String> = if let Some(se_mutex) = &self.search {
            let se = se_mutex
                .lock()
                .map_err(|_| AgenticError::Agent("search lock poisoned".into()))?;
            se.search_fts(&query, 10)
                .unwrap_or_default()
                .into_iter()
                .filter(|r| r.id != note_id)
                .map(|r| {
                    format!(
                        "- id={} title=\"{}\" snippet=\"{}\"",
                        r.id, r.title, r.snippet
                    )
                })
                .collect()
        } else {
            vec![]
        };

        let candidate_text = if candidates.is_empty() {
            "No candidates found via search.".to_string()
        } else {
            candidates.join("\n")
        };

        let system = Message::system(
            "You are a Zettelkasten assistant. Given a note and a list of candidate notes, \
             select the most relevant ones to link to and briefly explain why. \
             Respond ONLY with valid JSON: \
             {\"suggestions\": [{\"note_id\": \"<id>\", \"title\": \"<title>\", \"reason\": \"<why>\"}]}",
        );

        let user = Message::user(format!(
            "Current note:\nTitle: {}\n\n{}\n\nCandidates:\n{}",
            ctx.frontmatter.title, ctx.note_content, candidate_text
        ));

        let opts = ChatOpts {
            json_mode: true,
            max_tokens: Some(512),
            ..Default::default()
        };

        let raw = self.llm.chat(&[system, user], &opts).await?;

        serde_json::from_str::<Value>(&raw).map_err(|e| {
            AgenticError::Parse(format!("zettelkasten-linker bad JSON: {e} — raw: {raw}"))
        })
    }
}
