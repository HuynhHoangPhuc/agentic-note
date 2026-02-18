use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgenticError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("Search error: {0}")]
    Search(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("Batch error: {0}")]
    Batch(String),

    #[error("Metrics error: {0}")]
    Metrics(String),

    #[error("Scheduler error: {0}")]
    Scheduler(String),

    #[error("Indexer error: {0}")]
    Indexer(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Wasm error: {0}")]
    Wasm(String),

    #[error("MultiVault error: {0}")]
    MultiVault(String),
}

pub type Result<T> = std::result::Result<T, AgenticError>;
