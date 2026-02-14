pub mod config;
pub mod error;
pub mod id;
pub mod types;

pub use config::{AppConfig, EmbeddingsConfig, PluginsConfig, SyncConfig};
pub use error::{AgenticError, Result};
pub use id::next_id;
pub use types::{ConflictPolicy, ErrorPolicy, FrontMatter, NoteId, NoteStatus, ParaCategory};
