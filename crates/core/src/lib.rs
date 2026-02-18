//! Shared foundation types, error handling, and configuration for agentic-note.
//!
//! Re-exports core domain types, configuration structs, and error handling helpers
//! used by all other crates in the workspace.

pub mod config;
pub mod error;
pub mod id;
pub mod storage;
#[cfg(feature = "postgres")]
pub mod storage_postgres;
pub mod storage_sqlite;
pub mod types;

pub use config::{
    AppConfig, DatabaseConfig, EmbeddingsConfig, EncryptionConfig, IndexerConfig, LlmCacheConfig,
    MetricsConfig, PluginsConfig, SchedulerConfig, SyncConfig, VaultEntry, WasmConfig,
};
pub use error::{AgenticError, Result};
pub use id::next_id;
pub use storage::{Row, StorageBackend};
pub use types::{ConflictPolicy, ErrorPolicy, FrontMatter, NoteId, NoteStatus, ParaCategory};
