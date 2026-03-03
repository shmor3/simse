use thiserror::Error;

#[derive(Debug, Error)]
pub enum VnetError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
