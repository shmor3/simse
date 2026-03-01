use thiserror::Error;

#[derive(Debug, Error)]
pub enum AcpError {
	#[error("Client not initialized: call initialize first")]
	NotInitialized,
	#[error("Connection failed: {0}")]
	ConnectionFailed(String),
	#[error("Session error: {0}")]
	SessionError(String),
	#[error("Timeout: {method} exceeded {timeout_ms}ms")]
	Timeout { method: String, timeout_ms: u64 },
	#[error("Stream error: {0}")]
	StreamError(String),
	#[error("Permission denied: {0}")]
	PermissionDenied(String),
	#[error("Circuit breaker open: {0}")]
	CircuitBreakerOpen(String),
	#[error("Server unavailable: {0}")]
	ServerUnavailable(String),
	#[error("Protocol error: {0}")]
	ProtocolError(String),
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Serialization error: {0}")]
	Serialization(String),
}

impl AcpError {
	pub fn code(&self) -> &str {
		match self {
			Self::NotInitialized => "ACP_NOT_INITIALIZED",
			Self::ConnectionFailed(_) => "ACP_CONNECTION_FAILED",
			Self::SessionError(_) => "ACP_SESSION_ERROR",
			Self::Timeout { .. } => "ACP_TIMEOUT",
			Self::StreamError(_) => "ACP_STREAM_ERROR",
			Self::PermissionDenied(_) => "ACP_PERMISSION_DENIED",
			Self::CircuitBreakerOpen(_) => "ACP_CIRCUIT_BREAKER_OPEN",
			Self::ServerUnavailable(_) => "ACP_SERVER_UNAVAILABLE",
			Self::ProtocolError(_) => "ACP_PROTOCOL_ERROR",
			Self::Io(_) => "ACP_IO",
			Self::Serialization(_) => "ACP_SERIALIZATION",
		}
	}

	pub fn to_json_rpc_error(&self) -> serde_json::Value {
		serde_json::json!({
			"acpCode": self.code(),
			"message": self.to_string(),
		})
	}
}
