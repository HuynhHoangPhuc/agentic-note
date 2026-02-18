use agentic_note_core::types::{FrontMatter, NoteId};
use agentic_note_vault::Note;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Per-note execution context passed through every pipeline stage.
///
/// Stages read input from `note_content`/`frontmatter` and write results
/// into `outputs` under their own stage name key.
#[derive(Debug, Clone)]
pub struct StageContext {
    pub note_id: NoteId,
    pub note_content: String,
    pub frontmatter: FrontMatter,
    /// Stage outputs keyed by stage name.
    pub outputs: HashMap<String, Value>,
    pub vault_path: PathBuf,
}

impl StageContext {
    /// Build a context from a loaded `Note`.
    pub fn from_note(note: &Note, vault_path: &Path) -> Self {
        Self {
            note_id: note.id,
            note_content: note.body.clone(),
            frontmatter: note.frontmatter.clone(),
            outputs: HashMap::new(),
            vault_path: vault_path.to_path_buf(),
        }
    }

    /// Store a stage result under `stage` key.
    pub fn set_output(&mut self, stage: &str, value: Value) {
        self.outputs.insert(stage.to_string(), value);
    }

    /// Retrieve a previous stage result by name.
    pub fn get_output(&self, stage: &str) -> Option<&Value> {
        self.outputs.get(stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentic_note_core::types::{NoteId, NoteStatus, ParaCategory};
    use chrono::Utc;

    fn dummy_frontmatter() -> FrontMatter {
        FrontMatter {
            id: NoteId::new(),
            title: "Test".into(),
            created: Utc::now(),
            modified: Utc::now(),
            tags: vec![],
            para: ParaCategory::Inbox,
            links: vec![],
            status: NoteStatus::Seed,
        }
    }

    #[test]
    fn set_and_get_output_round_trips() {
        let fm = dummy_frontmatter();
        let mut ctx = StageContext {
            note_id: fm.id,
            note_content: String::new(),
            frontmatter: fm,
            outputs: HashMap::new(),
            vault_path: PathBuf::from("/tmp/vault"),
        };
        ctx.set_output("summarise", serde_json::json!({"summary": "hello"}));
        let out = ctx
            .get_output("summarise")
            .expect("expected output present");
        assert_eq!(out["summary"], "hello");
        assert!(ctx.get_output("missing").is_none());
    }
}
