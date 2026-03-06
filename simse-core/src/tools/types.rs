//! Tool registry types — definitions, calls, results, metrics.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::SimseError;

// ---------------------------------------------------------------------------
// ToolCategory
// ---------------------------------------------------------------------------

/// Classification of a tool's purpose.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
	Read,
	Edit,
	Search,
	Execute,
	Library,
	Task,
	Subagent,
	#[default]
	Other,
}

impl fmt::Display for ToolCategory {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let s = match self {
			Self::Read => "read",
			Self::Edit => "edit",
			Self::Search => "search",
			Self::Execute => "execute",
			Self::Library => "library",
			Self::Task => "task",
			Self::Subagent => "subagent",
			Self::Other => "other",
		};
		f.write_str(s)
	}
}

// ---------------------------------------------------------------------------
// ToolParameter
// ---------------------------------------------------------------------------

/// Describes a single parameter accepted by a tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolParameter {
	/// JSON schema type (e.g. "string", "number", "boolean").
	#[serde(rename = "type")]
	pub param_type: String,
	/// Human-readable description.
	pub description: String,
	/// Whether this parameter is required.
	#[serde(default)]
	pub required: bool,
}

// ---------------------------------------------------------------------------
// ToolAnnotations
// ---------------------------------------------------------------------------

/// Optional metadata annotations for a tool definition.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToolAnnotations {
	/// Human-readable title for the tool.
	pub title: Option<String>,
	/// Whether the tool performs destructive operations.
	pub destructive: Option<bool>,
	/// Whether the tool is read-only.
	pub read_only: Option<bool>,
}

// ---------------------------------------------------------------------------
// ToolDefinition
// ---------------------------------------------------------------------------

/// Describes a tool that can be registered in the tool registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
	/// Unique name identifying the tool.
	pub name: String,
	/// Human-readable description of the tool.
	pub description: String,
	/// Parameters the tool accepts, keyed by parameter name.
	pub parameters: HashMap<String, ToolParameter>,
	/// Classification of the tool.
	#[serde(default)]
	pub category: ToolCategory,
	/// Optional metadata annotations.
	pub annotations: Option<ToolAnnotations>,
	/// Per-tool execution timeout in milliseconds (overrides registry default).
	pub timeout_ms: Option<u64>,
	/// Per-tool output character limit (overrides registry default).
	pub max_output_chars: Option<usize>,
}

// ---------------------------------------------------------------------------
// ToolCallRequest
// ---------------------------------------------------------------------------

/// A request to invoke a tool, typically parsed from model output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
	/// Unique identifier for this tool call.
	pub id: String,
	/// Name of the tool to invoke.
	pub name: String,
	/// Arguments to pass to the tool handler.
	pub arguments: serde_json::Value,
}

// ---------------------------------------------------------------------------
// ToolCallResult
// ---------------------------------------------------------------------------

/// The result produced by executing a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
	/// Matches the `id` from the originating `ToolCallRequest`.
	pub id: String,
	/// Name of the tool that was invoked.
	pub name: String,
	/// The text output (or error message) from the tool.
	pub output: String,
	/// Whether execution failed.
	pub is_error: bool,
	/// Wall-clock execution time in milliseconds.
	pub duration_ms: Option<u64>,
	/// Optional unified diff produced by the tool (e.g. file edits).
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub diff: Option<String>,
}

// ---------------------------------------------------------------------------
// ToolHandler
// ---------------------------------------------------------------------------

/// Async function that executes a tool given JSON arguments.
///
/// Handlers receive a `serde_json::Value` (typically an object) and must
/// return either a `String` output or a `SimseError`.
pub type ToolHandler = Arc<
	dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<String, SimseError>> + Send>>
		+ Send
		+ Sync,
>;

// ---------------------------------------------------------------------------
// RegisteredTool
// ---------------------------------------------------------------------------

/// A tool definition paired with its handler, stored in the registry.
pub struct RegisteredTool {
	pub definition: ToolDefinition,
	pub handler: ToolHandler,
}

impl fmt::Debug for RegisteredTool {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("RegisteredTool")
			.field("definition", &self.definition)
			.field("handler", &"<fn>")
			.finish()
	}
}

impl Clone for RegisteredTool {
	fn clone(&self) -> Self {
		Self {
			definition: self.definition.clone(),
			handler: Arc::clone(&self.handler),
		}
	}
}

// ---------------------------------------------------------------------------
// ToolMetrics
// ---------------------------------------------------------------------------

/// Aggregated performance metrics for a single tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolMetrics {
	/// Tool name.
	pub name: String,
	/// Total number of calls.
	pub call_count: u64,
	/// Number of calls that resulted in an error.
	pub error_count: u64,
	/// Sum of all call durations in milliseconds.
	pub total_duration_ms: u64,
	/// Average call duration in milliseconds.
	pub avg_duration_ms: f64,
	/// Timestamp (epoch millis) of the last call.
	pub last_called_at: u64,
}

// ---------------------------------------------------------------------------
// ParsedResponse
// ---------------------------------------------------------------------------

/// Result of parsing tool calls from a model response string.
#[derive(Debug, Clone)]
pub struct ParsedResponse {
	/// The text remaining after tool_use blocks are stripped.
	pub text: String,
	/// Tool call requests extracted from `<tool_use>` blocks.
	pub tool_calls: Vec<ToolCallRequest>,
}

// ---------------------------------------------------------------------------
// ToolRegistryOptions
// ---------------------------------------------------------------------------

/// Configuration options for creating a `ToolRegistry`.
#[derive(Default)]
pub struct ToolRegistryOptions {
	/// Optional permission resolver for gating tool execution.
	pub permission_resolver: Option<Arc<dyn ToolPermissionResolver>>,
	/// Default execution timeout for all tools (overridden per-tool).
	pub default_timeout_ms: Option<u64>,
	/// Maximum characters for tool output before truncation. Default: 50,000.
	pub max_output_chars: Option<usize>,
}

impl fmt::Debug for ToolRegistryOptions {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ToolRegistryOptions")
			.field(
				"permission_resolver",
				&self.permission_resolver.is_some(),
			)
			.field("default_timeout_ms", &self.default_timeout_ms)
			.field("max_output_chars", &self.max_output_chars)
			.finish()
	}
}

// Re-export trait here so it's available alongside options
use crate::tools::permissions::ToolPermissionResolver;
