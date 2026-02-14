use agentic_note_core::error::{AgenticError, Result};
use agentic_note_core::types::NoteId;
use serde::Serialize;
use std::path::Path;
use std::str::FromStr;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter, ReloadPolicy};

const HEAP_SIZE: usize = 15_000_000; // 15MB

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub id: NoteId,
    pub title: String,
    pub score: f32,
    pub snippet: String,
}

pub struct FtsIndex {
    index: Index,
    #[allow(dead_code)]
    schema: Schema,
    f_id: Field,
    f_title: Field,
    f_body: Field,
    f_tags: Field,
}

impl FtsIndex {
    /// Open or create a tantivy FTS index at the given directory.
    pub fn open(index_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(index_dir)?;

        let mut builder = Schema::builder();
        let f_id = builder.add_text_field("id", STRING | STORED);
        let f_title = builder.add_text_field("title", TEXT | STORED);
        let f_body = builder.add_text_field("body", TEXT);
        let f_tags = builder.add_text_field("tags", TEXT | STORED);
        let schema = builder.build();

        let dir = tantivy::directory::MmapDirectory::open(index_dir)
            .map_err(|e| AgenticError::Search(format!("tantivy dir: {e}")))?;

        let index = if Index::exists(&dir).unwrap_or(false) {
            Index::open(dir).map_err(|e| AgenticError::Search(format!("open index: {e}")))?
        } else {
            Index::create(dir, schema.clone(), tantivy::IndexSettings::default())
                .map_err(|e| AgenticError::Search(format!("create index: {e}")))?
        };

        Ok(Self {
            index,
            schema,
            f_id,
            f_title,
            f_body,
            f_tags,
        })
    }

    /// Get an index writer for batch operations.
    pub fn writer(&self) -> Result<IndexWriter> {
        self.index
            .writer(HEAP_SIZE)
            .map_err(|e| AgenticError::Search(format!("writer: {e}")))
    }

    /// Add or update a note in the index. Caller must commit the writer.
    pub fn index_note(
        &self,
        writer: &IndexWriter,
        id: &NoteId,
        title: &str,
        body: &str,
        tags: &[String],
    ) -> Result<()> {
        // Delete existing doc with same id
        let id_str = id.to_string();
        let term = tantivy::Term::from_field_text(self.f_id, &id_str);
        writer.delete_term(term);

        let mut doc = TantivyDocument::new();
        doc.add_text(self.f_id, &id_str);
        doc.add_text(self.f_title, title);
        doc.add_text(self.f_body, body);
        doc.add_text(self.f_tags, tags.join(" "));
        writer
            .add_document(doc)
            .map_err(|e| AgenticError::Search(format!("add doc: {e}")))?;
        Ok(())
    }

    /// Remove a note from the index. Caller must commit the writer.
    pub fn remove_note(&self, writer: &IndexWriter, id: &NoteId) {
        let term = tantivy::Term::from_field_text(self.f_id, &id.to_string());
        writer.delete_term(term);
    }

    /// Search the index, returning top N results.
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| AgenticError::Search(format!("reader: {e}")))?;

        let searcher = reader.searcher();
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.f_title, self.f_body, self.f_tags]);

        let query = query_parser
            .parse_query(query_str)
            .map_err(|e| AgenticError::Search(format!("parse query: {e}")))?;

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| AgenticError::Search(format!("search: {e}")))?;

        let mut results = Vec::new();
        for (score, doc_addr) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_addr)
                .map_err(|e| AgenticError::Search(format!("fetch doc: {e}")))?;

            let id_str = doc
                .get_first(self.f_id)
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title = doc
                .get_first(self.f_title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if let Ok(id) = NoteId::from_str(id_str) {
                results.push(SearchResult {
                    id,
                    title,
                    score,
                    snippet: String::new(),
                });
            }
        }
        Ok(results)
    }
}
