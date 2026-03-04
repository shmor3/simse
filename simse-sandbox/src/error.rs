use simse_vfs_engine::error::VfsError;
use simse_vnet_engine::error::VnetError;
use simse_vsh_engine::error::VshError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("SSH connection error: {0}")]
    SshConnection(String),
    #[error("SSH authentication error: {0}")]
    SshAuth(String),
    #[error("SSH channel error: {0}")]
    SshChannel(String),
    #[error("Backend switch error: {0}")]
    BackendSwitch(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("VFS error: {0}")]
    Vfs(#[from] VfsError),
    #[error("VSH error: {0}")]
    Vsh(#[from] VshError),
    #[error("VNet error: {0}")]
    Vnet(#[from] VnetError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl SandboxError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "SANDBOX_NOT_INITIALIZED",
            Self::SshConnection(_) => "SANDBOX_SSH_CONNECTION",
            Self::SshAuth(_) => "SANDBOX_SSH_AUTH",
            Self::SshChannel(_) => "SANDBOX_SSH_CHANNEL",
            Self::BackendSwitch(_) => "SANDBOX_BACKEND_SWITCH",
            Self::Timeout(_) => "SANDBOX_TIMEOUT",
            Self::InvalidParams(_) => "SANDBOX_INVALID_PARAMS",
            Self::Vfs(e) => e.code(),
            Self::Vsh(e) => e.code(),
            Self::Vnet(e) => e.code(),
            Self::Io(_) => "SANDBOX_IO_ERROR",
            Self::Json(_) => "SANDBOX_JSON_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "sandboxCode": self.code(),
            "message": self.to_string(),
        })
    }
}
