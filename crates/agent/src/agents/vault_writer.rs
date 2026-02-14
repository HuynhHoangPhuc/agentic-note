use agentic_note_core::error::{AgenticError, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::engine::{AgentHandler, StageContext};

/// Aggregates upstream stage outputs into a proposed-changes document.
///
/// Does NOT write to disk — it emits a JSON description of what should change,
/// which the review gate (or the CLI) decides whether to apply.
///
/// Output JSON:
/// ```json
/// {
///   "note_id": "...",
///   "proposed_frontmatter": { ... },
///   "proposed_links": ["note-id-1", "note-id-2"],
///   "proposed_summary": "...",
///   "sources": { "para": "para-classifier", "links": "zettelkasten-linker", ... }
/// }
/// ```
pub struct VaultWriter;

impl VaultWriter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VaultWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentHandler for VaultWriter {
    fn agent_id(&self) -> &str {
        "vault-writer"
    }

    async fn execute(&self, ctx: &mut StageContext, _config: &toml::Value) -> Result<Value> {
        let note_id = ctx.note_id.to_string();
        let mut proposed_frontmatter = json!({});
        let mut proposed_links: Vec<String> = vec![];
        let mut proposed_summary: Option<String> = None;
        let mut sources = json!({});

        // Pull PARA classification output.
        if let Some(classification) = ctx.get_output("classification") {
            if let Some(para) = classification.get("para").and_then(|v| v.as_str()) {
                proposed_frontmatter["para"] = json!(para);
            }
            if let Some(tags) = classification.get("tags") {
                proposed_frontmatter["tags"] = tags.clone();
            }
            sources["para"] = json!("para-classifier");
        }

        // Pull zettelkasten link suggestions.
        if let Some(link_out) = ctx.get_output("links") {
            if let Some(suggestions) = link_out.get("suggestions").and_then(|v| v.as_array()) {
                for s in suggestions {
                    if let Some(id) = s.get("note_id").and_then(|v| v.as_str()) {
                        proposed_links.push(id.to_string());
                    }
                }
            }
            sources["links"] = json!("zettelkasten-linker");
        }

        // Pull distillation summary.
        if let Some(distilled) = ctx.get_output("distillation") {
            if let Some(summary) = distilled.get("summary").and_then(|v| v.as_str()) {
                proposed_summary = Some(summary.to_string());
            }
            sources["summary"] = json!("distiller");
        }

        if proposed_frontmatter
            .as_object()
            .is_none_or(|o| o.is_empty())
            && proposed_links.is_empty()
            && proposed_summary.is_none()
        {
            return Err(AgenticError::Agent(
                "vault-writer: no upstream stage outputs found".into(),
            ));
        }

        Ok(json!({
            "note_id": note_id,
            "proposed_frontmatter": proposed_frontmatter,
            "proposed_links": proposed_links,
            "proposed_summary": proposed_summary,
            "sources": sources,
        }))
    }
}
