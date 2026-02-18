//! PostgreSQL implementation of StorageBackend (behind `postgres` feature flag).
//!
//! Uses sqlx with async connection pool.

use crate::error::{AgenticError, Result};
use crate::storage::{Row, StorageBackend};
use async_trait::async_trait;
use std::collections::HashMap;

use sqlx::Column;

/// PostgreSQL storage backend using sqlx.
pub struct PostgresBackend {
    pool: sqlx::PgPool,
}

impl PostgresBackend {
    /// Connect to PostgreSQL with the given URL and pool size.
    pub async fn connect(url: &str, max_connections: u32) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(url)
            .await
            .map_err(|e| AgenticError::Database(format!("postgres connect: {e}")))?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
    async fn execute(&self, sql: &str, params: &[&str]) -> Result<u64> {
        // sqlx requires numbered placeholders ($1, $2, ...) for Postgres.
        // Convert ?1, ?2, ... to $1, $2, ... if present.
        let sql = convert_placeholders(sql);
        let mut query = sqlx::query(&sql);
        for p in params {
            query = query.bind(*p);
        }
        let result = query
            .execute(&self.pool)
            .await
            .map_err(|e| AgenticError::Database(format!("postgres execute: {e}")))?;
        Ok(result.rows_affected())
    }

    async fn query_rows(&self, sql: &str, params: &[&str]) -> Result<Vec<Row>> {
        let sql = convert_placeholders(sql);
        let mut query = sqlx::query(&sql);
        for p in params {
            query = query.bind(*p);
        }
        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AgenticError::Database(format!("postgres query: {e}")))?;

        Ok(rows
            .iter()
            .map(|row| {
                use sqlx::Row as SqlxRow;
                let mut columns = HashMap::new();
                for col in row.columns() {
                    let name = col.name().to_string();
                    let val: String = row.try_get::<String, _>(col.ordinal()).unwrap_or_default();
                    columns.insert(name, val);
                }
                Row { columns }
            })
            .collect())
    }

    async fn query_one(&self, sql: &str, params: &[&str]) -> Result<Row> {
        let mut rows = self.query_rows(sql, params).await?;
        if rows.is_empty() {
            return Err(AgenticError::NotFound("no rows returned".into()));
        }
        Ok(rows.remove(0))
    }

    async fn execute_batch(&self, sql: &str) -> Result<()> {
        sqlx::raw_sql(sql)
            .execute(&self.pool)
            .await
            .map_err(|e| AgenticError::Database(format!("postgres batch: {e}")))?;
        Ok(())
    }
}

/// Convert SQLite-style `?1`, `?2` placeholders to Postgres-style `$1`, `$2`.
fn convert_placeholders(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '?' {
            if let Some(&next) = chars.peek() {
                if next.is_ascii_digit() {
                    result.push('$');
                    continue;
                }
            }
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_conversion() {
        assert_eq!(
            convert_placeholders("SELECT * WHERE id = ?1"),
            "SELECT * WHERE id = $1"
        );
        assert_eq!(
            convert_placeholders("INSERT INTO t (a, b) VALUES (?1, ?2)"),
            "INSERT INTO t (a, b) VALUES ($1, $2)"
        );
        assert_eq!(convert_placeholders("SELECT * FROM t"), "SELECT * FROM t");
    }
}
