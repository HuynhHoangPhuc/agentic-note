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
    ///
    /// # Errors
    ///
    /// Returns an error if the vault directory cannot be created or the note
    /// cannot be written to disk.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use agentic_note_core::types::ParaCategory;
    /// use agentic_note_vault::Note;
    /// # use std::path::Path;
    /// # fn main() -> agentic_note_core::Result<()> {
    /// let note = Note::create(
    ///     Path::new("/path/to/vault"),
    ///     "My note",
    ///     ParaCategory::Inbox,
    ///     "Body",
    ///     vec!["tag".to_string()],
    /// )?;
    /// # Ok(()) }
    /// ```
    pub fn create(
        vault: &Path,
        title: &str,
        para: ParaCategory,
        body: &str,
        tags: Vec<String>,
    ) -> Result<Note> {
        metrics::counter!("note_operations_total", "operation" => "create").increment(1);
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
    ///
    /// # Errors
    ///
    /// Returns `AgenticError::NotFound` if the file does not exist or cannot be read.
    pub fn read(path: &Path) -> Result<Note> {
        metrics::counter!("note_operations_total", "operation" => "read").increment(1);
        let raw = std::fs::read_to_string(path)
            .map_err(|e| AgenticError::NotFound(format!("{}: {e}", path.display())))?;
        let (fm, body) = frontmatter::parse(&raw)?;
        Ok(Note {
            id: fm.id,
            frontmatter: fm,
            body,
            path: path.to_path_buf(),
        })
    }

    /// Update an existing note: bumps modified timestamp and rewrites file.
    ///
    /// # Errors
    ///
    /// Returns an error if the note cannot be serialized or written to disk.
    pub fn update(&mut self) -> Result<()> {
        metrics::counter!("note_operations_total", "operation" => "update").increment(1);
        self.frontmatter.modified = Utc::now();
        let content = frontmatter::serialize(&self.frontmatter, &self.body)?;
        std::fs::write(&self.path, &content)?;
        Ok(())
    }

    /// Delete a note file.
    ///
    /// # Errors
    ///
    /// Returns `AgenticError::NotFound` if the file does not exist.
    pub fn delete(path: &Path) -> Result<()> {
        metrics::counter!("note_operations_total", "operation" => "delete").increment(1);
        if !path.exists() {
            return Err(AgenticError::NotFound(format!("{}", path.display())));
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
