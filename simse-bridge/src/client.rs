//! JSON-RPC client that communicates with the TS core subprocess.

use thiserror::Error;

use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("Failed to spawn bridge process: {0}")]
    SpawnFailed(String),
    #[error("Bridge process exited unexpectedly")]
    ProcessExited,
    #[error("JSON-RPC error {code}: {message}")]
    RpcError { code: i64, message: String },
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Request timed out")]
    Timeout,
}

/// Bridge client configuration.
pub struct BridgeConfig {
    pub command: String,       // e.g. "bun"
    pub args: Vec<String>,     // e.g. ["run", "bridge-server.ts"]
    pub data_dir: String,
    pub timeout_ms: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            command: "bun".into(),
            args: vec!["run".into(), "bridge-server.ts".into()],
            data_dir: String::new(),
            timeout_ms: 60_000,
        }
    }
}
