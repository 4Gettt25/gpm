use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("KuzuDB error: {0}")]
    Kuzu(String),

    #[error("Node not found: {0}")]
    NotFound(String),

    #[error("Schema migration failed: {0}")]
    Migration(String),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}
