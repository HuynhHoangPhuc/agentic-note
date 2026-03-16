//! Note CRUD, PARA organization, and YAML frontmatter management.
//!
//! Provides the `Vault` interface for note discovery alongside `Note` helpers
//! for reading and writing Markdown notes with YAML frontmatter.

pub mod frontmatter;
pub mod init;
pub mod markdown;
pub mod note;
pub mod para;

use zenon_core::config::AppConfig;
use zenon_core::error::{AgenticError, Result};
use zenon_core::types::{NoteId, NoteStatus, ParaCategory};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub use init::init_vault;
pub use note::Note;

/// Lightweight note summary (no body content) for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: NoteId,
    pub title: String,
    pub para: ParaCategory,
    pub tags: Vec<String>,
    pub status: NoteStatus,
    pub modified: DateTime<Utc>,
    pub path: PathBuf,
}

/// Filter criteria for listing notes.
#[derive(Debug, Default)]
pub struct NoteFilter {
    pub para: Option<ParaCategory>,
    pub tags: Option<Vec<String>>,
    pub status: Option<NoteStatus>,
}

/// The main vault handle, providing access to notes.
pub struct Vault {
    pub root: PathBuf,
    pub config: AppConfig,
}

impl Vault {
    /// Open an existing vault, validating structure and loading config.
    pub fn open(path: &Path) -> Result<Self> {
        let issues = para::validate_structure(path)?;
        if !issues.is_empty() {
            return Err(AgenticError::Config(format!(
                "vault structure issues: {}",
                issues.join(", ")
            )));
        }
        let config = AppConfig::load(Some(path.to_path_buf()))?;
        Ok(Vault {
            root: path.to_path_buf(),
            config,
        })
    }

    /// List notes matching the given filter.
    pub fn list_notes(&self, filter: &NoteFilter) -> Result<Vec<NoteSummary>> {
        let mut summaries = Vec::new();

        for entry in WalkDir::new(&self.root)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            // Read only frontmatter (first ~20 lines) for performance
            let raw = match std::fs::read_to_string(path) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let (fm, _) = match frontmatter::parse(&raw) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Apply filters
            if let Some(ref para) = filter.para {
                if &fm.para != para {
                    continue;
                }
            }
            if let Some(ref tags) = filter.tags {
                if !tags.iter().any(|t| fm.tags.contains(t)) {
                    continue;
                }
            }
            if let Some(ref status) = filter.status {
                if &fm.status != status {
                    continue;
                }
            }

            summaries.push(NoteSummary {
                id: fm.id,
                title: fm.title,
                para: fm.para,
                tags: fm.tags,
                status: fm.status,
                modified: fm.modified,
                path: path.to_path_buf(),
            });
        }

        // Sort by modified desc
        summaries.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(summaries)
    }
}
