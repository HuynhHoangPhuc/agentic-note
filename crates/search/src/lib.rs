pub mod fts;
pub mod graph;
pub mod reindex;

use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::NoteId;
use agentic_note_vault::{markdown, Note};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub use fts::{FtsIndex, SearchResult};
pub use graph::Graph;

/// SearchEngine facade: combines FTS, graph, and optional embeddings.
pub struct SearchEngine {
    pub fts: FtsIndex,
    db: Connection,
    #[allow(dead_code)]
    index_dir: PathBuf,
}

impl SearchEngine {
    /// Open or create a search engine for the given vault.
    pub fn open(vault_path: &Path) -> Result<Self> {
        let agentic_dir = vault_path.join(".agentic");
        std::fs::create_dir_all(&agentic_dir)?;

        let index_dir = agentic_dir.join("tantivy");
        let fts = FtsIndex::open(&index_dir)?;

        let db_path = agentic_dir.join("index.db");
        let db = Connection::open(&db_path)
            .map_err(|e| AgenticError::Search(format!("open db: {e}")))?;

        // Init graph tables
        let _graph = Graph::open(&db)?;

        Ok(Self { fts, db, index_dir })
    }

    /// Full-text search.
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.fts.search(query, limit)
    }

    /// Index a single note (incremental).
    pub fn index_note(&self, note: &Note) -> Result<()> {
        let mut writer = self.fts.writer()?;
        self.fts.index_note(
            &writer,
            &note.id,
            &note.frontmatter.title,
            &note.body,
            &note.frontmatter.tags,
        )?;
        writer.commit()
            .map_err(|e| AgenticError::Search(format!("commit: {e}")))?;

        let graph = Graph::open(&self.db)?;
        let links = markdown::extract_wikilinks(&note.body);
        graph.update_note(&note.id, &note.frontmatter.tags, &links)?;

        Ok(())
    }

    /// Remove a note from all indexes.
    pub fn remove_note(&self, id: &NoteId) -> Result<()> {
        let mut writer = self.fts.writer()?;
        self.fts.remove_note(&writer, id);
        writer.commit()
            .map_err(|e| AgenticError::Search(format!("commit: {e}")))?;

        let graph = Graph::open(&self.db)?;
        graph.remove_note(id)?;
        Ok(())
    }

    /// Full reindex of the vault.
    pub fn reindex(&self, vault_path: &Path) -> Result<usize> {
        let graph = Graph::open(&self.db)?;
        reindex::reindex_vault(vault_path, &self.fts, &graph)
    }

    /// Get the graph handle for tag/link queries.
    pub fn graph(&self) -> Result<Graph<'_>> {
        Graph::open(&self.db)
    }
}
