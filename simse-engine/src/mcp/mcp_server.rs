// ---------------------------------------------------------------------------
// MCP Server — hosts tools, resources, and prompts for external MCP clients
// ---------------------------------------------------------------------------
//
// Responsibilities:
//   - Tool, resource, and prompt registration with handler traits
//   - JSON-RPC request dispatch (initialize, tools/*, resources/*, prompts/*, etc.)
//   - Server capability advertisement based on registered handlers
//   - Logging level management
//   - Workspace roots tracking
//
// The server does NOT own a transport — the JSON-RPC wrapper in main.rs reads
// requests from stdio/HTTP and calls `handle_request` for each one. This keeps
// the server transport-agnostic and testable in isolation.
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;

use crate::mcp::error::McpError;
use crate::mcp::protocol::{
	CompletionResult, ContentItem, GetPromptParams, GetPromptResult, McpServerConfig,
	PromptDefinition, PromptInfo, PromptMessage, ReadResourceParams, ReadResourceResult,
	ResourceContent, ResourceDefinition, ResourceInfo, ResourceTemplateInfo, Root, ToolCallParams,
	ToolCallResult, ToolDefinition, ToolInfo,
};

// ---------------------------------------------------------------------------
// Handler traits
// ---------------------------------------------------------------------------

/// Handler for tool execution. Implement this trait for custom tool logic,
/// or use the blanket impl for async closures via [`FnToolHandler`].
#[async_trait]
pub trait ToolHandler: Send + Sync {
	async fn execute(&self, args: serde_json::Value) -> Result<ToolCallResult, McpError>;
}

/// Handler for resource reads. Implement this trait for custom resource logic,
/// or use the blanket impl for async closures via [`FnResourceHandler`].
#[async_trait]
pub trait ResourceHandler: Send + Sync {
	async fn read(&self, uri: &str) -> Result<String, McpError>;
}

/// Handler for prompt generation. Implement this trait for custom prompt logic,
/// or use the blanket impl for async closures via [`FnPromptHandler`].
#[async_trait]
pub trait PromptHandler: Send + Sync {
	async fn get(&self, args: serde_json::Value) -> Result<Vec<PromptMessage>, McpError>;
}

// ---------------------------------------------------------------------------
// Closure-based handler wrappers
// ---------------------------------------------------------------------------

/// Wrapper that implements [`ToolHandler`] for async closures.
///
/// # Example
/// ```ignore
/// server.register_tool_fn(def, |args| async move {
///     Ok(ToolCallResult { content: vec![ContentItem::Text { text: "ok".into() }], is_error: None })
/// });
/// ```
pub struct FnToolHandler<F>(pub F);

#[async_trait]
impl<F, Fut> ToolHandler for FnToolHandler<F>
where
	F: Fn(serde_json::Value) -> Fut + Send + Sync,
	Fut: std::future::Future<Output = Result<ToolCallResult, McpError>> + Send,
{
	async fn execute(&self, args: serde_json::Value) -> Result<ToolCallResult, McpError> {
		(self.0)(args).await
	}
}

/// Wrapper that implements [`ResourceHandler`] for async closures.
pub struct FnResourceHandler<F>(pub F);

#[async_trait]
impl<F, Fut> ResourceHandler for FnResourceHandler<F>
where
	F: Fn(String) -> Fut + Send + Sync,
	Fut: std::future::Future<Output = Result<String, McpError>> + Send,
{
	async fn read(&self, uri: &str) -> Result<String, McpError> {
		(self.0)(uri.to_string()).await
	}
}

/// Wrapper that implements [`PromptHandler`] for async closures.
pub struct FnPromptHandler<F>(pub F);

#[async_trait]
impl<F, Fut> PromptHandler for FnPromptHandler<F>
where
	F: Fn(serde_json::Value) -> Fut + Send + Sync,
	Fut: std::future::Future<Output = Result<Vec<PromptMessage>, McpError>> + Send,
{
	async fn get(&self, args: serde_json::Value) -> Result<Vec<PromptMessage>, McpError> {
		(self.0)(args).await
	}
}

// ---------------------------------------------------------------------------
// Registered handler containers (Arc-wrapped for im::HashMap cloneability)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct RegisteredTool {
	definition: ToolDefinition,
	handler: Arc<dyn ToolHandler>,
}

#[derive(Clone)]
struct RegisteredResource {
	definition: ResourceDefinition,
	handler: Arc<dyn ResourceHandler>,
}

#[derive(Clone)]
struct RegisteredPrompt {
	definition: PromptDefinition,
	handler: Arc<dyn PromptHandler>,
}

// ---------------------------------------------------------------------------
// MCP protocol version
// ---------------------------------------------------------------------------

const PROTOCOL_VERSION: &str = "2025-03-26";

// ---------------------------------------------------------------------------
// McpServer
// ---------------------------------------------------------------------------

/// An MCP server that hosts tools, resources, and prompts for external clients.
///
/// The server is transport-agnostic: it exposes a `handle_request` method that
/// processes incoming JSON-RPC method calls and returns JSON-RPC results. The
/// actual transport (stdio, HTTP) is managed externally.
///
/// Uses persistent data structures (`im::HashMap`) for registries and
/// owned-return methods for state transitions (FP pattern).
pub struct McpServer {
	config: McpServerConfig,
	tools: im::HashMap<String, RegisteredTool>,
	resources: im::HashMap<String, RegisteredResource>,
	prompts: im::HashMap<String, RegisteredPrompt>,
	running: AtomicBool,
	roots: Vec<Root>,
	logging_level: Mutex<String>,
}

impl McpServer {
	/// Create a new MCP server with the given configuration.
	pub fn new(config: McpServerConfig) -> Self {
		Self {
			config,
			tools: im::HashMap::new(),
			resources: im::HashMap::new(),
			prompts: im::HashMap::new(),
			running: AtomicBool::new(false),
			roots: Vec::new(),
			logging_level: Mutex::new("info".to_string()),
		}
	}

	// -----------------------------------------------------------------------
	// Tool registration (owned-return pattern)
	// -----------------------------------------------------------------------

	/// Register a tool with a trait-object handler.
	/// Consumes self, returns the updated server.
	pub fn register_tool(
		mut self,
		definition: ToolDefinition,
		handler: impl ToolHandler + 'static,
	) -> Self {
		let name = definition.name.clone();
		self.tools = self.tools.update(
			name,
			RegisteredTool {
				definition,
				handler: Arc::new(handler),
			},
		);
		self
	}

	/// Register a tool with an async closure handler.
	/// Consumes self, returns the updated server.
	///
	/// # Example
	/// ```ignore
	/// let server = server.register_tool_fn(def, |args| async move {
	///     Ok(ToolCallResult { content: vec![ContentItem::Text { text: "done".into() }], is_error: None })
	/// });
	/// ```
	pub fn register_tool_fn<F, Fut>(self, definition: ToolDefinition, handler: F) -> Self
	where
		F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
		Fut: std::future::Future<Output = Result<ToolCallResult, McpError>> + Send + 'static,
	{
		self.register_tool(definition, FnToolHandler(handler))
	}

	/// Unregister a tool by name. Consumes self, returns `(updated_server, was_removed)`.
	pub fn unregister_tool(mut self, name: &str) -> (Self, bool) {
		let existed = self.tools.contains_key(name);
		if existed {
			self.tools = self.tools.without(name);
		}
		(self, existed)
	}

	/// Returns the number of registered tools.
	pub fn tool_count(&self) -> usize {
		self.tools.len()
	}

	/// Returns the names of all registered tools, sorted alphabetically.
	pub fn tool_names(&self) -> Vec<String> {
		let mut names: Vec<String> = self.tools.keys().cloned().collect();
		names.sort();
		names
	}

	// -----------------------------------------------------------------------
	// Resource registration (owned-return pattern)
	// -----------------------------------------------------------------------

	/// Register a resource with a trait-object handler.
	/// Consumes self, returns the updated server.
	pub fn register_resource(
		mut self,
		definition: ResourceDefinition,
		handler: impl ResourceHandler + 'static,
	) -> Self {
		let uri = definition.uri.clone();
		self.resources = self.resources.update(
			uri,
			RegisteredResource {
				definition,
				handler: Arc::new(handler),
			},
		);
		self
	}

	/// Register a resource with an async closure handler.
	/// Consumes self, returns the updated server.
	pub fn register_resource_fn<F, Fut>(self, definition: ResourceDefinition, handler: F) -> Self
	where
		F: Fn(String) -> Fut + Send + Sync + 'static,
		Fut: std::future::Future<Output = Result<String, McpError>> + Send + 'static,
	{
		self.register_resource(definition, FnResourceHandler(handler))
	}

	/// Unregister a resource by URI. Consumes self, returns `(updated_server, was_removed)`.
	pub fn unregister_resource(mut self, uri: &str) -> (Self, bool) {
		let existed = self.resources.contains_key(uri);
		if existed {
			self.resources = self.resources.without(uri);
		}
		(self, existed)
	}

	/// Returns the number of registered resources.
	pub fn resource_count(&self) -> usize {
		self.resources.len()
	}

	// -----------------------------------------------------------------------
	// Prompt registration (owned-return pattern)
	// -----------------------------------------------------------------------

	/// Register a prompt with a trait-object handler.
	/// Consumes self, returns the updated server.
	pub fn register_prompt(
		mut self,
		definition: PromptDefinition,
		handler: impl PromptHandler + 'static,
	) -> Self {
		let name = definition.name.clone();
		self.prompts = self.prompts.update(
			name,
			RegisteredPrompt {
				definition,
				handler: Arc::new(handler),
			},
		);
		self
	}

	/// Register a prompt with an async closure handler.
	/// Consumes self, returns the updated server.
	pub fn register_prompt_fn<F, Fut>(self, definition: PromptDefinition, handler: F) -> Self
	where
		F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
		Fut: std::future::Future<Output = Result<Vec<PromptMessage>, McpError>> + Send + 'static,
	{
		self.register_prompt(definition, FnPromptHandler(handler))
	}

	/// Unregister a prompt by name. Consumes self, returns `(updated_server, was_removed)`.
	pub fn unregister_prompt(mut self, name: &str) -> (Self, bool) {
		let existed = self.prompts.contains_key(name);
		if existed {
			self.prompts = self.prompts.without(name);
		}
		(self, existed)
	}

	/// Returns the number of registered prompts.
	pub fn prompt_count(&self) -> usize {
		self.prompts.len()
	}

	// -----------------------------------------------------------------------
	// Roots (owned-return pattern)
	// -----------------------------------------------------------------------

	/// Set the workspace roots. Consumes self, returns the updated server.
	pub fn set_roots(mut self, roots: Vec<Root>) -> Self {
		self.roots = roots;
		self
	}

	/// Returns a reference to the current workspace roots.
	pub fn roots(&self) -> &[Root] {
		&self.roots
	}

	// -----------------------------------------------------------------------
	// Running state
	// -----------------------------------------------------------------------

	/// Returns whether the server is marked as running.
	pub fn is_running(&self) -> bool {
		self.running.load(Ordering::Relaxed)
	}

	/// Mark the server as running.
	pub fn set_running(&self, running: bool) {
		self.running.store(running, Ordering::Relaxed);
	}

	// -----------------------------------------------------------------------
	// Request dispatch
	// -----------------------------------------------------------------------

	/// Handle an incoming MCP JSON-RPC request. This is the main dispatch
	/// point called by the transport layer for each request.
	///
	/// Returns a JSON value on success, or an `McpError` for unknown methods
	/// or handler failures.
	pub async fn handle_request(
		&self,
		method: &str,
		params: serde_json::Value,
	) -> Result<serde_json::Value, McpError> {
		match method {
			"initialize" => self.handle_initialize(params),
			"initialized" => Ok(json!({})),
			"tools/list" => self.handle_tools_list(),
			"tools/call" => self.handle_tools_call(params).await,
			"resources/list" => self.handle_resources_list(),
			"resources/read" => self.handle_resources_read(params).await,
			"resources/templates/list" => self.handle_resource_templates_list(),
			"prompts/list" => self.handle_prompts_list(),
			"prompts/get" => self.handle_prompts_get(params).await,
			"ping" => Ok(json!({})),
			"logging/setLevel" => self.handle_set_logging_level(params),
			"roots/list" => self.handle_roots_list(),
			"completion/complete" => self.handle_completion_complete(),
			_ => Err(McpError::ProtocolError(format!(
				"Unknown method: {}",
				method
			))),
		}
	}

	// -----------------------------------------------------------------------
	// Method handlers
	// -----------------------------------------------------------------------

	fn handle_initialize(
		&self,
		_params: serde_json::Value,
	) -> Result<serde_json::Value, McpError> {
		let mut capabilities = serde_json::Map::new();

		if !self.tools.is_empty() {
			capabilities.insert(
				"tools".to_string(),
				json!({ "listChanged": true }),
			);
		}

		if !self.resources.is_empty() {
			capabilities.insert(
				"resources".to_string(),
				json!({ "listChanged": true }),
			);
		}

		if !self.prompts.is_empty() {
			capabilities.insert(
				"prompts".to_string(),
				json!({ "listChanged": true }),
			);
		}

		capabilities.insert("logging".to_string(), json!({}));

		Ok(json!({
			"protocolVersion": PROTOCOL_VERSION,
			"capabilities": capabilities,
			"serverInfo": {
				"name": self.config.name,
				"version": self.config.version
			}
		}))
	}

	fn handle_tools_list(&self) -> Result<serde_json::Value, McpError> {
		let tools: Vec<ToolInfo> = self
			.tools
			.values()
			.map(|rt| ToolInfo {
				name: rt.definition.name.clone(),
				description: Some(rt.definition.description.clone()),
				input_schema: rt.definition.input_schema.clone(),
				annotations: None,
			})
			.collect();

		Ok(json!({ "tools": tools }))
	}

	async fn handle_tools_call(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, McpError> {
		let call_params: ToolCallParams =
			serde_json::from_value(params).map_err(|e| McpError::Serialization(e.to_string()))?;

		let registered = self.tools.get(&call_params.name).ok_or_else(|| {
			McpError::ToolError {
				tool: call_params.name.clone(),
				message: format!("Tool not found: {}", call_params.name),
			}
		})?;

		match registered.handler.execute(call_params.arguments).await {
			Ok(result) => {
				Ok(serde_json::to_value(&result)
					.map_err(|e| McpError::Serialization(e.to_string()))?)
			}
			Err(e) => {
				// Return the error as a tool result with isError: true
				let error_result = ToolCallResult {
					content: vec![ContentItem::Text {
						text: e.to_string(),
					}],
					is_error: Some(true),
				};
				Ok(serde_json::to_value(&error_result)
					.map_err(|e| McpError::Serialization(e.to_string()))?)
			}
		}
	}

	fn handle_resources_list(&self) -> Result<serde_json::Value, McpError> {
		let resources: Vec<ResourceInfo> = self
			.resources
			.values()
			.map(|rr| ResourceInfo {
				uri: rr.definition.uri.clone(),
				name: rr.definition.name.clone(),
				description: rr.definition.description.clone(),
				mime_type: rr.definition.mime_type.clone(),
			})
			.collect();

		Ok(json!({ "resources": resources }))
	}

	async fn handle_resources_read(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, McpError> {
		let read_params: ReadResourceParams =
			serde_json::from_value(params).map_err(|e| McpError::Serialization(e.to_string()))?;

		let registered =
			self.resources
				.get(&read_params.uri)
				.ok_or_else(|| McpError::ResourceError {
					uri: read_params.uri.clone(),
					message: format!("Resource not found: {}", read_params.uri),
				})?;

		let text = registered.handler.read(&read_params.uri).await?;

		let result = ReadResourceResult {
			contents: vec![ResourceContent {
				uri: read_params.uri,
				text: Some(text),
				blob: None,
				mime_type: registered.definition.mime_type.clone(),
			}],
		};

		serde_json::to_value(&result).map_err(|e| McpError::Serialization(e.to_string()))
	}

	fn handle_resource_templates_list(&self) -> Result<serde_json::Value, McpError> {
		let templates: Vec<ResourceTemplateInfo> = Vec::new();
		Ok(json!({ "resourceTemplates": templates }))
	}

	fn handle_prompts_list(&self) -> Result<serde_json::Value, McpError> {
		let prompts: Vec<PromptInfo> = self
			.prompts
			.values()
			.map(|rp| PromptInfo {
				name: rp.definition.name.clone(),
				description: Some(rp.definition.description.clone()),
				arguments: rp.definition.arguments.clone(),
			})
			.collect();

		Ok(json!({ "prompts": prompts }))
	}

	async fn handle_prompts_get(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, McpError> {
		let get_params: GetPromptParams =
			serde_json::from_value(params).map_err(|e| McpError::Serialization(e.to_string()))?;

		let registered =
			self.prompts
				.get(&get_params.name)
				.ok_or_else(|| McpError::ProtocolError(format!(
					"Prompt not found: {}",
					get_params.name
				)))?;

		let messages = registered.handler.get(get_params.arguments).await?;

		let result = GetPromptResult { messages };

		serde_json::to_value(&result).map_err(|e| McpError::Serialization(e.to_string()))
	}

	fn handle_set_logging_level(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, McpError> {
		if let Some(level) = params.get("level").and_then(|v| v.as_str()) {
			if let Ok(mut current) = self.logging_level.lock() {
				*current = level.to_string();
			}
		}
		Ok(json!({}))
	}

	fn handle_roots_list(&self) -> Result<serde_json::Value, McpError> {
		serde_json::to_value(&self.roots)
			.map_err(|e| McpError::Serialization(e.to_string()))
	}

	fn handle_completion_complete(&self) -> Result<serde_json::Value, McpError> {
		let result = CompletionResult {
			values: Vec::new(),
			has_more: Some(false),
			total: Some(0),
		};
		Ok(json!({ "completion": result }))
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mcp::protocol::{McpServerConfig, TransportType};

	fn test_config() -> McpServerConfig {
		McpServerConfig {
			name: "test-server".to_string(),
			version: "0.1.0".to_string(),
			transport: TransportType::Stdio,
		}
	}

	fn test_tool_def(name: &str) -> ToolDefinition {
		ToolDefinition {
			name: name.to_string(),
			description: format!("Test tool: {}", name),
			input_schema: json!({
				"type": "object",
				"properties": {
					"input": { "type": "string" }
				}
			}),
		}
	}

	fn test_resource_def(uri: &str, name: &str) -> ResourceDefinition {
		ResourceDefinition {
			uri: uri.to_string(),
			name: name.to_string(),
			description: Some(format!("Test resource: {}", name)),
			mime_type: Some("text/plain".to_string()),
		}
	}

	fn test_prompt_def(name: &str) -> PromptDefinition {
		PromptDefinition {
			name: name.to_string(),
			description: format!("Test prompt: {}", name),
			arguments: None,
		}
	}

	// -- Tool registration (owned-return) --

	#[test]
	fn test_register_tool_increments_count() {
		let server = McpServer::new(test_config());
		assert_eq!(server.tool_count(), 0);

		let server = server.register_tool_fn(test_tool_def("search"), |_args| async move {
			Ok(ToolCallResult {
				content: vec![ContentItem::Text {
					text: "found".into(),
				}],
				is_error: None,
			})
		});

		assert_eq!(server.tool_count(), 1);
		assert_eq!(server.tool_names(), vec!["search".to_string()]);
	}

	#[test]
	fn test_register_multiple_tools() {
		let server = McpServer::new(test_config());

		let server = server.register_tool_fn(test_tool_def("alpha"), |_| async move {
			Ok(ToolCallResult {
				content: vec![],
				is_error: None,
			})
		});
		let server = server.register_tool_fn(test_tool_def("beta"), |_| async move {
			Ok(ToolCallResult {
				content: vec![],
				is_error: None,
			})
		});
		let server = server.register_tool_fn(test_tool_def("gamma"), |_| async move {
			Ok(ToolCallResult {
				content: vec![],
				is_error: None,
			})
		});

		assert_eq!(server.tool_count(), 3);
		assert_eq!(
			server.tool_names(),
			vec![
				"alpha".to_string(),
				"beta".to_string(),
				"gamma".to_string()
			]
		);
	}

	#[test]
	fn test_unregister_tool() {
		let server = McpServer::new(test_config());

		let server = server.register_tool_fn(test_tool_def("search"), |_| async move {
			Ok(ToolCallResult {
				content: vec![],
				is_error: None,
			})
		});
		assert_eq!(server.tool_count(), 1);

		let (server, removed) = server.unregister_tool("search");
		assert!(removed);
		assert_eq!(server.tool_count(), 0);
		assert!(server.tool_names().is_empty());
	}

	#[test]
	fn test_unregister_nonexistent_tool() {
		let server = McpServer::new(test_config());
		let (_server, removed) = server.unregister_tool("nonexistent");
		assert!(!removed);
	}

	// -- Resource registration (owned-return) --

	#[test]
	fn test_register_resource_increments_count() {
		let server = McpServer::new(test_config());
		assert_eq!(server.resource_count(), 0);

		let server = server.register_resource_fn(
			test_resource_def("file:///config.json", "Config"),
			|_uri| async move { Ok("{}".to_string()) },
		);

		assert_eq!(server.resource_count(), 1);
	}

	#[test]
	fn test_unregister_resource() {
		let server = McpServer::new(test_config());

		let server = server.register_resource_fn(
			test_resource_def("file:///config.json", "Config"),
			|_uri| async move { Ok("{}".to_string()) },
		);
		assert_eq!(server.resource_count(), 1);

		let (server, removed) = server.unregister_resource("file:///config.json");
		assert!(removed);
		assert_eq!(server.resource_count(), 0);
	}

	#[test]
	fn test_unregister_nonexistent_resource() {
		let server = McpServer::new(test_config());
		let (_server, removed) = server.unregister_resource("file:///nope");
		assert!(!removed);
	}

	// -- Prompt registration (owned-return) --

	#[test]
	fn test_register_prompt_increments_count() {
		let server = McpServer::new(test_config());
		assert_eq!(server.prompt_count(), 0);

		let server = server.register_prompt_fn(test_prompt_def("summarize"), |_args| async move {
			Ok(vec![PromptMessage {
				role: "user".into(),
				content: json!("Please summarize"),
			}])
		});

		assert_eq!(server.prompt_count(), 1);
	}

	#[test]
	fn test_unregister_prompt() {
		let server = McpServer::new(test_config());

		let server = server.register_prompt_fn(test_prompt_def("summarize"), |_args| async move {
			Ok(vec![])
		});
		assert_eq!(server.prompt_count(), 1);

		let (server, removed) = server.unregister_prompt("summarize");
		assert!(removed);
		assert_eq!(server.prompt_count(), 0);
	}

	#[test]
	fn test_unregister_nonexistent_prompt() {
		let server = McpServer::new(test_config());
		let (_server, removed) = server.unregister_prompt("nonexistent");
		assert!(!removed);
	}

	// -- Roots (owned-return) --

	#[test]
	fn test_set_roots() {
		let server = McpServer::new(test_config());
		assert!(server.roots().is_empty());

		let server = server.set_roots(vec![
			Root {
				uri: "file:///workspace".into(),
				name: Some("workspace".into()),
			},
			Root {
				uri: "file:///home".into(),
				name: None,
			},
		]);

		assert_eq!(server.roots().len(), 2);
		assert_eq!(server.roots()[0].uri, "file:///workspace");
		assert_eq!(server.roots()[1].name, None);
	}

	// -- handle_request: initialize --

	#[tokio::test]
	async fn test_handle_initialize() {
		let server = McpServer::new(test_config());

		// Register a tool so capabilities include "tools"
		let server = server.register_tool_fn(test_tool_def("test"), |_| async move {
			Ok(ToolCallResult {
				content: vec![],
				is_error: None,
			})
		});

		let result = server
			.handle_request(
				"initialize",
				json!({
					"protocolVersion": "2025-03-26",
					"capabilities": {},
					"clientInfo": { "name": "test-client", "version": "1.0.0" }
				}),
			)
			.await
			.unwrap();

		assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
		assert_eq!(result["serverInfo"]["name"], "test-server");
		assert_eq!(result["serverInfo"]["version"], "0.1.0");
		assert!(result["capabilities"]["tools"].is_object());
		assert!(result["capabilities"]["logging"].is_object());
	}

	#[tokio::test]
	async fn test_handle_initialize_no_tools() {
		let server = McpServer::new(test_config());

		let result = server
			.handle_request("initialize", json!({}))
			.await
			.unwrap();

		// No tools registered, so "tools" capability should be absent
		assert!(result["capabilities"]["tools"].is_null());
		assert!(result["capabilities"]["logging"].is_object());
	}

	// -- handle_request: tools/list --

	#[tokio::test]
	async fn test_handle_tools_list() {
		let server = McpServer::new(test_config());

		let server = server.register_tool_fn(test_tool_def("search"), |_| async move {
			Ok(ToolCallResult {
				content: vec![],
				is_error: None,
			})
		});
		let server = server.register_tool_fn(test_tool_def("generate"), |_| async move {
			Ok(ToolCallResult {
				content: vec![],
				is_error: None,
			})
		});

		let result = server
			.handle_request("tools/list", json!({}))
			.await
			.unwrap();

		let tools = result["tools"].as_array().unwrap();
		assert_eq!(tools.len(), 2);

		let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
		assert!(names.contains(&"search"));
		assert!(names.contains(&"generate"));
	}

	#[tokio::test]
	async fn test_handle_tools_list_empty() {
		let server = McpServer::new(test_config());

		let result = server
			.handle_request("tools/list", json!({}))
			.await
			.unwrap();

		let tools = result["tools"].as_array().unwrap();
		assert!(tools.is_empty());
	}

	// -- handle_request: tools/call --

	#[tokio::test]
	async fn test_handle_tools_call_success() {
		let server = McpServer::new(test_config());

		let server = server.register_tool_fn(test_tool_def("echo"), |args| async move {
			let input = args
				.get("input")
				.and_then(|v| v.as_str())
				.unwrap_or("(empty)");
			Ok(ToolCallResult {
				content: vec![ContentItem::Text {
					text: format!("echo: {}", input),
				}],
				is_error: None,
			})
		});

		let result = server
			.handle_request(
				"tools/call",
				json!({ "name": "echo", "arguments": { "input": "hello" } }),
			)
			.await
			.unwrap();

		let content = result["content"].as_array().unwrap();
		assert_eq!(content.len(), 1);
		assert_eq!(content[0]["text"], "echo: hello");
		assert!(result["isError"].is_null());
	}

	#[tokio::test]
	async fn test_handle_tools_call_handler_error() {
		let server = McpServer::new(test_config());

		let server = server.register_tool_fn(test_tool_def("fail"), |_args| async move {
			Err(McpError::ToolError {
				tool: "fail".into(),
				message: "intentional failure".into(),
			})
		});

		let result = server
			.handle_request(
				"tools/call",
				json!({ "name": "fail", "arguments": {} }),
			)
			.await
			.unwrap();

		// Handler errors are returned as tool results with isError: true
		assert_eq!(result["isError"], true);
		let text = result["content"][0]["text"].as_str().unwrap();
		assert!(text.contains("intentional failure"));
	}

	#[tokio::test]
	async fn test_handle_tools_call_not_found() {
		let server = McpServer::new(test_config());

		let err = server
			.handle_request(
				"tools/call",
				json!({ "name": "nonexistent", "arguments": {} }),
			)
			.await;

		assert!(err.is_err());
		let e = err.unwrap_err();
		assert_eq!(e.code(), "MCP_TOOL_ERROR");
	}

	// -- handle_request: resources/list --

	#[tokio::test]
	async fn test_handle_resources_list() {
		let server = McpServer::new(test_config());

		let server = server.register_resource_fn(
			test_resource_def("file:///config.json", "Config"),
			|_uri| async move { Ok("{}".to_string()) },
		);

		let result = server
			.handle_request("resources/list", json!({}))
			.await
			.unwrap();

		let resources = result["resources"].as_array().unwrap();
		assert_eq!(resources.len(), 1);
		assert_eq!(resources[0]["uri"], "file:///config.json");
		assert_eq!(resources[0]["name"], "Config");
	}

	// -- handle_request: resources/read --

	#[tokio::test]
	async fn test_handle_resources_read() {
		let server = McpServer::new(test_config());

		let server = server.register_resource_fn(
			test_resource_def("file:///data.txt", "Data"),
			|_uri| async move { Ok("file content here".to_string()) },
		);

		let result = server
			.handle_request(
				"resources/read",
				json!({ "uri": "file:///data.txt" }),
			)
			.await
			.unwrap();

		let contents = result["contents"].as_array().unwrap();
		assert_eq!(contents.len(), 1);
		assert_eq!(contents[0]["uri"], "file:///data.txt");
		assert_eq!(contents[0]["text"], "file content here");
		assert_eq!(contents[0]["mimeType"], "text/plain");
	}

	#[tokio::test]
	async fn test_handle_resources_read_not_found() {
		let server = McpServer::new(test_config());

		let err = server
			.handle_request(
				"resources/read",
				json!({ "uri": "file:///nope" }),
			)
			.await;

		assert!(err.is_err());
		let e = err.unwrap_err();
		assert_eq!(e.code(), "MCP_RESOURCE_ERROR");
	}

	// -- handle_request: resources/templates/list --

	#[tokio::test]
	async fn test_handle_resource_templates_list() {
		let server = McpServer::new(test_config());

		let result = server
			.handle_request("resources/templates/list", json!({}))
			.await
			.unwrap();

		let templates = result["resourceTemplates"].as_array().unwrap();
		assert!(templates.is_empty());
	}

	// -- handle_request: prompts/list --

	#[tokio::test]
	async fn test_handle_prompts_list() {
		let server = McpServer::new(test_config());

		let server = server.register_prompt_fn(test_prompt_def("summarize"), |_args| async move {
			Ok(vec![PromptMessage {
				role: "user".into(),
				content: json!("Please summarize"),
			}])
		});

		let result = server
			.handle_request("prompts/list", json!({}))
			.await
			.unwrap();

		let prompts = result["prompts"].as_array().unwrap();
		assert_eq!(prompts.len(), 1);
		assert_eq!(prompts[0]["name"], "summarize");
	}

	// -- handle_request: prompts/get --

	#[tokio::test]
	async fn test_handle_prompts_get() {
		let server = McpServer::new(test_config());

		let server = server.register_prompt_fn(test_prompt_def("greet"), |args| async move {
			let name = args
				.get("name")
				.and_then(|v| v.as_str())
				.unwrap_or("world");
			Ok(vec![PromptMessage {
				role: "user".into(),
				content: json!(format!("Hello, {}!", name)),
			}])
		});

		let result = server
			.handle_request(
				"prompts/get",
				json!({ "name": "greet", "arguments": { "name": "Alice" } }),
			)
			.await
			.unwrap();

		let messages = result["messages"].as_array().unwrap();
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0]["role"], "user");
		assert_eq!(messages[0]["content"], "Hello, Alice!");
	}

	#[tokio::test]
	async fn test_handle_prompts_get_not_found() {
		let server = McpServer::new(test_config());

		let err = server
			.handle_request(
				"prompts/get",
				json!({ "name": "nonexistent", "arguments": {} }),
			)
			.await;

		assert!(err.is_err());
		let e = err.unwrap_err();
		assert_eq!(e.code(), "MCP_PROTOCOL_ERROR");
	}

	// -- handle_request: ping --

	#[tokio::test]
	async fn test_handle_ping() {
		let server = McpServer::new(test_config());

		let result = server.handle_request("ping", json!({})).await.unwrap();

		assert_eq!(result, json!({}));
	}

	// -- handle_request: logging/setLevel --

	#[tokio::test]
	async fn test_handle_set_logging_level() {
		let server = McpServer::new(test_config());

		let result = server
			.handle_request("logging/setLevel", json!({ "level": "debug" }))
			.await
			.unwrap();

		assert_eq!(result, json!({}));

		// Verify the level was updated
		let level = server.logging_level.lock().unwrap().clone();
		assert_eq!(level, "debug");
	}

	// -- handle_request: roots/list --

	#[tokio::test]
	async fn test_handle_roots_list() {
		let server = McpServer::new(test_config());
		let server = server.set_roots(vec![Root {
			uri: "file:///workspace".into(),
			name: Some("workspace".into()),
		}]);

		let result = server
			.handle_request("roots/list", json!({}))
			.await
			.unwrap();

		let roots = result.as_array().unwrap();
		assert_eq!(roots.len(), 1);
		assert_eq!(roots[0]["uri"], "file:///workspace");
	}

	// -- handle_request: completion/complete --

	#[tokio::test]
	async fn test_handle_completion_complete() {
		let server = McpServer::new(test_config());

		let result = server
			.handle_request("completion/complete", json!({}))
			.await
			.unwrap();

		let completion = &result["completion"];
		assert_eq!(completion["values"].as_array().unwrap().len(), 0);
		assert_eq!(completion["hasMore"], false);
		assert_eq!(completion["total"], 0);
	}

	// -- handle_request: unknown method --

	#[tokio::test]
	async fn test_handle_unknown_method() {
		let server = McpServer::new(test_config());

		let err = server
			.handle_request("unknown/method", json!({}))
			.await;

		assert!(err.is_err());
		let e = err.unwrap_err();
		assert_eq!(e.code(), "MCP_PROTOCOL_ERROR");
		assert!(e.to_string().contains("Unknown method: unknown/method"));
	}

	// -- handle_request: initialized (notification) --

	#[tokio::test]
	async fn test_handle_initialized_notification() {
		let server = McpServer::new(test_config());

		let result = server
			.handle_request("initialized", json!({}))
			.await
			.unwrap();

		assert_eq!(result, json!({}));
	}

	// -- Running state --

	#[test]
	fn test_running_state() {
		let server = McpServer::new(test_config());
		assert!(!server.is_running());

		server.set_running(true);
		assert!(server.is_running());

		server.set_running(false);
		assert!(!server.is_running());
	}

	// -- Tool re-registration replaces handler --

	#[tokio::test]
	async fn test_tool_reregistration_replaces_handler() {
		let server = McpServer::new(test_config());

		let server = server.register_tool_fn(test_tool_def("echo"), |_| async move {
			Ok(ToolCallResult {
				content: vec![ContentItem::Text {
					text: "v1".into(),
				}],
				is_error: None,
			})
		});

		// Re-register with same name but different handler
		let server = server.register_tool_fn(test_tool_def("echo"), |_| async move {
			Ok(ToolCallResult {
				content: vec![ContentItem::Text {
					text: "v2".into(),
				}],
				is_error: None,
			})
		});

		assert_eq!(server.tool_count(), 1);

		let result = server
			.handle_request(
				"tools/call",
				json!({ "name": "echo", "arguments": {} }),
			)
			.await
			.unwrap();

		assert_eq!(result["content"][0]["text"], "v2");
	}
}
