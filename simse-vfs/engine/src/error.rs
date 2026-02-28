use thiserror::Error;

#[derive(Debug, Error)]
pub enum VfsError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Already exists: {0}")]
    AlreadyExists(String),
    #[error("Not a file: {0}")]
    NotAFile(String),
    #[error("Not a directory: {0}")]
    NotADirectory(String),
    #[error("Not empty: {0}")]
    NotEmpty(String),
    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl VfsError {
    pub fn code(&self) -> &str {
        match self {
            Self::InvalidPath(_) => "VFS_INVALID_PATH",
            Self::NotFound(_) => "VFS_NOT_FOUND",
            Self::AlreadyExists(_) => "VFS_ALREADY_EXISTS",
            Self::NotAFile(_) => "VFS_NOT_FILE",
            Self::NotADirectory(_) => "VFS_NOT_DIRECTORY",
            Self::NotEmpty(_) => "VFS_NOT_EMPTY",
            Self::LimitExceeded(_) => "VFS_LIMIT_EXCEEDED",
            Self::InvalidOperation(_) => "VFS_INVALID_OPERATION",
            Self::Io(_) => "VFS_IO_ERROR",
            Self::Json(_) => "VFS_JSON_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "vfsCode": self.code(),
            "message": self.to_string(),
        })
    }
}
