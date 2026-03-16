use zenon_core::error::{AgenticError, Result};
use zenon_core::types::NoteId;
use rusqlite::Connection;
use std::str::FromStr;

/// SQLite-backed tag and link graph for note relationships.
pub struct Graph<'a> {
    conn: &'a Connection,
}

impl<'a> Graph<'a> {
    /// Initialize graph tables on the given connection.
    pub fn open(conn: &'a Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS note_tags (
                note_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (note_id, tag)
            );
            CREATE TABLE IF NOT EXISTS note_links (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                PRIMARY KEY (source_id, target_id)
            );
            CREATE INDEX IF NOT EXISTS idx_tags_tag ON note_tags(tag);
            CREATE INDEX IF NOT EXISTS idx_links_target ON note_links(target_id);",
        )
        .map_err(|e| AgenticError::Search(format!("init graph tables: {e}")))?;

        Ok(Self { conn })
    }

    /// Update tags and links for a note (replaces existing).
    pub fn update_note(&self, id: &NoteId, tags: &[String], links: &[String]) -> Result<()> {
        let id_str = id.to_string();

        // Clear existing
        self.conn
            .execute("DELETE FROM note_tags WHERE note_id = ?1", [&id_str])
            .map_err(|e| AgenticError::Search(format!("delete tags: {e}")))?;
        self.conn
            .execute("DELETE FROM note_links WHERE source_id = ?1", [&id_str])
            .map_err(|e| AgenticError::Search(format!("delete links: {e}")))?;

        // Insert tags
        let mut tag_stmt = self
            .conn
            .prepare("INSERT OR IGNORE INTO note_tags (note_id, tag) VALUES (?1, ?2)")
            .map_err(|e| AgenticError::Search(format!("prepare tag insert: {e}")))?;
        for tag in tags {
            tag_stmt
                .execute(rusqlite::params![&id_str, tag])
                .map_err(|e| AgenticError::Search(format!("insert tag: {e}")))?;
        }

        // Insert links
        let mut link_stmt = self
            .conn
            .prepare("INSERT OR IGNORE INTO note_links (source_id, target_id) VALUES (?1, ?2)")
            .map_err(|e| AgenticError::Search(format!("prepare link insert: {e}")))?;
        for link in links {
            link_stmt
                .execute(rusqlite::params![&id_str, link])
                .map_err(|e| AgenticError::Search(format!("insert link: {e}")))?;
        }

        Ok(())
    }

    /// Remove a note from the graph.
    pub fn remove_note(&self, id: &NoteId) -> Result<()> {
        let id_str = id.to_string();
        self.conn
            .execute("DELETE FROM note_tags WHERE note_id = ?1", [&id_str])
            .map_err(|e| AgenticError::Search(format!("rm tags: {e}")))?;
        self.conn
            .execute("DELETE FROM note_links WHERE source_id = ?1", [&id_str])
            .map_err(|e| AgenticError::Search(format!("rm links: {e}")))?;
        Ok(())
    }

    /// List all tags with their note count.
    pub fn tags(&self) -> Result<Vec<(String, usize)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag, COUNT(*) as cnt FROM note_tags GROUP BY tag ORDER BY cnt DESC")
            .map_err(|e| AgenticError::Search(format!("query tags: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
            })
            .map_err(|e| AgenticError::Search(format!("fetch tags: {e}")))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AgenticError::Search(format!("collect tags: {e}")))
    }

    /// Get all note IDs with a given tag.
    pub fn notes_by_tag(&self, tag: &str) -> Result<Vec<NoteId>> {
        let mut stmt = self
            .conn
            .prepare("SELECT note_id FROM note_tags WHERE tag = ?1")
            .map_err(|e| AgenticError::Search(format!("query by tag: {e}")))?;
        let rows = stmt
            .query_map([tag], |row| row.get::<_, String>(0))
            .map_err(|e| AgenticError::Search(format!("fetch by tag: {e}")))?;

        rows.filter_map(|r| r.ok())
            .filter_map(|s| NoteId::from_str(&s).ok())
            .collect::<Vec<_>>()
            .pipe_ok()
    }

    /// Get incoming links (backlinks) for a note.
    pub fn incoming_links(&self, id: &NoteId) -> Result<Vec<NoteId>> {
        let mut stmt = self
            .conn
            .prepare("SELECT source_id FROM note_links WHERE target_id = ?1")
            .map_err(|e| AgenticError::Search(format!("query backlinks: {e}")))?;
        let rows = stmt
            .query_map([id.to_string()], |row| row.get::<_, String>(0))
            .map_err(|e| AgenticError::Search(format!("fetch backlinks: {e}")))?;

        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|s| NoteId::from_str(&s).ok())
            .collect())
    }

    /// Get outgoing links from a note.
    pub fn outgoing_links(&self, id: &NoteId) -> Result<Vec<NoteId>> {
        let mut stmt = self
            .conn
            .prepare("SELECT target_id FROM note_links WHERE source_id = ?1")
            .map_err(|e| AgenticError::Search(format!("query outlinks: {e}")))?;
        let rows = stmt
            .query_map([id.to_string()], |row| row.get::<_, String>(0))
            .map_err(|e| AgenticError::Search(format!("fetch outlinks: {e}")))?;

        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|s| NoteId::from_str(&s).ok())
            .collect())
    }

    /// Find orphan notes (no incoming links).
    pub fn orphans(&self) -> Result<Vec<NoteId>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT nt.note_id FROM note_tags nt
                 WHERE nt.note_id NOT IN (SELECT target_id FROM note_links)",
            )
            .map_err(|e| AgenticError::Search(format!("query orphans: {e}")))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| AgenticError::Search(format!("fetch orphans: {e}")))?;

        Ok(rows
            .filter_map(|r| r.ok())
            .filter_map(|s| NoteId::from_str(&s).ok())
            .collect())
    }
}

/// Helper trait to wrap Vec in Ok.
trait PipeOk {
    fn pipe_ok(self) -> Result<Self>
    where
        Self: Sized;
}

impl<T> PipeOk for Vec<T> {
    fn pipe_ok(self) -> Result<Self> {
        Ok(self)
    }
}
