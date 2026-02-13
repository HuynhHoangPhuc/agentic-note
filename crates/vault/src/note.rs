use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::{FrontMatter, NoteId, NoteStatus, ParaCategory};
use chrono::Utc;
use std::path::{Path, PathBuf};

use crate::frontmatter;
use crate::para::para_path;

/// A note with parsed frontmatter, body content, and file path.
#[derive(Debug, Clone)]
pub struct Note {
    pub id: NoteId,
    pub frontmatter: FrontMatter,
    pub body: String,
    pub path: PathBuf,
}

impl Note {
    /// Create a new note file in the vault.
    pub fn create(
        vault: &Path,
        title: &str,
        para: ParaCategory,
        body: &str,
        tags: Vec<String>,
    ) -> Result<Note> {
        let id = agentic_note_core::next_id();
        let now = Utc::now();
        let fm = FrontMatter {
            id,
            title: title.to_string(),
            created: now,
            modified: now,
            tags,
            para: para.clone(),
            links: vec![],
            status: NoteStatus::Seed,
        };

        let filename = Self::filename(&id, title);
        let dir = para_path(vault, &para);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(&filename);

        let content = frontmatter::serialize(&fm, body)?;
        std::fs::write(&path, &content)?;

        Ok(Note {
            id,
            frontmatter: fm,
            body: body.to_string(),
            path,
        })
    }

    /// Read a note from a file path.
    pub fn read(path: &Path) -> Result<Note> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            AgenticError::NotFound(format!("{}: {e}", path.display()))
        })?;
        let (fm, body) = frontmatter::parse(&raw)?;
        Ok(Note {
            id: fm.id,
            frontmatter: fm,
            body,
            path: path.to_path_buf(),
        })
    }

    /// Update an existing note: bumps modified timestamp and rewrites file.
    pub fn update(&mut self) -> Result<()> {
        self.frontmatter.modified = Utc::now();
        let content = frontmatter::serialize(&self.frontmatter, &self.body)?;
        std::fs::write(&self.path, &content)?;
        Ok(())
    }

    /// Delete a note file.
    pub fn delete(path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(AgenticError::NotFound(format!(
                "{}",
                path.display()
            )));
        }
        std::fs::remove_file(path)?;
        Ok(())
    }

    /// Generate a filename from note ID and title: `{ulid}-{slug}.md`
    pub fn filename(id: &NoteId, title: &str) -> String {
        let slug = slug::slugify(title);
        format!("{}-{}.md", id, slug)
    }
}
