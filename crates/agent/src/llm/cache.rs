/// LLM response cache backed by SQLite.
///
/// Cache key is SHA-256 of (model + messages_json + opts_json), so identical
/// requests across sessions are de-duplicated automatically.
use agentic_note_core::error::{AgenticError, Result};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;

pub struct LlmCache {
    conn: Mutex<Connection>,
}

impl LlmCache {
    /// Open (or create) the SQLite cache at `db_path`.
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(db_path)
            .map_err(|e| AgenticError::Database(format!("llm cache open: {e}")))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS llm_cache (
                cache_key TEXT PRIMARY KEY,
                response  TEXT NOT NULL,
                model     TEXT NOT NULL,
                created   TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS llm_cache_created ON llm_cache(created);",
        )
        .map_err(|e| AgenticError::Database(format!("llm cache init: {e}")))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Compute a deterministic SHA-256 cache key from (model, messages_json, opts_json).
    pub fn compute_key(model: &str, messages_json: &str, opts_json: &str) -> String {
        let mut h = Sha256::new();
        h.update(model.as_bytes());
        h.update(b"\x00");
        h.update(messages_json.as_bytes());
        h.update(b"\x00");
        h.update(opts_json.as_bytes());
        format!("{:x}", h.finalize())
    }

    /// Look up a cached response. Returns `None` on miss.
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AgenticError::Database("llm cache lock poisoned".into()))?;

        let mut stmt = conn
            .prepare_cached("SELECT response FROM llm_cache WHERE cache_key = ?1")
            .map_err(|e| AgenticError::Database(format!("llm cache prepare: {e}")))?;

        let mut rows = stmt
            .query(params![key])
            .map_err(|e| AgenticError::Database(format!("llm cache query: {e}")))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| AgenticError::Database(format!("llm cache row: {e}")))?
        {
            let response: String = row
                .get::<_, String>(0)
                .map_err(|e| AgenticError::Database(format!("llm cache get col: {e}")))?;
            return Ok(Some(response));
        }

        Ok(None)
    }

    /// Store a response in the cache.
    pub fn put(&self, key: &str, response: &str, model: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AgenticError::Database("llm cache lock poisoned".into()))?;

        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO llm_cache (cache_key, response, model, created)
             VALUES (?1, ?2, ?3, ?4)",
            params![key, response, model, now],
        )
        .map_err(|e| AgenticError::Database(format!("llm cache insert: {e}")))?;

        Ok(())
    }

    /// Remove entries older than `ttl_secs` seconds.
    pub fn prune(&self, ttl_secs: u64) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AgenticError::Database("llm cache lock poisoned".into()))?;

        let cutoff = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::seconds(ttl_secs as i64))
            .unwrap_or(chrono::Utc::now())
            .to_rfc3339();

        let deleted = conn
            .execute(
                "DELETE FROM llm_cache WHERE created < ?1",
                params![cutoff],
            )
            .map_err(|e| AgenticError::Database(format!("llm cache prune: {e}")))?;

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn tmp_cache() -> Result<(LlmCache, NamedTempFile)> {
        let f = NamedTempFile::new().map_err(AgenticError::Io)?;
        let c = LlmCache::new(f.path())?;
        Ok((c, f))
    }

    #[test]
    fn test_get_miss() -> Result<()> {
        let (cache, _f) = tmp_cache()?;
        assert!(cache.get("nonexistent")?.is_none());
        Ok(())
    }

    #[test]
    fn test_put_and_get() -> Result<()> {
        let (cache, _f) = tmp_cache()?;
        cache.put("k1", "hello world", "gpt-4")?;
        assert_eq!(cache.get("k1")?.as_deref(), Some("hello world"));
        Ok(())
    }

    #[test]
    fn test_compute_key_deterministic() {
        let k1 = LlmCache::compute_key("gpt-4", "[{\"role\":\"user\"}]", "{}");
        let k2 = LlmCache::compute_key("gpt-4", "[{\"role\":\"user\"}]", "{}");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_compute_key_differs_on_model() {
        let k1 = LlmCache::compute_key("gpt-4", "msgs", "opts");
        let k2 = LlmCache::compute_key("gpt-3.5", "msgs", "opts");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_prune_removes_old_entries() -> Result<()> {
        let (cache, _f) = tmp_cache()?;
        cache.put("old", "old response", "gpt-4")?;
        // prune with 0 TTL removes everything
        let deleted = cache.prune(0)?;
        assert!(deleted >= 1);
        assert!(cache.get("old")?.is_none());
        Ok(())
    }
}
