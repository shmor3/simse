//! Tool registry — discovers, registers, and executes tools.
//!
//! Mirrors the TypeScript `tool-registry.ts` from `simse-code`. The registry
//! holds built-in tool stubs (library, VFS) and can discover MCP tools from
//! connected servers. Each tool has a [`ToolHandler`] that is called when the
//! agentic loop requests execution.

use std::collections::HashMap;

use simse_ui_core::tools::{
	DEFAULT_MAX_OUTPUT_CHARS, ToolCallRequest, ToolCallResult, ToolDefinition, ToolHandlerOutput,
	ToolParameter, truncate_output,
};

// ---------------------------------------------------------------------------
// ToolHandler trait
// ---------------------------------------------------------------------------

/// Async handler for a single tool.
///
/// Implementations receive the JSON arguments blob and return either a plain
/// string (via the `From` impls on [`ToolHandlerOutput`]) or a full
/// [`ToolHandlerOutput`] with optional diff.
pub trait ToolHandler: Send + Sync {
	fn execute(
		&self,
		args: serde_json::Value,
	) -> impl std::future::Future<Output = Result<ToolHandlerOutput, ToolExecutionError>> + Send;
}

/// Error returned when a tool handler fails.
#[derive(Debug, thiserror::Error)]
pub enum ToolExecutionError {
	#[error("{0}")]
	HandlerError(String),
	#[error("Unknown tool: \"{0}\"")]
	UnknownTool(String),
}

// ---------------------------------------------------------------------------
// RegisteredTool (type-erased handler)
// ---------------------------------------------------------------------------

/// Type-erased wrapper so we can store heterogeneous handlers in a `HashMap`.
trait DynToolHandler: Send + Sync {
	fn execute_dyn(
		&self,
		args: serde_json::Value,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<ToolHandlerOutput, ToolExecutionError>> + Send + '_>,
	>;
}

impl<T: ToolHandler> DynToolHandler for T {
	fn execute_dyn(
		&self,
		args: serde_json::Value,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<ToolHandlerOutput, ToolExecutionError>> + Send + '_>,
	> {
		Box::pin(self.execute(args))
	}
}

/// A tool definition paired with its handler.
pub struct RegisteredTool {
	pub definition: ToolDefinition,
	handler: Box<dyn DynToolHandler>,
}

// ---------------------------------------------------------------------------
// Built-in stub handlers
// ---------------------------------------------------------------------------

/// Stub handler that returns a placeholder message. Used for built-in tools
/// whose backends (library, VFS) are not yet wired up.
struct StubHandler {
	tool_name: String,
}

impl ToolHandler for StubHandler {
	async fn execute(
		&self,
		_args: serde_json::Value,
	) -> Result<ToolHandlerOutput, ToolExecutionError> {
		Ok(ToolHandlerOutput {
			output: format!("[{}] Not yet connected — stub result.", self.tool_name),
			diff: None,
		})
	}
}

// ---------------------------------------------------------------------------
// ToolRegistry
// ---------------------------------------------------------------------------

/// Central registry of all available tools.
///
/// Holds built-in and MCP-discovered tools, handles execution with output
/// truncation, and provides formatting for system prompts.
pub struct ToolRegistry {
	tools: HashMap<String, RegisteredTool>,
	/// Global maximum output characters. Per-tool overrides take precedence.
	pub max_output_chars: usize,
}

impl ToolRegistry {
	/// Create a new empty registry with the default output truncation limit.
	pub fn new() -> Self {
		Self {
			tools: HashMap::new(),
			max_output_chars: DEFAULT_MAX_OUTPUT_CHARS,
		}
	}

	/// Create a new registry with a custom global output truncation limit.
	pub fn with_max_output_chars(max_output_chars: usize) -> Self {
		Self {
			tools: HashMap::new(),
			max_output_chars,
		}
	}

	/// Register a tool with its handler.
	pub fn register(&mut self, definition: ToolDefinition, handler: impl ToolHandler + 'static) {
		let name = definition.name.clone();
		self.tools.insert(
			name,
			RegisteredTool {
				definition,
				handler: Box::new(handler),
			},
		);
	}

	/// Unregister a tool by name. Returns `true` if the tool was found and removed.
	pub fn unregister(&mut self, name: &str) -> bool {
		self.tools.remove(name).is_some()
	}

	/// Number of registered tools.
	pub fn tool_count(&self) -> usize {
		self.tools.len()
	}

	/// Get all tool definitions (sorted by name for stable ordering).
	pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
		let mut defs: Vec<ToolDefinition> =
			self.tools.values().map(|t| t.definition.clone()).collect();
		defs.sort_by(|a, b| a.name.cmp(&b.name));
		defs
	}

	/// Format all registered tools for system prompt injection using the
	/// `simse_ui_core::tools::format_tools_for_system_prompt` helper.
	pub fn format_for_system_prompt(&self) -> String {
		let defs = self.get_tool_definitions();
		if defs.is_empty() {
			return String::new();
		}
		simse_ui_core::tools::format_tools_for_system_prompt(&defs)
	}

	/// Execute a tool call. Looks up the handler by name, calls it, truncates
	/// output, and returns a [`ToolCallResult`].
	pub async fn execute(&self, call: &ToolCallRequest) -> ToolCallResult {
		let registered = match self.tools.get(&call.name) {
			Some(r) => r,
			None => {
				return ToolCallResult {
					id: call.id.clone(),
					name: call.name.clone(),
					output: format!("Unknown tool: \"{}\"", call.name),
					is_error: true,
					diff: None,
				};
			}
		};

		match registered.handler.execute_dyn(call.arguments.clone()).await {
			Ok(raw) => {
				// Determine the effective max output chars
				let max_chars = registered
					.definition
					.max_output_chars
					.unwrap_or(self.max_output_chars);
				let output = truncate_output(&raw.output, max_chars);
				ToolCallResult {
					id: call.id.clone(),
					name: call.name.clone(),
					output,
					is_error: false,
					diff: raw.diff,
				}
			}
			Err(err) => ToolCallResult {
				id: call.id.clone(),
				name: call.name.clone(),
				output: format!("Tool error: {err}"),
				is_error: true,
				diff: None,
			},
		}
	}

	/// Discover tools: clear existing tools, register built-ins, and discover
	/// MCP tools from connected servers.
	///
	/// Currently registers built-in stubs only. MCP discovery will be wired in
	/// a later task when the MCP client bridge is available.
	pub async fn discover(&mut self) {
		self.tools.clear();
		self.register_builtins();
		self.discover_mcp_tools().await;
	}

	/// Register built-in tool definitions with stub handlers.
	///
	/// These stubs will be replaced with real implementations once the library
	/// and VFS backends are connected.
	fn register_builtins(&mut self) {
		// -- Library tools --
		self.register(
			ToolDefinition {
				name: "library_search".into(),
				description:
					"Search the library for relevant volumes and context. Returns matching volumes ranked by relevance."
						.into(),
				parameters: {
					let mut p = HashMap::new();
					p.insert(
						"query".into(),
						ToolParameter {
							param_type: "string".into(),
							description: "The search query".into(),
							required: true,
						},
					);
					p.insert(
						"maxResults".into(),
						ToolParameter {
							param_type: "number".into(),
							description: "Maximum number of results to return (default: 5)".into(),
							required: false,
						},
					);
					p
				},
				max_output_chars: None,
			},
			StubHandler {
				tool_name: "library_search".into(),
			},
		);

		self.register(
			ToolDefinition {
				name: "library_shelve".into(),
				description: "Shelve a volume in the library for long-term storage.".into(),
				parameters: {
					let mut p = HashMap::new();
					p.insert(
						"text".into(),
						ToolParameter {
							param_type: "string".into(),
							description: "The text content to shelve".into(),
							required: true,
						},
					);
					p.insert(
						"topic".into(),
						ToolParameter {
							param_type: "string".into(),
							description: "Topic category for the volume".into(),
							required: true,
						},
					);
					p
				},
				max_output_chars: None,
			},
			StubHandler {
				tool_name: "library_shelve".into(),
			},
		);

		// -- VFS tools --
		self.register(
			ToolDefinition {
				name: "vfs_read".into(),
				description: "Read a file from the virtual filesystem sandbox.".into(),
				parameters: {
					let mut p = HashMap::new();
					p.insert(
						"path".into(),
						ToolParameter {
							param_type: "string".into(),
							description: "VFS path using vfs:// scheme (e.g. vfs:///hello.js)"
								.into(),
							required: true,
						},
					);
					p
				},
				max_output_chars: None,
			},
			StubHandler {
				tool_name: "vfs_read".into(),
			},
		);

		self.register(
			ToolDefinition {
				name: "vfs_write".into(),
				description: "Write a file to the virtual filesystem sandbox.".into(),
				parameters: {
					let mut p = HashMap::new();
					p.insert(
						"path".into(),
						ToolParameter {
							param_type: "string".into(),
							description: "VFS path using vfs:// scheme (e.g. vfs:///hello.js)"
								.into(),
							required: true,
						},
					);
					p.insert(
						"content".into(),
						ToolParameter {
							param_type: "string".into(),
							description: "The file content to write".into(),
							required: true,
						},
					);
					p
				},
				max_output_chars: None,
			},
			StubHandler {
				tool_name: "vfs_write".into(),
			},
		);

		self.register(
			ToolDefinition {
				name: "vfs_list".into(),
				description: "List files and directories in the virtual filesystem sandbox."
					.into(),
				parameters: {
					let mut p = HashMap::new();
					p.insert(
						"path".into(),
						ToolParameter {
							param_type: "string".into(),
							description: "VFS path using vfs:// scheme (e.g. vfs:///hello.js)"
								.into(),
							required: false,
						},
					);
					p
				},
				max_output_chars: None,
			},
			StubHandler {
				tool_name: "vfs_list".into(),
			},
		);

		self.register(
			ToolDefinition {
				name: "vfs_tree".into(),
				description: "Show a tree view of the virtual filesystem sandbox.".into(),
				parameters: {
					let mut p = HashMap::new();
					p.insert(
						"path".into(),
						ToolParameter {
							param_type: "string".into(),
							description: "VFS path using vfs:// scheme (e.g. vfs:///hello.js)"
								.into(),
							required: false,
						},
					);
					p
				},
				max_output_chars: None,
			},
			StubHandler {
				tool_name: "vfs_tree".into(),
			},
		);
	}

	/// Discover MCP tools from connected servers.
	///
	/// Stub: actual MCP wiring comes in a later task. When implemented, this
	/// will iterate connected MCP servers, list their tools, and register each
	/// as `mcp:{server}/{name}`.
	async fn discover_mcp_tools(&mut self) {
		// TODO: Iterate MCP client connected servers, list tools, register as
		// mcp:{server}/{name} with handlers that call mcpClient.callTool().
	}
}

impl Default for ToolRegistry {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	/// Simple test handler that echoes arguments back.
	struct EchoHandler;

	impl ToolHandler for EchoHandler {
		async fn execute(
			&self,
			args: serde_json::Value,
		) -> Result<ToolHandlerOutput, ToolExecutionError> {
			Ok(ToolHandlerOutput {
				output: format!("Echo: {args}"),
				diff: None,
			})
		}
	}

	/// Handler that always returns an error.
	struct ErrorHandler;

	impl ToolHandler for ErrorHandler {
		async fn execute(
			&self,
			_args: serde_json::Value,
		) -> Result<ToolHandlerOutput, ToolExecutionError> {
			Err(ToolExecutionError::HandlerError(
				"something went wrong".into(),
			))
		}
	}

	/// Handler that returns a large output for truncation testing.
	struct LargeOutputHandler {
		size: usize,
	}

	impl ToolHandler for LargeOutputHandler {
		async fn execute(
			&self,
			_args: serde_json::Value,
		) -> Result<ToolHandlerOutput, ToolExecutionError> {
			Ok(ToolHandlerOutput {
				output: "x".repeat(self.size),
				diff: None,
			})
		}
	}

	/// Handler that returns output with a diff.
	struct DiffHandler;

	impl ToolHandler for DiffHandler {
		async fn execute(
			&self,
			_args: serde_json::Value,
		) -> Result<ToolHandlerOutput, ToolExecutionError> {
			Ok(ToolHandlerOutput {
				output: "Wrote 42 bytes".into(),
				diff: Some("+new content".into()),
			})
		}
	}

	fn make_call(name: &str) -> ToolCallRequest {
		ToolCallRequest {
			id: "call_1".into(),
			name: name.into(),
			arguments: json!({}),
		}
	}

	fn make_call_with_args(name: &str, args: serde_json::Value) -> ToolCallRequest {
		ToolCallRequest {
			id: "call_1".into(),
			name: name.into(),
			arguments: args,
		}
	}

	fn simple_tool_def(name: &str) -> ToolDefinition {
		ToolDefinition {
			name: name.into(),
			description: format!("Test tool: {name}"),
			parameters: HashMap::new(),
			max_output_chars: None,
		}
	}

	// -- Registry basics --

	#[test]
	fn new_registry_is_empty() {
		let reg = ToolRegistry::new();
		assert_eq!(reg.tool_count(), 0);
	}

	#[test]
	fn default_max_output_chars() {
		let reg = ToolRegistry::new();
		assert_eq!(reg.max_output_chars, DEFAULT_MAX_OUTPUT_CHARS);
	}

	#[test]
	fn custom_max_output_chars() {
		let reg = ToolRegistry::with_max_output_chars(1000);
		assert_eq!(reg.max_output_chars, 1000);
	}

	#[test]
	fn register_and_count() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("echo"), EchoHandler);
		assert_eq!(reg.tool_count(), 1);
	}

	#[test]
	fn register_replaces_existing() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("echo"), EchoHandler);
		reg.register(simple_tool_def("echo"), ErrorHandler);
		assert_eq!(reg.tool_count(), 1);
	}

	#[test]
	fn unregister_existing() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("echo"), EchoHandler);
		assert!(reg.unregister("echo"));
		assert_eq!(reg.tool_count(), 0);
	}

	#[test]
	fn unregister_nonexistent() {
		let mut reg = ToolRegistry::new();
		assert!(!reg.unregister("nope"));
	}

	// -- Tool definitions --

	#[test]
	fn get_tool_definitions_sorted() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("zebra"), EchoHandler);
		reg.register(simple_tool_def("alpha"), EchoHandler);
		reg.register(simple_tool_def("middle"), EchoHandler);
		let defs = reg.get_tool_definitions();
		let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
		assert_eq!(names, vec!["alpha", "middle", "zebra"]);
	}

	// -- Format for system prompt --

	#[test]
	fn format_empty_returns_empty_string() {
		let reg = ToolRegistry::new();
		assert_eq!(reg.format_for_system_prompt(), "");
	}

	#[test]
	fn format_with_tools_wraps_in_tool_use_tags() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("my_tool"), EchoHandler);
		let prompt = reg.format_for_system_prompt();
		assert!(prompt.starts_with("<tool_use>"));
		assert!(prompt.ends_with("</tool_use>"));
		assert!(prompt.contains("my_tool"));
	}

	// -- Execute --

	#[tokio::test]
	async fn execute_unknown_tool() {
		let reg = ToolRegistry::new();
		let result = reg.execute(&make_call("nonexistent")).await;
		assert!(result.is_error);
		assert!(result.output.contains("Unknown tool"));
		assert!(result.output.contains("nonexistent"));
	}

	#[tokio::test]
	async fn execute_success() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("echo"), EchoHandler);
		let call = make_call_with_args("echo", json!({"message": "hello"}));
		let result = reg.execute(&call).await;
		assert!(!result.is_error);
		assert!(result.output.contains("Echo:"));
		assert!(result.output.contains("hello"));
		assert_eq!(result.id, "call_1");
		assert_eq!(result.name, "echo");
	}

	#[tokio::test]
	async fn execute_handler_error() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("fail"), ErrorHandler);
		let result = reg.execute(&make_call("fail")).await;
		assert!(result.is_error);
		assert!(result.output.contains("Tool error"));
		assert!(result.output.contains("something went wrong"));
	}

	#[tokio::test]
	async fn execute_with_diff() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("writer"), DiffHandler);
		let result = reg.execute(&make_call("writer")).await;
		assert!(!result.is_error);
		assert_eq!(result.output, "Wrote 42 bytes");
		assert_eq!(result.diff.as_deref(), Some("+new content"));
	}

	// -- Output truncation --

	#[tokio::test]
	async fn execute_truncates_large_output() {
		let mut reg = ToolRegistry::with_max_output_chars(100);
		reg.register(simple_tool_def("big"), LargeOutputHandler { size: 500 });
		let result = reg.execute(&make_call("big")).await;
		assert!(!result.is_error);
		assert!(result.output.ends_with("[OUTPUT TRUNCATED]"));
		assert!(result.output.len() < 500);
	}

	#[tokio::test]
	async fn execute_respects_per_tool_max_output_chars() {
		let mut reg = ToolRegistry::with_max_output_chars(100_000);
		let mut def = simple_tool_def("limited");
		def.max_output_chars = Some(50);
		reg.register(def, LargeOutputHandler { size: 500 });
		let result = reg.execute(&make_call("limited")).await;
		assert!(!result.is_error);
		assert!(result.output.ends_with("[OUTPUT TRUNCATED]"));
		// Should be truncated to 50 + "[OUTPUT TRUNCATED]".len()
		assert_eq!(result.output.len(), 50 + "[OUTPUT TRUNCATED]".len());
	}

	#[tokio::test]
	async fn execute_no_truncation_for_small_output() {
		let mut reg = ToolRegistry::with_max_output_chars(1000);
		reg.register(simple_tool_def("small"), LargeOutputHandler { size: 50 });
		let result = reg.execute(&make_call("small")).await;
		assert!(!result.is_error);
		assert!(!result.output.contains("[OUTPUT TRUNCATED]"));
		assert_eq!(result.output.len(), 50);
	}

	// -- Discover --

	#[tokio::test]
	async fn discover_registers_builtins() {
		let mut reg = ToolRegistry::new();
		reg.discover().await;
		assert!(reg.tool_count() >= 6, "Expected at least 6 built-in tools");
		let defs = reg.get_tool_definitions();
		let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
		assert!(names.contains(&"library_search"));
		assert!(names.contains(&"library_shelve"));
		assert!(names.contains(&"vfs_read"));
		assert!(names.contains(&"vfs_write"));
		assert!(names.contains(&"vfs_list"));
		assert!(names.contains(&"vfs_tree"));
	}

	#[tokio::test]
	async fn discover_clears_existing_tools() {
		let mut reg = ToolRegistry::new();
		reg.register(simple_tool_def("custom"), EchoHandler);
		assert_eq!(reg.tool_count(), 1);
		reg.discover().await;
		// custom should be gone, replaced by builtins
		let defs = reg.get_tool_definitions();
		let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
		assert!(!names.contains(&"custom"));
	}

	#[tokio::test]
	async fn builtin_stubs_return_placeholder() {
		let mut reg = ToolRegistry::new();
		reg.discover().await;
		let call = make_call_with_args("library_search", json!({"query": "test"}));
		let result = reg.execute(&call).await;
		assert!(!result.is_error);
		assert!(result.output.contains("stub"));
	}

	#[tokio::test]
	async fn builtin_tool_definitions_have_correct_parameters() {
		let mut reg = ToolRegistry::new();
		reg.discover().await;
		let defs = reg.get_tool_definitions();

		// library_search should have query (required) and maxResults (optional)
		let search = defs.iter().find(|d| d.name == "library_search").unwrap();
		assert!(search.parameters.get("query").unwrap().required);
		assert!(!search.parameters.get("maxResults").unwrap().required);

		// vfs_write should have path and content (both required)
		let write = defs.iter().find(|d| d.name == "vfs_write").unwrap();
		assert!(write.parameters.get("path").unwrap().required);
		assert!(write.parameters.get("content").unwrap().required);

		// vfs_list path should be optional
		let list = defs.iter().find(|d| d.name == "vfs_list").unwrap();
		assert!(!list.parameters.get("path").unwrap().required);
	}

	// -- Default trait --

	#[test]
	fn default_creates_empty_registry() {
		let reg = ToolRegistry::default();
		assert_eq!(reg.tool_count(), 0);
		assert_eq!(reg.max_output_chars, DEFAULT_MAX_OUTPUT_CHARS);
	}
}
