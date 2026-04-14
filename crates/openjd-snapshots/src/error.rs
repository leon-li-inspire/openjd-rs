use thiserror::Error;

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Manifest validation error: {0}")]
    Validation(String),
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Operation cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SnapshotError>;
