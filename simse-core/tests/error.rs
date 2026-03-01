//! Tests for the unified error system.

use simse_core::error::*;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Error code uniqueness
// ---------------------------------------------------------------------------

#[test]
fn all_error_code_strings_are_unique() {
	let codes: Vec<&str> = vec![
		// ConfigErrorCode
		ConfigErrorCode::InvalidField.as_str(),
		ConfigErrorCode::MissingRequired.as_str(),
		ConfigErrorCode::ValidationFailed.as_str(),
		// ProviderErrorCode
		ProviderErrorCode::Timeout.as_str(),
		ProviderErrorCode::Unavailable.as_str(),
		ProviderErrorCode::AuthFailed.as_str(),
		ProviderErrorCode::RateLimited.as_str(),
		ProviderErrorCode::HttpError.as_str(),
		// ChainErrorCode
		ChainErrorCode::Empty.as_str(),
		ChainErrorCode::StepFailed.as_str(),
		ChainErrorCode::InvalidStep.as_str(),
		ChainErrorCode::McpNotConfigured.as_str(),
		ChainErrorCode::ExecutionFailed.as_str(),
		ChainErrorCode::McpToolError.as_str(),
		ChainErrorCode::NotFound.as_str(),
		// TemplateErrorCode
		TemplateErrorCode::Empty.as_str(),
		TemplateErrorCode::MissingVariables.as_str(),
		TemplateErrorCode::InvalidValue.as_str(),
		// McpErrorCode
		McpErrorCode::ConnectionError.as_str(),
		McpErrorCode::ToolError.as_str(),
		McpErrorCode::ResourceError.as_str(),
		McpErrorCode::ServerError.as_str(),
		// LibraryErrorCode
		LibraryErrorCode::EmptyText.as_str(),
		LibraryErrorCode::EmbeddingFailed.as_str(),
		LibraryErrorCode::NotInitialized.as_str(),
		LibraryErrorCode::DuplicateDetected.as_str(),
		// LoopErrorCode
		LoopErrorCode::DoomLoop.as_str(),
		LoopErrorCode::TurnLimit.as_str(),
		LoopErrorCode::Aborted.as_str(),
		LoopErrorCode::CompactionFailed.as_str(),
		// ResilienceErrorCode
		ResilienceErrorCode::CircuitOpen.as_str(),
		ResilienceErrorCode::Timeout.as_str(),
		ResilienceErrorCode::RetryExhausted.as_str(),
		ResilienceErrorCode::RetryAborted.as_str(),
		// TaskErrorCode
		TaskErrorCode::NotFound.as_str(),
		TaskErrorCode::LimitReached.as_str(),
		TaskErrorCode::CircularDependency.as_str(),
		TaskErrorCode::InvalidStatus.as_str(),
		// ToolErrorCode
		ToolErrorCode::NotFound.as_str(),
		ToolErrorCode::ExecutionFailed.as_str(),
		ToolErrorCode::PermissionDenied.as_str(),
		ToolErrorCode::Timeout.as_str(),
		ToolErrorCode::ParseError.as_str(),
		// VfsErrorCode
		VfsErrorCode::InvalidPath.as_str(),
		VfsErrorCode::NotFound.as_str(),
		VfsErrorCode::AlreadyExists.as_str(),
		VfsErrorCode::LimitExceeded.as_str(),
		// Passthrough codes
		"ACP_ERROR",
		"MCP_ENGINE_ERROR",
		"VECTOR_ERROR",
		"VFS_ENGINE_ERROR",
		"IO_ERROR",
		"OTHER_ERROR",
	];

	let unique: HashSet<&str> = codes.iter().copied().collect();
	assert_eq!(
		codes.len(),
		unique.len(),
		"duplicate error code strings detected: {} total vs {} unique",
		codes.len(),
		unique.len()
	);
}

// ---------------------------------------------------------------------------
// Display impls for each domain variant
// ---------------------------------------------------------------------------

#[test]
fn display_config_error() {
	let err = SimseError::config(ConfigErrorCode::InvalidField, "bad field 'name'");
	assert_eq!(err.to_string(), "config error: bad field 'name'");
}

#[test]
fn display_provider_error() {
	let err = SimseError::provider(ProviderErrorCode::Timeout, "timed out after 30s", None);
	assert_eq!(err.to_string(), "provider error: timed out after 30s");
}

#[test]
fn display_provider_error_with_status() {
	let err = SimseError::provider(
		ProviderErrorCode::HttpError,
		"server error",
		Some(500),
	);
	assert_eq!(err.to_string(), "provider error: server error");
	assert_eq!(err.http_status(), Some(500));
}

#[test]
fn display_chain_error() {
	let err = SimseError::chain(ChainErrorCode::Empty, "chain has no steps");
	assert_eq!(err.to_string(), "chain error: chain has no steps");
}

#[test]
fn display_template_error() {
	let err = SimseError::template(TemplateErrorCode::MissingVariables, "missing {name}");
	assert_eq!(err.to_string(), "template error: missing {name}");
}

#[test]
fn display_mcp_error() {
	let err = SimseError::mcp(McpErrorCode::ConnectionError, "refused");
	assert_eq!(err.to_string(), "MCP error: refused");
}

#[test]
fn display_library_error() {
	let err = SimseError::library(LibraryErrorCode::EmptyText, "empty");
	assert_eq!(err.to_string(), "library error: empty");
}

#[test]
fn display_loop_error() {
	let err = SimseError::loop_err(LoopErrorCode::DoomLoop, "stuck");
	assert_eq!(err.to_string(), "loop error: stuck");
}

#[test]
fn display_resilience_error() {
	let err = SimseError::resilience(ResilienceErrorCode::CircuitOpen, "open");
	assert_eq!(err.to_string(), "resilience error: open");
}

#[test]
fn display_task_error() {
	let err = SimseError::task(TaskErrorCode::NotFound, "no such task");
	assert_eq!(err.to_string(), "task error: no such task");
}

#[test]
fn display_tool_error() {
	let err = SimseError::tool(ToolErrorCode::ExecutionFailed, "crashed");
	assert_eq!(err.to_string(), "tool error: crashed");
}

#[test]
fn display_vfs_error() {
	let err = SimseError::vfs(VfsErrorCode::NotFound, "/missing");
	assert_eq!(err.to_string(), "VFS error: /missing");
}

#[test]
fn display_io_error() {
	let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file gone");
	let err: SimseError = io_err.into();
	assert_eq!(err.to_string(), "IO error: file gone");
}

#[test]
fn display_other_error() {
	let err = SimseError::other("something unexpected");
	assert_eq!(err.to_string(), "something unexpected");
}

// ---------------------------------------------------------------------------
// code() returns correct string for each variant
// ---------------------------------------------------------------------------

#[test]
fn code_config_variants() {
	assert_eq!(
		SimseError::config(ConfigErrorCode::InvalidField, "x").code(),
		"CONFIG_INVALID_FIELD"
	);
	assert_eq!(
		SimseError::config(ConfigErrorCode::MissingRequired, "x").code(),
		"CONFIG_MISSING_REQUIRED"
	);
	assert_eq!(
		SimseError::config(ConfigErrorCode::ValidationFailed, "x").code(),
		"CONFIG_VALIDATION_FAILED"
	);
}

#[test]
fn code_provider_variants() {
	assert_eq!(
		SimseError::provider(ProviderErrorCode::Timeout, "x", None).code(),
		"PROVIDER_TIMEOUT"
	);
	assert_eq!(
		SimseError::provider(ProviderErrorCode::Unavailable, "x", None).code(),
		"PROVIDER_UNAVAILABLE"
	);
	assert_eq!(
		SimseError::provider(ProviderErrorCode::AuthFailed, "x", None).code(),
		"PROVIDER_AUTH_FAILED"
	);
	assert_eq!(
		SimseError::provider(ProviderErrorCode::RateLimited, "x", None).code(),
		"PROVIDER_RATE_LIMITED"
	);
	assert_eq!(
		SimseError::provider(ProviderErrorCode::HttpError, "x", Some(404)).code(),
		"PROVIDER_HTTP_ERROR"
	);
}

#[test]
fn code_chain_variants() {
	assert_eq!(
		SimseError::chain(ChainErrorCode::Empty, "x").code(),
		"CHAIN_EMPTY"
	);
	assert_eq!(
		SimseError::chain(ChainErrorCode::StepFailed, "x").code(),
		"CHAIN_STEP_FAILED"
	);
	assert_eq!(
		SimseError::chain(ChainErrorCode::InvalidStep, "x").code(),
		"CHAIN_INVALID_STEP"
	);
	assert_eq!(
		SimseError::chain(ChainErrorCode::McpNotConfigured, "x").code(),
		"CHAIN_MCP_NOT_CONFIGURED"
	);
	assert_eq!(
		SimseError::chain(ChainErrorCode::ExecutionFailed, "x").code(),
		"CHAIN_EXECUTION_FAILED"
	);
	assert_eq!(
		SimseError::chain(ChainErrorCode::McpToolError, "x").code(),
		"CHAIN_MCP_TOOL_ERROR"
	);
	assert_eq!(
		SimseError::chain(ChainErrorCode::NotFound, "x").code(),
		"CHAIN_NOT_FOUND"
	);
}

#[test]
fn code_template_variants() {
	assert_eq!(
		SimseError::template(TemplateErrorCode::Empty, "x").code(),
		"TEMPLATE_EMPTY"
	);
	assert_eq!(
		SimseError::template(TemplateErrorCode::MissingVariables, "x").code(),
		"TEMPLATE_MISSING_VARIABLES"
	);
	assert_eq!(
		SimseError::template(TemplateErrorCode::InvalidValue, "x").code(),
		"TEMPLATE_INVALID_VALUE"
	);
}

#[test]
fn code_mcp_variants() {
	assert_eq!(
		SimseError::mcp(McpErrorCode::ConnectionError, "x").code(),
		"MCP_CONNECTION_ERROR"
	);
	assert_eq!(
		SimseError::mcp(McpErrorCode::ToolError, "x").code(),
		"MCP_TOOL_ERROR"
	);
	assert_eq!(
		SimseError::mcp(McpErrorCode::ResourceError, "x").code(),
		"MCP_RESOURCE_ERROR"
	);
	assert_eq!(
		SimseError::mcp(McpErrorCode::ServerError, "x").code(),
		"MCP_SERVER_ERROR"
	);
}

#[test]
fn code_library_variants() {
	assert_eq!(
		SimseError::library(LibraryErrorCode::EmptyText, "x").code(),
		"LIBRARY_EMPTY_TEXT"
	);
	assert_eq!(
		SimseError::library(LibraryErrorCode::EmbeddingFailed, "x").code(),
		"LIBRARY_EMBEDDING_FAILED"
	);
	assert_eq!(
		SimseError::library(LibraryErrorCode::NotInitialized, "x").code(),
		"LIBRARY_NOT_INITIALIZED"
	);
	assert_eq!(
		SimseError::library(LibraryErrorCode::DuplicateDetected, "x").code(),
		"LIBRARY_DUPLICATE_DETECTED"
	);
}

#[test]
fn code_loop_variants() {
	assert_eq!(
		SimseError::loop_err(LoopErrorCode::DoomLoop, "x").code(),
		"LOOP_DOOM_LOOP"
	);
	assert_eq!(
		SimseError::loop_err(LoopErrorCode::TurnLimit, "x").code(),
		"LOOP_TURN_LIMIT"
	);
	assert_eq!(
		SimseError::loop_err(LoopErrorCode::Aborted, "x").code(),
		"LOOP_ABORTED"
	);
	assert_eq!(
		SimseError::loop_err(LoopErrorCode::CompactionFailed, "x").code(),
		"LOOP_COMPACTION_FAILED"
	);
}

#[test]
fn code_resilience_variants() {
	assert_eq!(
		SimseError::resilience(ResilienceErrorCode::CircuitOpen, "x").code(),
		"RESILIENCE_CIRCUIT_OPEN"
	);
	assert_eq!(
		SimseError::resilience(ResilienceErrorCode::Timeout, "x").code(),
		"RESILIENCE_TIMEOUT"
	);
	assert_eq!(
		SimseError::resilience(ResilienceErrorCode::RetryExhausted, "x").code(),
		"RESILIENCE_RETRY_EXHAUSTED"
	);
	assert_eq!(
		SimseError::resilience(ResilienceErrorCode::RetryAborted, "x").code(),
		"RESILIENCE_RETRY_ABORTED"
	);
}

#[test]
fn code_task_variants() {
	assert_eq!(
		SimseError::task(TaskErrorCode::NotFound, "x").code(),
		"TASK_NOT_FOUND"
	);
	assert_eq!(
		SimseError::task(TaskErrorCode::LimitReached, "x").code(),
		"TASK_LIMIT_REACHED"
	);
	assert_eq!(
		SimseError::task(TaskErrorCode::CircularDependency, "x").code(),
		"TASK_CIRCULAR_DEPENDENCY"
	);
	assert_eq!(
		SimseError::task(TaskErrorCode::InvalidStatus, "x").code(),
		"TASK_INVALID_STATUS"
	);
}

#[test]
fn code_tool_variants() {
	assert_eq!(
		SimseError::tool(ToolErrorCode::NotFound, "x").code(),
		"TOOL_NOT_FOUND"
	);
	assert_eq!(
		SimseError::tool(ToolErrorCode::ExecutionFailed, "x").code(),
		"TOOL_EXECUTION_FAILED"
	);
	assert_eq!(
		SimseError::tool(ToolErrorCode::PermissionDenied, "x").code(),
		"TOOL_PERMISSION_DENIED"
	);
	assert_eq!(
		SimseError::tool(ToolErrorCode::Timeout, "x").code(),
		"TOOL_TIMEOUT"
	);
	assert_eq!(
		SimseError::tool(ToolErrorCode::ParseError, "x").code(),
		"TOOL_PARSE_ERROR"
	);
}

#[test]
fn code_vfs_variants() {
	assert_eq!(
		SimseError::vfs(VfsErrorCode::InvalidPath, "x").code(),
		"VFS_INVALID_PATH"
	);
	assert_eq!(
		SimseError::vfs(VfsErrorCode::NotFound, "x").code(),
		"VFS_NOT_FOUND"
	);
	assert_eq!(
		SimseError::vfs(VfsErrorCode::AlreadyExists, "x").code(),
		"VFS_ALREADY_EXISTS"
	);
	assert_eq!(
		SimseError::vfs(VfsErrorCode::LimitExceeded, "x").code(),
		"VFS_LIMIT_EXCEEDED"
	);
}

#[test]
fn code_io_variant() {
	let io_err = std::io::Error::new(std::io::ErrorKind::Other, "fail");
	let err: SimseError = io_err.into();
	assert_eq!(err.code(), "IO_ERROR");
}

#[test]
fn code_other_variant() {
	let err = SimseError::other("oops");
	assert_eq!(err.code(), "OTHER_ERROR");
}

// ---------------------------------------------------------------------------
// #[from] conversion from engine crate errors
// ---------------------------------------------------------------------------

#[test]
fn from_acp_error() {
	let acp_err = simse_acp_engine::error::AcpError::NotInitialized;
	let err: SimseError = acp_err.into();
	assert_eq!(err.code(), "ACP_ERROR");
	assert!(
		err.to_string().contains("not initialized"),
		"expected 'not initialized' in: {}",
		err
	);
}

#[test]
fn from_mcp_error() {
	let mcp_err =
		simse_mcp_engine::error::McpError::ConnectionFailed("refused".into());
	let err: SimseError = mcp_err.into();
	assert_eq!(err.code(), "MCP_ENGINE_ERROR");
	assert!(
		err.to_string().contains("refused"),
		"expected 'refused' in: {}",
		err
	);
}

#[test]
fn from_vector_error() {
	let vec_err = simse_vector_engine::error::VectorError::EmptyText;
	let err: SimseError = vec_err.into();
	assert_eq!(err.code(), "VECTOR_ERROR");
	assert!(
		err.to_string().contains("empty text"),
		"expected 'empty text' in: {}",
		err
	);
}

#[test]
fn from_vfs_error() {
	let vfs_err =
		simse_vfs_engine::error::VfsError::NotFound("/missing".into());
	let err: SimseError = vfs_err.into();
	assert_eq!(err.code(), "VFS_ENGINE_ERROR");
	assert!(
		err.to_string().contains("/missing"),
		"expected '/missing' in: {}",
		err
	);
}

#[test]
fn from_io_error() {
	let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
	let err: SimseError = io_err.into();
	assert_eq!(err.code(), "IO_ERROR");
	assert_eq!(err.to_string(), "IO error: denied");
}

// ---------------------------------------------------------------------------
// is_retriable
// ---------------------------------------------------------------------------

#[test]
fn is_retriable_provider_timeout() {
	let err = SimseError::provider(ProviderErrorCode::Timeout, "slow", None);
	assert!(err.is_retriable());
}

#[test]
fn is_retriable_provider_unavailable() {
	let err = SimseError::provider(ProviderErrorCode::Unavailable, "down", None);
	assert!(err.is_retriable());
}

#[test]
fn is_retriable_provider_rate_limited() {
	let err = SimseError::provider(ProviderErrorCode::RateLimited, "429", Some(429));
	assert!(err.is_retriable());
}

#[test]
fn is_retriable_resilience_timeout() {
	let err = SimseError::resilience(ResilienceErrorCode::Timeout, "timed out");
	assert!(err.is_retriable());
}

#[test]
fn is_retriable_tool_timeout() {
	let err = SimseError::tool(ToolErrorCode::Timeout, "tool timed out");
	assert!(err.is_retriable());
}

#[test]
fn is_not_retriable_config_error() {
	let err = SimseError::config(ConfigErrorCode::InvalidField, "bad");
	assert!(!err.is_retriable());
}

#[test]
fn is_not_retriable_provider_auth_failed() {
	let err = SimseError::provider(ProviderErrorCode::AuthFailed, "bad key", None);
	assert!(!err.is_retriable());
}

// ---------------------------------------------------------------------------
// http_status
// ---------------------------------------------------------------------------

#[test]
fn http_status_returns_some_for_provider_with_status() {
	let err = SimseError::provider(ProviderErrorCode::HttpError, "500", Some(500));
	assert_eq!(err.http_status(), Some(500));
}

#[test]
fn http_status_returns_none_for_provider_without_status() {
	let err = SimseError::provider(ProviderErrorCode::Timeout, "slow", None);
	assert_eq!(err.http_status(), None);
}

#[test]
fn http_status_returns_none_for_non_provider() {
	let err = SimseError::config(ConfigErrorCode::InvalidField, "bad");
	assert_eq!(err.http_status(), None);
}

// ---------------------------------------------------------------------------
// Result type alias
// ---------------------------------------------------------------------------

#[test]
fn result_type_alias_works() {
	fn fallible() -> simse_core::error::Result<u32> {
		Err(SimseError::other("nope"))
	}
	assert!(fallible().is_err());
}

// ---------------------------------------------------------------------------
// Error code Display impls
// ---------------------------------------------------------------------------

#[test]
fn error_code_display() {
	assert_eq!(format!("{}", ConfigErrorCode::InvalidField), "CONFIG_INVALID_FIELD");
	assert_eq!(format!("{}", ProviderErrorCode::Timeout), "PROVIDER_TIMEOUT");
	assert_eq!(format!("{}", ChainErrorCode::Empty), "CHAIN_EMPTY");
	assert_eq!(format!("{}", TemplateErrorCode::Empty), "TEMPLATE_EMPTY");
	assert_eq!(format!("{}", McpErrorCode::ToolError), "MCP_TOOL_ERROR");
	assert_eq!(format!("{}", LibraryErrorCode::EmptyText), "LIBRARY_EMPTY_TEXT");
	assert_eq!(format!("{}", LoopErrorCode::DoomLoop), "LOOP_DOOM_LOOP");
	assert_eq!(format!("{}", ResilienceErrorCode::CircuitOpen), "RESILIENCE_CIRCUIT_OPEN");
	assert_eq!(format!("{}", TaskErrorCode::NotFound), "TASK_NOT_FOUND");
	assert_eq!(format!("{}", ToolErrorCode::NotFound), "TOOL_NOT_FOUND");
	assert_eq!(format!("{}", VfsErrorCode::InvalidPath), "VFS_INVALID_PATH");
}

// ---------------------------------------------------------------------------
// Error code Clone + PartialEq
// ---------------------------------------------------------------------------

#[test]
fn error_code_clone_eq() {
	let code = ConfigErrorCode::InvalidField;
	let cloned = code.clone();
	assert_eq!(code, cloned);
}
