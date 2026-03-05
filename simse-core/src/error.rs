//! Unified error system for simse-core.
//!
//! Replaces 13 separate TypeScript error files with a single `SimseError` enum
//! that covers all domains: config, provider, chain, template, MCP, library,
//! loop, resilience, task, tool, VFS, plus passthrough variants for each engine
//! crate error.

use std::fmt;

// ---------------------------------------------------------------------------
// Error code enums
// ---------------------------------------------------------------------------

/// Error codes for configuration errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigErrorCode {
	InvalidField,
	MissingRequired,
	ValidationFailed,
}

impl ConfigErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::InvalidField => "CONFIG_INVALID_FIELD",
			Self::MissingRequired => "CONFIG_MISSING_REQUIRED",
			Self::ValidationFailed => "CONFIG_VALIDATION_FAILED",
		}
	}
}

impl fmt::Display for ConfigErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for AI provider errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderErrorCode {
	Timeout,
	Unavailable,
	AuthFailed,
	RateLimited,
	HttpError,
}

impl ProviderErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::Timeout => "PROVIDER_TIMEOUT",
			Self::Unavailable => "PROVIDER_UNAVAILABLE",
			Self::AuthFailed => "PROVIDER_AUTH_FAILED",
			Self::RateLimited => "PROVIDER_RATE_LIMITED",
			Self::HttpError => "PROVIDER_HTTP_ERROR",
		}
	}
}

impl fmt::Display for ProviderErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for chain execution errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainErrorCode {
	Empty,
	StepFailed,
	InvalidStep,
	McpNotConfigured,
	ExecutionFailed,
	McpToolError,
	NotFound,
}

impl ChainErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::Empty => "CHAIN_EMPTY",
			Self::StepFailed => "CHAIN_STEP_FAILED",
			Self::InvalidStep => "CHAIN_INVALID_STEP",
			Self::McpNotConfigured => "CHAIN_MCP_NOT_CONFIGURED",
			Self::ExecutionFailed => "CHAIN_EXECUTION_FAILED",
			Self::McpToolError => "CHAIN_MCP_TOOL_ERROR",
			Self::NotFound => "CHAIN_NOT_FOUND",
		}
	}
}

impl fmt::Display for ChainErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for prompt template errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateErrorCode {
	Empty,
	MissingVariables,
	InvalidValue,
}

impl TemplateErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::Empty => "TEMPLATE_EMPTY",
			Self::MissingVariables => "TEMPLATE_MISSING_VARIABLES",
			Self::InvalidValue => "TEMPLATE_INVALID_VALUE",
		}
	}
}

impl fmt::Display for TemplateErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for MCP protocol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpErrorCode {
	ConnectionError,
	ToolError,
	ResourceError,
	ServerError,
}

impl McpErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::ConnectionError => "MCP_CONNECTION_ERROR",
			Self::ToolError => "MCP_TOOL_ERROR",
			Self::ResourceError => "MCP_RESOURCE_ERROR",
			Self::ServerError => "MCP_SERVER_ERROR",
		}
	}
}

impl fmt::Display for McpErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for library (vector store) errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryErrorCode {
	EmptyText,
	EmbeddingFailed,
	NotInitialized,
	DuplicateDetected,
	NotFound,
	InvalidInput,
}

impl LibraryErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::EmptyText => "LIBRARY_EMPTY_TEXT",
			Self::EmbeddingFailed => "LIBRARY_EMBEDDING_FAILED",
			Self::NotInitialized => "LIBRARY_NOT_INITIALIZED",
			Self::DuplicateDetected => "LIBRARY_DUPLICATE_DETECTED",
			Self::NotFound => "LIBRARY_NOT_FOUND",
			Self::InvalidInput => "LIBRARY_INVALID_INPUT",
		}
	}
}

impl fmt::Display for LibraryErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for agentic loop errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopErrorCode {
	DoomLoop,
	TurnLimit,
	Aborted,
	CompactionFailed,
}

impl LoopErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::DoomLoop => "LOOP_DOOM_LOOP",
			Self::TurnLimit => "LOOP_TURN_LIMIT",
			Self::Aborted => "LOOP_ABORTED",
			Self::CompactionFailed => "LOOP_COMPACTION_FAILED",
		}
	}
}

impl fmt::Display for LoopErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for resilience (circuit breaker, retry, timeout) errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResilienceErrorCode {
	CircuitOpen,
	Timeout,
	RetryExhausted,
	RetryAborted,
}

impl ResilienceErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::CircuitOpen => "RESILIENCE_CIRCUIT_OPEN",
			Self::Timeout => "RESILIENCE_TIMEOUT",
			Self::RetryExhausted => "RESILIENCE_RETRY_EXHAUSTED",
			Self::RetryAborted => "RESILIENCE_RETRY_ABORTED",
		}
	}
}

impl fmt::Display for ResilienceErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for task list errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskErrorCode {
	NotFound,
	LimitReached,
	CircularDependency,
	InvalidStatus,
}

impl TaskErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::NotFound => "TASK_NOT_FOUND",
			Self::LimitReached => "TASK_LIMIT_REACHED",
			Self::CircularDependency => "TASK_CIRCULAR_DEPENDENCY",
			Self::InvalidStatus => "TASK_INVALID_STATUS",
		}
	}
}

impl fmt::Display for TaskErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for tool registry errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolErrorCode {
	NotFound,
	ExecutionFailed,
	PermissionDenied,
	Timeout,
	ParseError,
}

impl ToolErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::NotFound => "TOOL_NOT_FOUND",
			Self::ExecutionFailed => "TOOL_EXECUTION_FAILED",
			Self::PermissionDenied => "TOOL_PERMISSION_DENIED",
			Self::Timeout => "TOOL_TIMEOUT",
			Self::ParseError => "TOOL_PARSE_ERROR",
		}
	}
}

impl fmt::Display for ToolErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// Error codes for virtual filesystem errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VfsErrorCode {
	InvalidPath,
	NotFound,
	AlreadyExists,
	LimitExceeded,
}

impl VfsErrorCode {
	pub fn as_str(&self) -> &str {
		match self {
			Self::InvalidPath => "VFS_INVALID_PATH",
			Self::NotFound => "VFS_NOT_FOUND",
			Self::AlreadyExists => "VFS_ALREADY_EXISTS",
			Self::LimitExceeded => "VFS_LIMIT_EXCEEDED",
		}
	}
}

impl fmt::Display for VfsErrorCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

// ---------------------------------------------------------------------------
// Unified SimseError
// ---------------------------------------------------------------------------

/// Unified error type spanning all simse domains.
///
/// Each domain variant carries a typed error code enum and a human-readable
/// message. Engine crate errors pass through via `#[from]` conversions.
#[derive(Debug, thiserror::Error)]
pub enum SimseError {
	#[error("config error: {message}")]
	Config {
		code: ConfigErrorCode,
		message: String,
	},

	#[error("provider error: {message}")]
	Provider {
		code: ProviderErrorCode,
		message: String,
		status: Option<u16>,
	},

	#[error("chain error: {message}")]
	Chain {
		code: ChainErrorCode,
		message: String,
	},

	#[error("template error: {message}")]
	Template {
		code: TemplateErrorCode,
		message: String,
	},

	#[error("MCP error: {message}")]
	Mcp {
		code: McpErrorCode,
		message: String,
	},

	#[error("library error: {message}")]
	Library {
		code: LibraryErrorCode,
		message: String,
	},

	#[error("loop error: {message}")]
	Loop {
		code: LoopErrorCode,
		message: String,
	},

	#[error("resilience error: {message}")]
	Resilience {
		code: ResilienceErrorCode,
		message: String,
	},

	#[error("task error: {message}")]
	Task {
		code: TaskErrorCode,
		message: String,
	},

	#[error("tool error: {message}")]
	Tool {
		code: ToolErrorCode,
		message: String,
	},

	#[error("VFS error: {message}")]
	Vfs {
		code: VfsErrorCode,
		message: String,
	},

	// Passthrough from engine crates
	#[error(transparent)]
	Acp(#[from] simse_acp_engine::error::AcpError),

	#[error(transparent)]
	McpEngine(#[from] simse_mcp_engine::error::McpError),

	#[error(transparent)]
	Vector(#[from] simse_adaptive_engine::error::VectorError),

	#[error(transparent)]
	VfsEngine(#[from] simse_vfs_engine::error::VfsError),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("{0}")]
	Other(String),
}

impl SimseError {
	/// Returns a static error code string for the error variant.
	///
	/// Domain error codes follow the pattern `DOMAIN_CODE` (e.g.,
	/// `CONFIG_INVALID_FIELD`). Engine passthrough variants return a generic
	/// code like `ACP_ERROR`. Use the inner engine error's `.code()` method
	/// for finer-grained codes.
	pub fn code(&self) -> &str {
		match self {
			Self::Config { code, .. } => code.as_str(),
			Self::Provider { code, .. } => code.as_str(),
			Self::Chain { code, .. } => code.as_str(),
			Self::Template { code, .. } => code.as_str(),
			Self::Mcp { code, .. } => code.as_str(),
			Self::Library { code, .. } => code.as_str(),
			Self::Loop { code, .. } => code.as_str(),
			Self::Resilience { code, .. } => code.as_str(),
			Self::Task { code, .. } => code.as_str(),
			Self::Tool { code, .. } => code.as_str(),
			Self::Vfs { code, .. } => code.as_str(),
			Self::Acp(_) => "ACP_ERROR",
			Self::McpEngine(_) => "MCP_ENGINE_ERROR",
			Self::Vector(_) => "VECTOR_ERROR",
			Self::VfsEngine(_) => "VFS_ENGINE_ERROR",
			Self::Io(_) => "IO_ERROR",
			Self::Other(_) => "OTHER_ERROR",
		}
	}

	/// Returns `true` if this error is retriable (timeouts, rate limits, etc.).
	pub fn is_retriable(&self) -> bool {
		matches!(
			self,
			Self::Provider {
				code: ProviderErrorCode::Timeout
					| ProviderErrorCode::Unavailable
					| ProviderErrorCode::RateLimited,
				..
			} | Self::Resilience {
				code: ResilienceErrorCode::Timeout,
				..
			} | Self::Tool {
				code: ToolErrorCode::Timeout,
				..
			}
		)
	}

	/// Returns the HTTP status code if this is a provider error with one.
	pub fn http_status(&self) -> Option<u16> {
		match self {
			Self::Provider { status, .. } => *status,
			_ => None,
		}
	}
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl SimseError {
	pub fn config(code: ConfigErrorCode, message: impl Into<String>) -> Self {
		Self::Config {
			code,
			message: message.into(),
		}
	}

	pub fn provider(
		code: ProviderErrorCode,
		message: impl Into<String>,
		status: Option<u16>,
	) -> Self {
		Self::Provider {
			code,
			message: message.into(),
			status,
		}
	}

	pub fn chain(code: ChainErrorCode, message: impl Into<String>) -> Self {
		Self::Chain {
			code,
			message: message.into(),
		}
	}

	pub fn template(code: TemplateErrorCode, message: impl Into<String>) -> Self {
		Self::Template {
			code,
			message: message.into(),
		}
	}

	pub fn mcp(code: McpErrorCode, message: impl Into<String>) -> Self {
		Self::Mcp {
			code,
			message: message.into(),
		}
	}

	pub fn library(code: LibraryErrorCode, message: impl Into<String>) -> Self {
		Self::Library {
			code,
			message: message.into(),
		}
	}

	pub fn loop_err(code: LoopErrorCode, message: impl Into<String>) -> Self {
		Self::Loop {
			code,
			message: message.into(),
		}
	}

	pub fn resilience(code: ResilienceErrorCode, message: impl Into<String>) -> Self {
		Self::Resilience {
			code,
			message: message.into(),
		}
	}

	pub fn task(code: TaskErrorCode, message: impl Into<String>) -> Self {
		Self::Task {
			code,
			message: message.into(),
		}
	}

	pub fn tool(code: ToolErrorCode, message: impl Into<String>) -> Self {
		Self::Tool {
			code,
			message: message.into(),
		}
	}

	pub fn vfs(code: VfsErrorCode, message: impl Into<String>) -> Self {
		Self::Vfs {
			code,
			message: message.into(),
		}
	}

	pub fn other(message: impl Into<String>) -> Self {
		Self::Other(message.into())
	}
}

// ---------------------------------------------------------------------------
// JSON-RPC error conversion
// ---------------------------------------------------------------------------

impl SimseError {
	/// Convert to a JSON-RPC error data payload with a `coreCode` key.
	pub fn to_json_rpc_error(&self) -> serde_json::Value {
		serde_json::json!({ "coreCode": self.code() })
	}
}

/// Convenience type alias for Results using `SimseError`.
pub type Result<T> = std::result::Result<T, SimseError>;
