use agentic_note_core::error::{AgenticError, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// A single item in the review queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: String,
    pub pipeline: String,
    pub note_id: String,
    pub proposed_changes: Value,
    pub status: String,
    pub created: String,
    pub resolved: Option<String>,
}

/// SQLite-backed review queue for proposed agent changes.
pub struct ReviewQueue {
    conn: Connection,
}

impl ReviewQueue {
    /// Open (or create) the review queue at the given SQLite path.
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .map_err(|e| AgenticError::Agent(format!("review db open: {e}")))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS reviews (
                id       TEXT PRIMARY KEY,
                pipeline TEXT NOT NULL,
                note_id  TEXT NOT NULL,
                proposed_changes TEXT NOT NULL,
                status   TEXT NOT NULL DEFAULT 'pending',
                created  TEXT NOT NULL,
                resolved TEXT
            );",
        )
        .map_err(|e| AgenticError::Agent(format!("review db init: {e}")))?;

        Ok(Self { conn })
    }

    /// Enqueue a new proposed change set. Returns the generated review ID.
    pub fn enqueue(&self, pipeline: &str, note_id: &str, changes: Value) -> Result<String> {
        let id = ulid::Ulid::new().to_string();
        let created = Utc::now().to_rfc3339();
        let changes_json = serde_json::to_string(&changes)
            .map_err(|e| AgenticError::Parse(format!("serialize changes: {e}")))?;

        self.conn
            .execute(
                "INSERT INTO reviews (id, pipeline, note_id, proposed_changes, status, created)
                 VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
                params![id, pipeline, note_id, changes_json, created],
            )
            .map_err(|e| AgenticError::Agent(format!("enqueue: {e}")))?;

        tracing::debug!("enqueued review {id} for pipeline={pipeline} note={note_id}");
        Ok(id)
    }

    /// List reviews, optionally filtered by status ("pending", "approved", "rejected").
    pub fn list(&self, status: Option<&str>) -> Result<Vec<ReviewItem>> {
        let sql = if status.is_some() {
            "SELECT id, pipeline, note_id, proposed_changes, status, created, resolved
             FROM reviews WHERE status = ?1 ORDER BY created DESC"
        } else {
            "SELECT id, pipeline, note_id, proposed_changes, status, created, resolved
             FROM reviews ORDER BY created DESC"
        };

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|e| AgenticError::Agent(format!("list prepare: {e}")))?;

        let rows = if let Some(s) = status {
            stmt.query_map(params![s], row_to_item)
        } else {
            stmt.query_map([], row_to_item)
        }
        .map_err(|e| AgenticError::Agent(format!("list query: {e}")))?;

        rows.map(|r| r.map_err(|e| AgenticError::Agent(format!("row: {e}"))))
            .collect()
    }

    /// Fetch a single review by ID.
    pub fn get(&self, id: &str) -> Result<ReviewItem> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, pipeline, note_id, proposed_changes, status, created, resolved
                 FROM reviews WHERE id = ?1",
            )
            .map_err(|e| AgenticError::Agent(format!("get prepare: {e}")))?;

        stmt.query_row(params![id], row_to_item)
            .map_err(|_| AgenticError::NotFound(format!("review '{id}' not found")))
    }

    /// Approve a review. Returns the proposed_changes value for the caller to apply.
    pub fn approve(&self, id: &str) -> Result<Value> {
        let item = self.get(id)?;
        if item.status != "pending" {
            return Err(AgenticError::Conflict(format!(
                "review '{id}' is already {}",
                item.status
            )));
        }
        let resolved = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE reviews SET status = 'approved', resolved = ?1 WHERE id = ?2",
                params![resolved, id],
            )
            .map_err(|e| AgenticError::Agent(format!("approve: {e}")))?;

        tracing::info!("review {id} approved");
        Ok(item.proposed_changes)
    }

    /// Reject a review.
    pub fn reject(&self, id: &str) -> Result<()> {
        let item = self.get(id)?;
        if item.status != "pending" {
            return Err(AgenticError::Conflict(format!(
                "review '{id}' is already {}",
                item.status
            )));
        }
        let resolved = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE reviews SET status = 'rejected', resolved = ?1 WHERE id = ?2",
                params![resolved, id],
            )
            .map_err(|e| AgenticError::Agent(format!("reject: {e}")))?;

        tracing::info!("review {id} rejected");
        Ok(())
    }
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewItem> {
    let changes_str: String = row.get(3)?;
    let proposed_changes: Value =
        serde_json::from_str(&changes_str).unwrap_or(Value::Null);
    Ok(ReviewItem {
        id: row.get(0)?,
        pipeline: row.get(1)?,
        note_id: row.get(2)?,
        proposed_changes,
        status: row.get(4)?,
        created: row.get(5)?,
        resolved: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::NamedTempFile;

    fn open_temp_queue() -> (ReviewQueue, NamedTempFile) {
        let f = NamedTempFile::new().unwrap();
        let q = ReviewQueue::open(f.path()).unwrap();
        (q, f)
    }

    #[test]
    fn enqueue_list_get_round_trip() {
        let (q, _f) = open_temp_queue();
        let changes = json!({"frontmatter": {"para": "projects"}});
        let id = q.enqueue("classify", "note-01", changes.clone()).unwrap();

        let all = q.list(None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].status, "pending");
        assert_eq!(all[0].proposed_changes, changes);

        let item = q.get(&id).unwrap();
        assert_eq!(item.pipeline, "classify");
        assert_eq!(item.note_id, "note-01");
    }

    #[test]
    fn approve_returns_changes_and_sets_status() {
        let (q, _f) = open_temp_queue();
        let changes = json!({"para": "areas"});
        let id = q.enqueue("classify", "note-02", changes.clone()).unwrap();

        let returned = q.approve(&id).unwrap();
        assert_eq!(returned, changes);

        let item = q.get(&id).unwrap();
        assert_eq!(item.status, "approved");
        assert!(item.resolved.is_some());
    }

    #[test]
    fn reject_sets_status_and_double_reject_errors() {
        let (q, _f) = open_temp_queue();
        let id = q
            .enqueue("classify", "note-03", json!({}))
            .unwrap();
        q.reject(&id).unwrap();

        let item = q.get(&id).unwrap();
        assert_eq!(item.status, "rejected");

        // Double reject should fail
        assert!(q.reject(&id).is_err());
    }
}
