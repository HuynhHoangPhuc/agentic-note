//! SQLite implementation of StorageBackend.
//!
//! Wraps rusqlite::Connection behind the async trait interface.
//! Operations run on a blocking thread via tokio::task::spawn_blocking.

use crate::error::{AgenticError, Result};
use crate::storage::{Row, StorageBackend};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// SQLite storage backend using rusqlite.
pub struct SqliteBackend {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteBackend {
    /// Open (or create) a SQLite database at the given path.
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = rusqlite::Connection::open(db_path)
            .map_err(|e| AgenticError::Database(format!("sqlite open: {e}")))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory SQLite backend (useful for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| AgenticError::Database(format!("sqlite in-memory: {e}")))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Get direct access to the underlying connection (for legacy code that needs rusqlite).
    pub fn connection(&self) -> &Arc<Mutex<rusqlite::Connection>> {
        &self.conn
    }
}

#[async_trait]
impl StorageBackend for SqliteBackend {
    async fn execute(&self, sql: &str, params: &[&str]) -> Result<u64> {
        let conn = self.conn.clone();
        let sql = sql.to_string();
        let params: Vec<String> = params.iter().map(|s| s.to_string()).collect();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                AgenticError::Database(format!("sqlite lock: {e}"))
            })?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
                .iter()
                .map(|s| s as &dyn rusqlite::types::ToSql)
                .collect();
            let changed = conn
                .execute(&sql, param_refs.as_slice())
                .map_err(|e| AgenticError::Database(format!("sqlite execute: {e}")))?;
            Ok(changed as u64)
        })
        .await
        .map_err(|e| AgenticError::Database(format!("spawn_blocking: {e}")))?
    }

    async fn query_rows(&self, sql: &str, params: &[&str]) -> Result<Vec<Row>> {
        let conn = self.conn.clone();
        let sql = sql.to_string();
        let params: Vec<String> = params.iter().map(|s| s.to_string()).collect();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                AgenticError::Database(format!("sqlite lock: {e}"))
            })?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
                .iter()
                .map(|s| s as &dyn rusqlite::types::ToSql)
                .collect();
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| AgenticError::Database(format!("sqlite prepare: {e}")))?;
            let col_names: Vec<String> = stmt
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect();
            let rows = stmt
                .query_map(param_refs.as_slice(), |row| {
                    let mut columns = HashMap::new();
                    for (i, name) in col_names.iter().enumerate() {
                        let val: String = row.get(i).unwrap_or_default();
                        columns.insert(name.clone(), val);
                    }
                    Ok(Row { columns })
                })
                .map_err(|e| AgenticError::Database(format!("sqlite query: {e}")))?;

            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| AgenticError::Database(format!("sqlite collect: {e}")))
        })
        .await
        .map_err(|e| AgenticError::Database(format!("spawn_blocking: {e}")))?
    }

    async fn query_one(&self, sql: &str, params: &[&str]) -> Result<Row> {
        let mut rows = self.query_rows(sql, params).await?;
        if rows.is_empty() {
            return Err(AgenticError::NotFound("no rows returned".into()));
        }
        Ok(rows.remove(0))
    }

    async fn execute_batch(&self, sql: &str) -> Result<()> {
        let conn = self.conn.clone();
        let sql = sql.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| {
                AgenticError::Database(format!("sqlite lock: {e}"))
            })?;
            conn.execute_batch(&sql)
                .map_err(|e| AgenticError::Database(format!("sqlite batch: {e}")))
        })
        .await
        .map_err(|e| AgenticError::Database(format!("spawn_blocking: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_backend_basic_operations() {
        let backend = SqliteBackend::open_in_memory().expect("open sqlite memory");
        backend
            .execute_batch(
                "CREATE TABLE test (id TEXT PRIMARY KEY, value TEXT);",
            )
            .await
            .expect("create table");

        backend
            .execute(
                "INSERT INTO test (id, value) VALUES (?1, ?2)",
                &["k1", "v1"],
            )
            .await
            .expect("insert row");

        let rows = backend
            .query_rows("SELECT id, value FROM test WHERE id = ?1", &["k1"])
            .await
            .expect("query rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("id"), Some("k1"));
        assert_eq!(rows[0].get("value"), Some("v1"));

        let row = backend
            .query_one("SELECT id, value FROM test WHERE id = ?1", &["k1"])
            .await
            .expect("query one");
        assert_eq!(row.get("value"), Some("v1"));
    }
}
