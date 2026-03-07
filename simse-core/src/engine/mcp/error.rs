use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
	#[error("Client/server not initialized: call initialize first")]
	NotInitialized,
	#[error("Connection failed: {0}")]
	ConnectionFailed(String),
	#[error("Server not connected: {0}")]
	ServerNotConnected(String),
	#[error("Tool error [{tool}]: {message}")]
	ToolError { tool: String, message: String },
	#[error("Resource error [{uri}]: {message}")]
	ResourceError { uri: String, message: String },
	#[error("Transport configuration error: {0}")]
	TransportConfigError(String),
	#[error("Timeout: {method} exceeded {timeout_ms}ms")]
	Timeout { method: String, timeout_ms: u64 },
	#[error("Circuit breaker open: {0}")]
	CircuitBreakerOpen(String),
	#[error("Protocol error: {0}")]
	ProtocolError(String),
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Serialization error: {0}")]
	Serialization(String),
}

impl McpError {
	pub fn code(&self) -> &str {
		match self {
			Self::NotInitialized => "MCP_NOT_INITIALIZED",
			Self::ConnectionFailed(_) => "MCP_CONNECTION_FAILED",
			Self::ServerNotConnected(_) => "MCP_SERVER_NOT_CONNECTED",
			Self::ToolError { .. } => "MCP_TOOL_ERROR",
			Self::ResourceError { .. } => "MCP_RESOURCE_ERROR",
			Self::TransportConfigError(_) => "MCP_TRANSPORT_CONFIG_ERROR",
			Self::Timeout { .. } => "MCP_TIMEOUT",
			Self::CircuitBreakerOpen(_) => "MCP_CIRCUIT_BREAKER_OPEN",
			Self::ProtocolError(_) => "MCP_PROTOCOL_ERROR",
			Self::Io(_) => "MCP_IO",
			Self::Serialization(_) => "MCP_SERIALIZATION",
		}
	}

	pub fn to_json_rpc_error(&self) -> serde_json::Value {
		serde_json::json!({
			"mcpCode": self.code(),
			"message": self.to_string(),
		})
	}
}
