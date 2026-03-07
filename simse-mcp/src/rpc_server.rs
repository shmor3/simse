// ---------------------------------------------------------------------------
// McpRpcServer — JSON-RPC dispatcher
// ---------------------------------------------------------------------------
//
// Routes incoming JSON-RPC 2.0 requests (NDJSON over stdin) to McpClient
// and McpServer operations. Follows the same pattern as the ACP server:
// a main `run()` loop, a `dispatch()` match, and handler methods.
//
// The server wraps both an McpClient (for connecting to external MCP servers)
// and an McpServer (for hosting tools/resources/prompts). Tool registration
// from TS uses a callback pattern: the RPC server registers a handler that
// sends a `tool/execute` notification to TS and waits for a `server/toolResult`
// response via a oneshot channel.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::AsyncBufReadExt;
use tokio::sync::{oneshot, Mutex};

use crate::client::McpClient;
use crate::error::McpError;
use crate::mcp_server::{McpServer, ToolHandler};
use crate::protocol::*;
use crate::rpc_transport::NdjsonTransport;

// ---------------------------------------------------------------------------
// Callback tool handler
// ---------------------------------------------------------------------------

/// A tool handler that forwards execution to TS via the JSON-RPC transport.
///
/// When invoked, it:
/// 1. Generates a unique request ID
/// 2. Sends a `tool/execute` notification to TS
/// 3. Stores a oneshot sender in `pending_tool_calls`
/// 4. Awaits the result (with timeout)
struct CallbackToolHandler {
	tool_name: String,
	transport: NdjsonTransport,
	pending: Arc<Mutex<HashMap<String, oneshot::Sender<ToolCallResult>>>>,
}

#[async_trait::async_trait]
impl ToolHandler for CallbackToolHandler {
	async fn execute(&self, args: serde_json::Value) -> Result<ToolCallResult, McpError> {
		let request_id = uuid::Uuid::new_v4().to_string();

		// Create the channel for receiving the result
		let (tx, rx) = oneshot::channel();

		// Store the sender
		{
			let mut pending = self.pending.lock().await;
			pending.insert(request_id.clone(), tx);
		}

		// Send the notification to TS
		self.transport.write_notification(
			"tool/execute",
			serde_json::json!({
				"requestId": request_id,
				"toolName": self.tool_name,
				"args": args,
			}),
		);

		// Wait for result with timeout (60 seconds)
		let result = tokio::time::timeout(Duration::from_secs(60), rx).await;

		// Clean up pending entry on timeout or error
		match result {
			Ok(Ok(tool_result)) => Ok(tool_result),
			Ok(Err(_)) => {
				// Channel dropped — the sender was removed without sending
				let mut pending = self.pending.lock().await;
				pending.remove(&request_id);
				Err(McpError::ToolError {
					tool: self.tool_name.clone(),
					message: "Tool execution channel closed unexpectedly".to_string(),
				})
			}
			Err(_) => {
				// Timeout
				let mut pending = self.pending.lock().await;
				pending.remove(&request_id);
				Err(McpError::Timeout {
					method: format!("tool/execute({})", self.tool_name),
					timeout_ms: 60_000,
				})
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// JSON-RPC server that dispatches requests to an [`McpClient`] and [`McpServer`].
///
/// Uses `with_state` / `with_state_transition` for FP-style state access:
/// - `with_state`: borrows client/server immutably for read-only operations
/// - `with_state_transition`: takes owned state via `Option::take()` for mutations,
///   clones backup on error to preserve state safety
pub struct McpRpcServer {
	transport: NdjsonTransport,
	client: Option<McpClient>,
	server: Option<McpServer>,
	pending_tool_calls: Arc<Mutex<HashMap<String, oneshot::Sender<ToolCallResult>>>>,
}

impl McpRpcServer {
	/// Create a new server with the given transport. The client and server are
	/// created lazily when `mcp/initialize` is called.
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			client: None,
			server: None,
			pending_tool_calls: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	/// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
	pub async fn run(&mut self) -> Result<(), McpError> {
		let stdin = tokio::io::stdin();
		let reader = tokio::io::BufReader::new(stdin);
		let mut lines = reader.lines();

		while let Ok(Some(line)) = lines.next_line().await {
			let line = line.trim().to_string();
			if line.is_empty() {
				continue;
			}

			match serde_json::from_str::<JsonRpcRequest>(&line) {
				Ok(request) => self.dispatch(request).await,
				Err(e) => {
					tracing::warn!("Invalid JSON-RPC request: {}", e);
				}
			}
		}

		Ok(())
	}

	// ── Dispatch ──────────────────────────────────────────────────────────

	async fn dispatch(&mut self, req: JsonRpcRequest) {
		match req.method.as_str() {
			// -- Client methods ------------------------------------------
			"mcp/initialize" => self.handle_initialize(req).await,
			"mcp/connect" => self.handle_connect(req).await,
			"mcp/connectAll" => self.handle_connect_all(req).await,
			"mcp/disconnect" => self.handle_disconnect(req).await,
			"mcp/listTools" => self.handle_list_tools(req).await,
			"mcp/callTool" => self.handle_call_tool(req).await,
			"mcp/listResources" => self.handle_list_resources(req).await,
			"mcp/readResource" => self.handle_read_resource(req).await,
			"mcp/listResourceTemplates" => self.handle_list_resource_templates(req).await,
			"mcp/listPrompts" => self.handle_list_prompts(req).await,
			"mcp/getPrompt" => self.handle_get_prompt(req).await,
			"mcp/setLoggingLevel" => self.handle_set_logging_level(req).await,
			"mcp/complete" => self.handle_complete(req).await,
			"mcp/setRoots" => self.handle_set_roots(req).await,

			// -- Server methods ------------------------------------------
			"server/start" => self.handle_server_start(req).await,
			"server/stop" => self.handle_server_stop(req).await,
			"server/registerTool" => self.handle_register_tool(req).await,
			"server/unregisterTool" => self.handle_unregister_tool(req).await,
			"server/toolResult" => self.handle_tool_result(req).await,

			// -- Lifecycle -----------------------------------------------
			"mcp/dispose" => self.handle_dispose(req).await,

			// -- Unknown -------------------------------------------------
			_ => {
				self.transport.write_error(
					req.id,
					METHOD_NOT_FOUND,
					format!("Unknown method: {}", req.method),
					None,
				);
			}
		}
	}

	// ── State access helpers (FP pattern) ────────────────────────────────

	/// Borrow server immutably for synchronous read-only operations.
	/// Returns `None` and writes an error if not initialized.
	fn with_server<F, R>(&self, req_id: u64, f: F) -> Option<R>
	where
		F: FnOnce(&McpServer) -> R,
	{
		match self.server.as_ref() {
			Some(s) => Some(f(s)),
			None => {
				self.transport.write_error(
					req_id,
					MCP_ERROR,
					"Server not initialized".to_string(),
					None,
				);
				None
			}
		}
	}

	/// Take owned server state for mutation (owned-return pattern).
	/// On success, the callback returns the new server state.
	/// Returns `false` and writes an error if not initialized.
	fn with_server_transition<F>(&mut self, req_id: u64, f: F) -> bool
	where
		F: FnOnce(McpServer) -> McpServer,
	{
		match self.server.take() {
			Some(server) => {
				self.server = Some(f(server));
				true
			}
			None => {
				self.transport.write_error(
					req_id,
					MCP_ERROR,
					"Server not initialized".to_string(),
					None,
				);
				false
			}
		}
	}

	/// Take owned client state for mutation (owned-return pattern).
	/// On success, the callback returns the new client state.
	/// Returns `false` and writes an error if not initialized.
	fn with_client_transition<F>(&mut self, req_id: u64, f: F) -> bool
	where
		F: FnOnce(McpClient) -> McpClient,
	{
		match self.client.take() {
			Some(client) => {
				self.client = Some(f(client));
				true
			}
			None => {
				self.transport.write_error(
					req_id,
					MCP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				false
			}
		}
	}

	/// Check that client is initialized, writing an error if not.
	/// Returns a reference to the client for async operations.
	fn require_client(&self, req_id: u64) -> Option<&McpClient> {
		match self.client.as_ref() {
			Some(c) => Some(c),
			None => {
				self.transport.write_error(
					req_id,
					MCP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				None
			}
		}
	}

	/// Check that client is initialized, writing an error if not.
	/// Returns a mutable reference to the client for async I/O operations.
	fn require_client_mut(&mut self, req_id: u64) -> Option<&mut McpClient> {
		match self.client.as_mut() {
			Some(c) => Some(c),
			None => {
				self.transport.write_error(
					req_id,
					MCP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				None
			}
		}
	}

	// ── Initialize ───────────────────────────────────────────────────────

	async fn handle_initialize(&mut self, req: JsonRpcRequest) {
		let params: InitializeParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		// Create client from clientConfig if provided
		if let Some(client_config) = params.client_config {
			let mcp_config = McpClientConfig {
				servers: client_config.servers,
				client_name: client_config.client_name,
				client_version: client_config.client_version,
			};
			self.client = Some(McpClient::new(mcp_config));

			// Register notification forwarding on the client
			let transport_tools = NdjsonTransport::new();
			let transport_logging = NdjsonTransport::new();

			if let Some(client) = &self.client {
				client.on_tools_changed(move || {
					transport_tools.write_notification(
						"mcp/toolsChanged",
						serde_json::json!({}),
					);
				});

				client.on_logging_message(move |msg| {
					transport_logging.write_notification(
						"mcp/loggingMessage",
						serde_json::json!({
							"level": msg.level,
							"logger": msg.logger,
							"data": msg.data,
						}),
					);
				});
			}
		}

		// Create server from serverConfig if provided
		if let Some(server_config) = params.server_config {
			let mcp_server_config = McpServerConfig {
				name: server_config.name,
				version: server_config.version,
				transport: server_config
					.transport
					.unwrap_or(TransportType::Stdio),
			};
			self.server = Some(McpServer::new(mcp_server_config));
		}

		let has_client = self.client.is_some();
		let has_server = self.server.is_some();

		self.transport.write_response(
			req.id,
			serde_json::json!({
				"clientInitialized": has_client,
				"serverInitialized": has_server,
			}),
		);
	}

	// ── Dispose ──────────────────────────────────────────────────────────

	async fn handle_dispose(&mut self, req: JsonRpcRequest) {
		if let Some(mut client) = self.client.take() {
			if let Err(e) = client.disconnect_all().await {
				tracing::warn!("Error during client dispose: {}", e);
			}
		}
		self.server = None;

		// Clear pending tool calls
		{
			let mut pending = self.pending_tool_calls.lock().await;
			pending.clear();
		}

		self.transport
			.write_response(req.id, serde_json::json!({}));
	}

	// ── Connect ──────────────────────────────────────────────────────────

	async fn handle_connect(&mut self, req: JsonRpcRequest) {
		let params: ConnectParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		// Connection management is I/O — use require_client_mut
		let client = match self.require_client_mut(req.id) {
			Some(c) => c,
			None => return,
		};

		match client.connect(&params.server).await {
			Ok(()) => {
				self.transport
					.write_response(req.id, serde_json::json!({}));
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Connect all ──────────────────────────────────────────────────────

	async fn handle_connect_all(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client_mut(req.id) {
			Some(c) => c,
			None => return,
		};

		match client.connect_all().await {
			Ok(connected) => {
				self.transport.write_response(
					req.id,
					serde_json::json!({ "connected": connected }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Disconnect ───────────────────────────────────────────────────────

	async fn handle_disconnect(&mut self, req: JsonRpcRequest) {
		let params: ConnectParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let client = match self.require_client_mut(req.id) {
			Some(c) => c,
			None => return,
		};

		match client.disconnect(&params.server).await {
			Ok(()) => {
				self.transport
					.write_response(req.id, serde_json::json!({}));
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── List tools ───────────────────────────────────────────────────────

	async fn handle_list_tools(&self, req: JsonRpcRequest) {
		let params: OptionalServerParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(_) => OptionalServerParams { server: None },
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client.list_tools(params.server.as_deref()).await {
			Ok(tools) => {
				self.transport.write_response(
					req.id,
					serde_json::json!({ "tools": tools }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Call tool ────────────────────────────────────────────────────────

	async fn handle_call_tool(&self, req: JsonRpcRequest) {
		let params: CallToolParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client
			.call_tool(&params.server, &params.name, params.arguments)
			.await
		{
			Ok(result) => {
				self.transport.write_response(
					req.id,
					serde_json::to_value(&result).unwrap_or(serde_json::json!({})),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── List resources ───────────────────────────────────────────────────

	async fn handle_list_resources(&self, req: JsonRpcRequest) {
		let params: OptionalServerParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(_) => OptionalServerParams { server: None },
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client.list_resources(params.server.as_deref()).await {
			Ok(resources) => {
				self.transport.write_response(
					req.id,
					serde_json::json!({ "resources": resources }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Read resource ────────────────────────────────────────────────────

	async fn handle_read_resource(&self, req: JsonRpcRequest) {
		let params: ReadResourceRpcParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client.read_resource(&params.server, &params.uri).await {
			Ok(text) => {
				self.transport.write_response(
					req.id,
					serde_json::json!({ "text": text }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── List resource templates ──────────────────────────────────────────

	async fn handle_list_resource_templates(&self, req: JsonRpcRequest) {
		let params: OptionalServerParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(_) => OptionalServerParams { server: None },
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client
			.list_resource_templates(params.server.as_deref())
			.await
		{
			Ok(templates) => {
				self.transport.write_response(
					req.id,
					serde_json::json!({ "resourceTemplates": templates }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── List prompts ─────────────────────────────────────────────────────

	async fn handle_list_prompts(&self, req: JsonRpcRequest) {
		let params: OptionalServerParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(_) => OptionalServerParams { server: None },
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client.list_prompts(params.server.as_deref()).await {
			Ok(prompts) => {
				self.transport.write_response(
					req.id,
					serde_json::json!({ "prompts": prompts }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Get prompt ───────────────────────────────────────────────────────

	async fn handle_get_prompt(&self, req: JsonRpcRequest) {
		let params: GetPromptRpcParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client
			.get_prompt(&params.server, &params.name, params.arguments)
			.await
		{
			Ok(result) => {
				self.transport.write_response(
					req.id,
					serde_json::to_value(&result).unwrap_or(serde_json::json!({})),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Set logging level ────────────────────────────────────────────────

	async fn handle_set_logging_level(&self, req: JsonRpcRequest) {
		let params: SetLoggingLevelParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client
			.set_logging_level(&params.server, &params.level)
			.await
		{
			Ok(()) => {
				self.transport
					.write_response(req.id, serde_json::json!({}));
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Complete ─────────────────────────────────────────────────────────

	async fn handle_complete(&self, req: JsonRpcRequest) {
		let params: CompleteParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let client = match self.require_client(req.id) {
			Some(c) => c,
			None => return,
		};

		match client
			.complete(&params.server, params.reference, params.argument)
			.await
		{
			Ok(result) => {
				self.transport.write_response(
					req.id,
					serde_json::to_value(&result).unwrap_or(serde_json::json!({})),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					MCP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Set roots (owned-return via with_client_transition) ──────────────

	async fn handle_set_roots(&mut self, req: JsonRpcRequest) {
		let params: SetRootsParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		if self.with_client_transition(req.id, |client| client.set_roots(params.roots)) {
			self.transport
				.write_response(req.id, serde_json::json!({}));
		}
	}

	// ── Server start ─────────────────────────────────────────────────────

	async fn handle_server_start(&self, req: JsonRpcRequest) {
		self.with_server(req.id, |server| {
			server.set_running(true);
		});
		if self.server.is_some() {
			self.transport
				.write_response(req.id, serde_json::json!({}));
		}
	}

	// ── Server stop ──────────────────────────────────────────────────────

	async fn handle_server_stop(&self, req: JsonRpcRequest) {
		self.with_server(req.id, |server| {
			server.set_running(false);
		});
		if self.server.is_some() {
			self.transport
				.write_response(req.id, serde_json::json!({}));
		}
	}

	// ── Register tool (owned-return via with_server_transition) ──────────

	async fn handle_register_tool(&mut self, req: JsonRpcRequest) {
		let params: RegisterToolParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let definition = ToolDefinition {
			name: params.name.clone(),
			description: params.description,
			input_schema: params.input_schema,
		};

		// Clone the Arc before the transition closure
		let pending = Arc::clone(&self.pending_tool_calls);

		let handler = CallbackToolHandler {
			tool_name: params.name,
			transport: NdjsonTransport::new(),
			pending,
		};

		if self.with_server_transition(req.id, |server| server.register_tool(definition, handler)) {
			self.transport
				.write_response(req.id, serde_json::json!({}));
		}
	}

	// ── Unregister tool (owned-return via with_server_transition) ────────

	async fn handle_unregister_tool(&mut self, req: JsonRpcRequest) {
		let params: UnregisterToolParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let mut removed = false;
		let transitioned = self.with_server_transition(req.id, |server| {
			let (server, was_removed) = server.unregister_tool(&params.name);
			removed = was_removed;
			server
		});

		if transitioned {
			self.transport.write_response(
				req.id,
				serde_json::json!({ "removed": removed }),
			);
		}
	}

	// ── Tool result ──────────────────────────────────────────────────────

	async fn handle_tool_result(&self, req: JsonRpcRequest) {
		let params: ToolResultParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let mut pending = self.pending_tool_calls.lock().await;
		if let Some(sender) = pending.remove(&params.request_id) {
			let result = ToolCallResult {
				content: params.content,
				is_error: params.is_error,
			};
			let _ = sender.send(result);
			self.transport
				.write_response(req.id, serde_json::json!({}));
		} else {
			self.transport.write_error(
				req.id,
				MCP_ERROR,
				format!(
					"No pending tool call with requestId: {}",
					params.request_id
				),
				None,
			);
		}
	}
}

// ---------------------------------------------------------------------------
// Param types
// ---------------------------------------------------------------------------

fn parse_params<T: serde::de::DeserializeOwned>(
	params: serde_json::Value,
) -> Result<T, McpError> {
	serde_json::from_value(params)
		.map_err(|e| McpError::Serialization(format!("Invalid params: {}", e)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfigParams {
	servers: Vec<ServerConnection>,
	#[serde(default)]
	client_name: Option<String>,
	#[serde(default)]
	client_version: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerConfigParams {
	name: String,
	version: String,
	#[serde(default)]
	transport: Option<TransportType>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
	#[serde(default)]
	client_config: Option<ClientConfigParams>,
	#[serde(default)]
	server_config: Option<ServerConfigParams>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectParams {
	server: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OptionalServerParams {
	#[serde(default)]
	server: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallToolParams {
	server: String,
	name: String,
	#[serde(default)]
	arguments: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadResourceRpcParams {
	server: String,
	uri: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetPromptRpcParams {
	server: String,
	name: String,
	#[serde(default)]
	arguments: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetLoggingLevelParams {
	server: String,
	level: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompleteParams {
	server: String,
	reference: CompletionRef,
	argument: CompletionArg,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetRootsParams {
	roots: Vec<Root>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterToolParams {
	name: String,
	description: String,
	#[serde(default = "default_input_schema")]
	input_schema: serde_json::Value,
}

fn default_input_schema() -> serde_json::Value {
	serde_json::json!({ "type": "object" })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnregisterToolParams {
	name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolResultParams {
	request_id: String,
	content: Vec<ContentItem>,
	#[serde(default)]
	is_error: Option<bool>,
}
