use zenon_core::error::{AgenticError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::double_ratchet::DrSessionMaterial;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub peer_id: String,
    pub payload: Vec<u8>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStateRecord {
    pub peer_id: String,
    pub material: DrSessionMaterial,
    pub updated_at: String,
}

pub struct SessionStore {
    conn: Connection,
}

impl SessionStore {
    pub fn ensure_secure_permissions(db_path: &Path) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(db_path, perms)
                .map_err(|e| AgenticError::Sync(format!("set dr_sessions permissions: {e}")))?;
        }
        Ok(())
    }
}

impl SessionStore {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .map_err(|e| AgenticError::Sync(format!("open session store: {e}")))?;
        Self::ensure_secure_permissions(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dr_sessions (
                peer_id TEXT PRIMARY KEY,
                payload BLOB NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .map_err(|e| AgenticError::Sync(format!("create dr_sessions: {e}")))?;
        Ok(Self { conn })
    }

    pub fn load(&self, peer_id: &str) -> Result<Option<SessionRecord>> {
        self.conn
            .query_row(
                "SELECT peer_id, payload, updated_at FROM dr_sessions WHERE peer_id = ?1",
                params![peer_id],
                |row| {
                    Ok(SessionRecord {
                        peer_id: row.get(0)?,
                        payload: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(|e| AgenticError::Sync(format!("load session: {e}")))
    }

    pub fn save(&self, record: &SessionRecord) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO dr_sessions (peer_id, payload, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(peer_id) DO UPDATE SET
                   payload = excluded.payload,
                   updated_at = excluded.updated_at",
                params![record.peer_id, record.payload, record.updated_at],
            )
            .map_err(|e| AgenticError::Sync(format!("save session: {e}")))?;
        Ok(())
    }

    pub fn load_state(&self, peer_id: &str) -> Result<Option<SessionStateRecord>> {
        let Some(record) = self.load(peer_id)? else {
            return Ok(None);
        };

        let material: DrSessionMaterial = bincode::deserialize(&record.payload)
            .map_err(|e| AgenticError::Sync(format!("decode session state: {e}")))?;

        Ok(Some(SessionStateRecord {
            peer_id: record.peer_id,
            material,
            updated_at: record.updated_at,
        }))
    }

    pub fn save_state(&self, record: &SessionStateRecord) -> Result<()> {
        let payload = bincode::serialize(&record.material)
            .map_err(|e| AgenticError::Sync(format!("encode session state: {e}")))?;

        self.save(&SessionRecord {
            peer_id: record.peer_id.clone(),
            payload,
            updated_at: record.updated_at.clone(),
        })
    }

    pub fn delete(&self, peer_id: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM dr_sessions WHERE peer_id = ?1",
                params![peer_id],
            )
            .map_err(|e| AgenticError::Sync(format!("delete session: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn session_store_round_trip() {
        let dir = TempDir::new().expect("temp dir");
        let db_path = dir.path().join("sessions.db");
        let store = SessionStore::new(&db_path).expect("open session store");

        let record = SessionRecord {
            peer_id: "peer-1".to_string(),
            payload: vec![1, 2, 3],
            updated_at: "2026-02-18T00:00:00Z".to_string(),
        };

        store.save(&record).expect("save session");
        let loaded = store
            .load("peer-1")
            .expect("load session")
            .expect("session present");
        assert_eq!(loaded.peer_id, record.peer_id);
        assert_eq!(loaded.payload, record.payload);

        store.delete("peer-1").expect("delete session");
        assert!(store.load("peer-1").expect("load after delete").is_none());
    }
}
