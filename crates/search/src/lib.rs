pub mod fts;
pub mod graph;
pub mod reindex;

#[cfg(feature = "embeddings")]
pub mod embedding;
#[cfg(feature = "embeddings")]
pub mod hybrid;
#[cfg(feature = "embeddings")]
pub mod model_download;

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
    #[cfg(feature = "embeddings")]
    embedding_index: Option<embedding::EmbeddingIndex>,
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

        #[cfg(feature = "embeddings")]
        let embedding_index = {
            let cache_dir = model_download::default_cache_dir();
            match model_download::ensure_model(&cache_dir) {
                Ok(model_path) => match embedding::EmbeddingIndex::open(&db, &model_path) {
                    Ok(idx) => {
                        tracing::info!("embeddings enabled");
                        Some(idx)
                    }
                    Err(e) => {
                        tracing::warn!("embeddings unavailable: {e}");
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!("model download failed: {e}");
                    None
                }
            }
        };

        Ok(Self {
            fts,
            db,
            index_dir,
            #[cfg(feature = "embeddings")]
            embedding_index,
        })
    }

    /// Full-text search.
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.fts.search(query, limit)
    }

    /// Semantic search via embeddings (requires `embeddings` feature).
    #[cfg(feature = "embeddings")]
    pub fn search_semantic(&mut self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let idx = self
            .embedding_index
            .as_mut()
            .ok_or_else(|| AgenticError::Embedding("embeddings not available".into()))?;
        let results = idx.search(&self.db, query, limit)?;
        Ok(results
            .into_iter()
            .filter_map(|(id_str, score)| {
                id_str.parse().ok().map(|id| SearchResult {
                    id,
                    title: String::new(),
                    snippet: String::new(),
                    score,
                })
            })
            .collect())
    }

    /// Hybrid search: FTS + semantic fused via RRF (requires `embeddings` feature).
    #[cfg(feature = "embeddings")]
    pub fn search_hybrid(&mut self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let fts_results = self.search_fts(query, limit * 2)?;
        let idx = self
            .embedding_index
            .as_mut()
            .ok_or_else(|| AgenticError::Embedding("embeddings not available".into()))?;
        let semantic_results = idx.search(&self.db, query, limit * 2)?;
        let mut fused = hybrid::fuse_rrf(&fts_results, &semantic_results, 60);
        fused.truncate(limit);
        Ok(fused)
    }

    /// Index a single note (incremental).
    pub fn index_note(&mut self, note: &Note) -> Result<()> {
        let mut writer = self.fts.writer()?;
        self.fts.index_note(
            &writer,
            &note.id,
            &note.frontmatter.title,
            &note.body,
            &note.frontmatter.tags,
        )?;
        writer
            .commit()
            .map_err(|e| AgenticError::Search(format!("commit: {e}")))?;

        let graph = Graph::open(&self.db)?;
        let links = markdown::extract_wikilinks(&note.body);
        graph.update_note(&note.id, &note.frontmatter.tags, &links)?;

        // Also index embedding if available
        #[cfg(feature = "embeddings")]
        if let Some(ref mut idx) = self.embedding_index {
            let text = format!("{} {}", note.frontmatter.title, note.body);
            if let Err(e) = idx.index_note(&self.db, &note.id.to_string(), &text) {
                tracing::warn!("embedding index failed for {}: {e}", note.id);
            }
        }

        Ok(())
    }

    /// Remove a note from all indexes.
    pub fn remove_note(&self, id: &NoteId) -> Result<()> {
        let mut writer = self.fts.writer()?;
        self.fts.remove_note(&writer, id);
        writer
            .commit()
            .map_err(|e| AgenticError::Search(format!("commit: {e}")))?;

        let graph = Graph::open(&self.db)?;
        graph.remove_note(id)?;

        #[cfg(feature = "embeddings")]
        if let Err(e) = embedding::EmbeddingIndex::remove_note(&self.db, &id.to_string()) {
            tracing::warn!("embedding remove failed for {id}: {e}");
        }

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
