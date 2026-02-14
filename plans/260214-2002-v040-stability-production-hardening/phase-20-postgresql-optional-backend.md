# Phase 20: PostgreSQL Optional Backend

## Context Links
- [plan.md](plan.md)
- [crates/core/src/config.rs](/Users/phuc/Developer/agentic-note/crates/core/src/config.rs) — add DatabaseConfig
- [crates/search/src/graph.rs](/Users/phuc/Developer/agentic-note/crates/search/src/graph.rs) — abstract DB trait
- [crates/review/src/queue.rs](/Users/phuc/Developer/agentic-note/crates/review/src/queue.rs) — abstract DB trait
- [Cargo.toml](/Users/phuc/Developer/agentic-note/Cargo.toml) — workspace deps

## Overview
- **Priority:** P1
- **Status:** Complete
- **Implementation Status:** complete
- **Review Status:** complete
- **Effort:** 3h
- **Description:** Add PostgreSQL as optional storage backend for deployments >10k notes. SQLite remains default. Feature flag `postgres` gates the PostgreSQL driver.

## Key Insights
- Current code uses `rusqlite::Connection` directly in `Graph` and `ReviewQueue`
- sqlx AnyPool enables runtime dispatch but compile-time query macros don't work across backends
- Better approach: define `StorageBackend` trait with SQLite impl (default) and Postgres impl behind feature flag
- sqlx built-in connection pool sufficient (no deadpool needed)
- Migrations via `sqlx migrate` embedded at compile time

## Requirements

### Functional
- Define `StorageBackend` async trait in new `crates/core/src/storage.rs`
- SQLite implementation wraps existing rusqlite logic
- PostgreSQL implementation uses sqlx with connection pool
- Config: `[database]` section with `backend = "sqlite" | "postgres"`, `url` field
- Migration files for both backends (identical schema)

### Non-Functional
- Zero overhead for SQLite-only builds (feature-gated)
- Connection pooling for Postgres (sqlx default pool)
- Graceful error on missing `postgres` feature when config requests it

## Architecture

```
AppConfig.database.backend
    |
    +-- "sqlite" --> SqliteBackend (rusqlite, default)
    |
    +-- "postgres" --> PostgresBackend (sqlx, feature = "postgres")
```

Trait definition:
```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn execute(&self, sql: &str, params: &[&str]) -> Result<()>;
    async fn query_rows(&self, sql: &str, params: &[&str]) -> Result<Vec<Row>>;
    async fn query_one(&self, sql: &str, params: &[&str]) -> Result<Row>;
}
```

Graph and ReviewQueue refactored to accept `Arc<dyn StorageBackend>` instead of raw `rusqlite::Connection`.

## Related Code Files

### Modify
- `Cargo.toml` — add sqlx workspace dep with `postgres` feature
- `crates/core/Cargo.toml` — add sqlx dep
- `crates/core/src/config.rs` — add `DatabaseConfig` struct
- `crates/core/src/error.rs` — add `Database(String)` variant
- `crates/core/src/lib.rs` — export storage module
- `crates/search/src/graph.rs` — refactor to use StorageBackend trait
- `crates/search/Cargo.toml` — conditional sqlx dep
- `crates/review/src/queue.rs` — refactor to use StorageBackend trait
- `crates/review/Cargo.toml` — conditional sqlx dep
- `crates/cli/src/main.rs` — initialize correct backend from config

### Create
- `crates/core/src/storage.rs` — StorageBackend trait + Row type
- `crates/core/src/storage_sqlite.rs` — SQLite impl (wraps rusqlite)
- `crates/core/src/storage_postgres.rs` — Postgres impl (behind feature flag)
- `migrations/sqlite/001_init.sql` — SQLite migration
- `migrations/postgres/001_init.sql` — Postgres migration

## Implementation Steps

1. Add `DatabaseConfig` to `crates/core/src/config.rs`:
   ```rust
   pub struct DatabaseConfig {
       pub backend: String,      // "sqlite" | "postgres"
       pub url: Option<String>,  // postgres connection URL
       pub max_connections: u32, // pool size, default 5
   }
   ```
2. Add `Database(String)` error variant to `error.rs`
3. Create `storage.rs` with `StorageBackend` trait and `Row` abstraction
4. Create `storage_sqlite.rs` wrapping existing rusqlite calls
5. Create `storage_postgres.rs` behind `#[cfg(feature = "postgres")]`
6. Add sqlx to workspace Cargo.toml with features
7. Refactor `Graph` to accept `Arc<dyn StorageBackend>` — keep same public API
8. Refactor `ReviewQueue` to accept `Arc<dyn StorageBackend>`
9. Create migration SQL files (same schema, dialect-adjusted)
10. Update CLI main.rs to instantiate backend from config
11. Add tests for both backends (Postgres tests behind feature flag)

## Todo List
- [x]Add DatabaseConfig to config.rs
- [x]Add Database error variant
- [x]Create StorageBackend trait in storage.rs
- [x]Implement SqliteBackend
- [x]Implement PostgresBackend (feature-gated)
- [x]Refactor Graph to use trait
- [x]Refactor ReviewQueue to use trait
- [x]Create migration files
- [x]Update CLI initialization
- [x]Add unit tests
- [x]Run `cargo build` and `cargo build --features postgres`

## Success Criteria
- `cargo build` compiles with SQLite only (no sqlx postgres driver)
- `cargo build --features postgres` compiles with both
- Existing SQLite tests pass unchanged
- Graph and ReviewQueue work with both backends
- Config validation rejects `backend = "postgres"` without feature flag

## Risk Assessment
- **sqlx compile times**: sqlx adds ~30s to clean builds. Mitigated by feature flag.
- **Schema drift**: Two migration sets could diverge. Mitigated by shared SQL (only type differences).
- **Breaking change**: Graph/ReviewQueue API changes. Mitigated by keeping same public method signatures.

## Security Considerations
- Postgres connection URL may contain credentials; treat like API keys (0600 perms on config)
- Use `sslmode=require` default for Postgres connections
- Connection pool prevents connection exhaustion

## Next Steps
- Phase 21 (Batch LLM) is independent
- Future: consider sqlx for CAS blob store if needed
