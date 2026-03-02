use thiserror::Error;

#[derive(Debug, Error)]
pub enum VshError {
	#[error("Not initialized")]
	NotInitialized,
	#[error("Session not found: {0}")]
	SessionNotFound(String),
	#[error("Execution failed: {0}")]
	ExecutionFailed(String),
	#[error("Command timeout: {0}")]
	Timeout(String),
	#[error("Sandbox violation: {0}")]
	SandboxViolation(String),
	#[error("Invalid params: {0}")]
	InvalidParams(String),
	#[error("Limit exceeded: {0}")]
	LimitExceeded(String),
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("JSON error: {0}")]
	Json(#[from] serde_json::Error),
}

impl VshError {
	pub fn code(&self) -> &str {
		match self {
			Self::NotInitialized => "VSH_NOT_INITIALIZED",
			Self::SessionNotFound(_) => "VSH_SESSION_NOT_FOUND",
			Self::ExecutionFailed(_) => "VSH_EXECUTION_FAILED",
			Self::Timeout(_) => "VSH_TIMEOUT",
			Self::SandboxViolation(_) => "VSH_SANDBOX_VIOLATION",
			Self::InvalidParams(_) => "VSH_INVALID_PARAMS",
			Self::LimitExceeded(_) => "VSH_LIMIT_EXCEEDED",
			Self::Io(_) => "VSH_IO_ERROR",
			Self::Json(_) => "VSH_JSON_ERROR",
		}
	}

	pub fn to_json_rpc_error(&self) -> serde_json::Value {
		serde_json::json!({
			"vshCode": self.code(),
			"message": self.to_string(),
		})
	}
}
