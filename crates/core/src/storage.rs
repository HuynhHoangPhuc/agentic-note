//! Storage backend abstraction for database operations.
//!
//! Provides a trait that can be implemented for SQLite (default) and
//! PostgreSQL (behind `postgres` feature flag).

use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// A single row returned from a query, mapping column names to string values.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub columns: HashMap<String, String>,
}

impl Row {
    pub fn get(&self, col: &str) -> Option<&str> {
        self.columns.get(col).map(|s| s.as_str())
    }
}

/// Async storage backend trait for database operations.
/// Implemented by SqliteBackend (default) and PostgresBackend (feature-gated).
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Execute a statement (INSERT, UPDATE, DELETE, CREATE TABLE, etc.).
    async fn execute(&self, sql: &str, params: &[&str]) -> Result<u64>;

    /// Query multiple rows.
    async fn query_rows(&self, sql: &str, params: &[&str]) -> Result<Vec<Row>>;

    /// Query a single row. Returns error if not found.
    async fn query_one(&self, sql: &str, params: &[&str]) -> Result<Row>;

    /// Execute a batch of SQL statements (no params). Used for schema init.
    async fn execute_batch(&self, sql: &str) -> Result<()>;
}
