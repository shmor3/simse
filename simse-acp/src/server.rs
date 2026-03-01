// ---------------------------------------------------------------------------
// AcpServer — JSON-RPC dispatcher
// ---------------------------------------------------------------------------
//
// Routes incoming JSON-RPC 2.0 requests (NDJSON over stdin) to AcpClient
// operations.  Follows the same pattern as the VectorServer: a main `run()`
// loop, a `dispatch()` match, and handler methods for each method.
//
// Streaming is handled by spawning a tokio task per stream, which reads
// chunks from an AcpStream and writes notifications to stdout.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use futures::StreamExt;
use serde::Deserialize;
use tokio::io::AsyncBufReadExt;

use crate::client::{
	AcpClient, AcpConfig, ChatMessage, ChatOptions, EmbedResult, GenerateOptions, GenerateResult,
	McpServerEntry, ServerEntry, StreamOptions,
};
use crate::error::AcpError;
use crate::protocol::*;
use crate::transport::NdjsonTransport;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// JSON-RPC server that dispatches requests to an [`AcpClient`].
pub struct AcpServer {
	transport: NdjsonTransport,
	client: Option<AcpClient>,
	/// Active stream cancellation senders — keyed by stream ID.
	active_streams: HashMap<String, tokio::sync::mpsc::Sender<()>>,
}

impl AcpServer {
	/// Create a new server with the given transport. The client is created
	/// lazily when `acp/initialize` is called.
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			client: None,
			active_streams: HashMap::new(),
		}
	}

	/// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
	pub async fn run(&mut self) -> Result<(), AcpError> {
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
			// -- Lifecycle -----------------------------------------------
			"acp/initialize" => self.handle_initialize(req).await,
			"acp/dispose" => self.handle_dispose(req).await,
			"acp/serverHealth" => self.handle_server_health(req).await,

			// -- Generation ----------------------------------------------
			"acp/generate" => self.handle_generate(req).await,
			"acp/chat" => self.handle_chat(req).await,
			"acp/streamStart" => self.handle_stream_start(req).await,
			"acp/embed" => self.handle_embed(req).await,

			// -- Agent discovery -----------------------------------------
			"acp/listAgents" => self.handle_list_agents(req).await,

			// -- Session management --------------------------------------
			"acp/listSessions" => self.handle_list_sessions(req).await,
			"acp/loadSession" => self.handle_load_session(req).await,
			"acp/deleteSession" => self.handle_delete_session(req).await,
			"acp/setSessionMode" => self.handle_set_session_mode(req).await,
			"acp/setSessionModel" => self.handle_set_session_model(req).await,

			// -- Permission management -----------------------------------
			"acp/setPermissionPolicy" => self.handle_set_permission_policy(req).await,
			"acp/permissionResponse" => self.handle_permission_response(req).await,

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

	// ── Client accessor ──────────────────────────────────────────────────

	fn require_client(&self) -> Option<&AcpClient> {
		self.client.as_ref()
	}

	// ── Initialize ───────────────────────────────────────────────────────

	async fn handle_initialize(&mut self, req: JsonRpcRequest) {
		let params: InitializeServerParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		// Build AcpConfig from the initialization params.
		let servers: Vec<ServerEntry> = params
			.servers
			.into_iter()
			.map(|s| ServerEntry {
				name: s.name,
				command: s.command,
				args: s.args.unwrap_or_default(),
				cwd: s.cwd,
				env: s.env.unwrap_or_default(),
				default_agent: s.default_agent,
				timeout_ms: s.timeout_ms,
				permission_policy: s.permission_policy,
			})
			.collect();

		let mcp_servers: Vec<McpServerEntry> = params
			.mcp_servers
			.unwrap_or_default()
			.into_iter()
			.map(|m| McpServerEntry {
				name: m.name,
				config: m.config,
			})
			.collect();

		let config = AcpConfig {
			servers,
			default_server: params.default_server,
			default_agent: params.default_agent,
			mcp_servers,
		};

		match AcpClient::new(config).await {
			Ok(client) => {
				let server_names = client.server_names();
				self.client = Some(client);
				self.transport.write_response(
					req.id,
					serde_json::json!({ "serverNames": server_names }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Dispose ──────────────────────────────────────────────────────────

	async fn handle_dispose(&mut self, req: JsonRpcRequest) {
		if let Some(client) = self.client.take() {
			if let Err(e) = client.dispose().await {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
				return;
			}
		}
		self.active_streams.clear();
		self.transport
			.write_response(req.id, serde_json::json!({}));
	}

	// ── Server health ────────────────────────────────────────────────────

	async fn handle_server_health(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: ServerHealthParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(_) => ServerHealthParams { server: None },
		};

		let available = client.is_available(params.server.as_deref());
		let server_names = client.server_names();

		self.transport.write_response(
			req.id,
			serde_json::json!({
				"available": available,
				"serverNames": server_names,
			}),
		);
	}

	// ── Generate ─────────────────────────────────────────────────────────

	async fn handle_generate(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: GenerateParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let options = GenerateOptions {
			agent_id: params.agent_id,
			server_name: params.server_name,
			system_prompt: params.system_prompt,
			sampling: params.sampling,
			session_id: params.session_id,
		};

		match client.generate(&params.prompt, options).await {
			Ok(result) => {
				self.transport.write_response(
					req.id,
					serialize_generate_result(&result),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Chat ─────────────────────────────────────────────────────────────

	async fn handle_chat(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: ChatParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let messages: Vec<ChatMessage> = params
			.messages
			.into_iter()
			.map(|m| ChatMessage {
				role: m.role,
				content: m.content,
			})
			.collect();

		let options = ChatOptions {
			agent_id: params.agent_id,
			server_name: params.server_name,
			sampling: params.sampling,
			session_id: params.session_id,
		};

		match client.chat(&messages, options).await {
			Ok(result) => {
				self.transport.write_response(
					req.id,
					serialize_generate_result(&result),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Stream start ─────────────────────────────────────────────────────

	async fn handle_stream_start(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: StreamStartParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let options = StreamOptions {
			agent_id: params.agent_id,
			server_name: params.server_name,
			system_prompt: params.system_prompt,
			sampling: params.sampling,
			session_id: params.session_id,
			stream_timeout_ms: params.stream_timeout_ms,
		};

		match client.generate_stream(&params.prompt, options).await {
			Ok(stream) => {
				let stream_id = uuid::Uuid::new_v4().to_string();

				// Create a cancellation channel for this stream.
				let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::channel::<()>(1);
				self.active_streams
					.insert(stream_id.clone(), cancel_tx);

				// Return the stream ID immediately.
				self.transport.write_response(
					req.id,
					serde_json::json!({ "streamId": stream_id }),
				);

				// Spawn a task that reads the stream and writes notifications.
				let sid = stream_id.clone();
				let transport = NdjsonTransport::new();

				tokio::spawn(async move {
					let mut stream = Box::pin(stream);

					loop {
						tokio::select! {
							biased;
							_ = cancel_rx.recv() => {
								// Cancellation requested.
								transport.write_notification(
									"stream/complete",
									serde_json::json!({
										"streamId": sid,
										"cancelled": true,
									}),
								);
								break;
							}
							chunk = stream.next() => {
								match chunk {
									Some(StreamChunk::Delta { text }) => {
										transport.write_notification(
											"stream/delta",
											serde_json::json!({
												"streamId": sid,
												"text": text,
											}),
										);
									}
									Some(StreamChunk::ToolCall { tool_call }) => {
										transport.write_notification(
											"stream/toolCall",
											serde_json::json!({
												"streamId": sid,
												"toolCall": tool_call,
											}),
										);
									}
									Some(StreamChunk::ToolCallUpdate { update }) => {
										transport.write_notification(
											"stream/toolCallUpdate",
											serde_json::json!({
												"streamId": sid,
												"update": update,
											}),
										);
									}
									Some(StreamChunk::Complete { usage }) => {
										transport.write_notification(
											"stream/complete",
											serde_json::json!({
												"streamId": sid,
												"usage": usage,
											}),
										);
										break;
									}
									None => {
										// Stream ended without Complete chunk.
										transport.write_notification(
											"stream/complete",
											serde_json::json!({
												"streamId": sid,
											}),
										);
										break;
									}
								}
							}
						}
					}
				});
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Embed ────────────────────────────────────────────────────────────

	async fn handle_embed(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: EmbedParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		let input_refs: Vec<&str> = params.input.iter().map(|s| s.as_str()).collect();

		match client
			.embed(&input_refs, params.model.as_deref(), params.server.as_deref())
			.await
		{
			Ok(result) => {
				self.transport.write_response(
					req.id,
					serialize_embed_result(&result),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── List agents ──────────────────────────────────────────────────────

	async fn handle_list_agents(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: OptionalServerParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(_) => OptionalServerParams { server: None },
		};

		match client.list_agents(params.server.as_deref()).await {
			Ok(agents) => {
				self.transport.write_response(
					req.id,
					serde_json::json!({ "agents": agents }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── List sessions ────────────────────────────────────────────────────

	async fn handle_list_sessions(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: OptionalServerParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(_) => OptionalServerParams { server: None },
		};

		match client.list_sessions(params.server.as_deref()).await {
			Ok(sessions) => {
				self.transport.write_response(
					req.id,
					serde_json::json!({ "sessions": sessions }),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Load session ─────────────────────────────────────────────────────

	async fn handle_load_session(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: SessionIdParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		match client
			.load_session(&params.session_id, params.server.as_deref())
			.await
		{
			Ok(info) => {
				self.transport.write_response(
					req.id,
					serde_json::to_value(info).unwrap_or(serde_json::json!({})),
				);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Delete session ───────────────────────────────────────────────────

	async fn handle_delete_session(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: SessionIdParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		match client
			.delete_session(&params.session_id, params.server.as_deref())
			.await
		{
			Ok(()) => {
				self.transport
					.write_response(req.id, serde_json::json!({}));
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Set session mode ─────────────────────────────────────────────────

	async fn handle_set_session_mode(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: SetSessionConfigParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		match client
			.set_session_mode(&params.session_id, &params.value, params.server.as_deref())
			.await
		{
			Ok(()) => {
				self.transport
					.write_response(req.id, serde_json::json!({}));
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Set session model ────────────────────────────────────────────────

	async fn handle_set_session_model(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: SetSessionConfigParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		match client
			.set_session_model(&params.session_id, &params.value, params.server.as_deref())
			.await
		{
			Ok(()) => {
				self.transport
					.write_response(req.id, serde_json::json!({}));
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	// ── Set permission policy ────────────────────────────────────────────

	async fn handle_set_permission_policy(&mut self, req: JsonRpcRequest) {
		let client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: SetPermissionPolicyParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		client.set_permission_policy(params.policy).await;
		self.transport
			.write_response(req.id, serde_json::json!({}));
	}

	// ── Permission response ──────────────────────────────────────────────

	async fn handle_permission_response(&mut self, req: JsonRpcRequest) {
		let _client = match self.require_client() {
			Some(c) => c,
			None => {
				self.transport.write_error(
					req.id,
					ACP_ERROR,
					"Not initialized".to_string(),
					None,
				);
				return;
			}
		};

		let params: PermissionResponseParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
				return;
			}
		};

		// Build permission result from the option ID.
		let result = PermissionResult {
			outcome: PermissionOutcome {
				outcome: "selected".to_string(),
				option_id: Some(params.option_id),
			},
		};

		// Permission responses are forwarded directly to the connection.
		// Since AcpClient doesn't expose respond_to_permission directly,
		// we acknowledge receipt. The TS layer handles the permission flow
		// through the connection's notification system.
		tracing::debug!(
			"Permission response for request {}: {:?}",
			params.request_id,
			result
		);

		self.transport
			.write_response(req.id, serde_json::json!({}));
	}
}

// ---------------------------------------------------------------------------
// Param types
// ---------------------------------------------------------------------------

fn parse_params<T: serde::de::DeserializeOwned>(
	params: serde_json::Value,
) -> Result<T, AcpError> {
	serde_json::from_value(params)
		.map_err(|e| AcpError::Serialization(format!("Invalid params: {}", e)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerConfigParams {
	name: String,
	command: String,
	#[serde(default)]
	args: Option<Vec<String>>,
	#[serde(default)]
	cwd: Option<String>,
	#[serde(default)]
	env: Option<HashMap<String, String>>,
	#[serde(default)]
	default_agent: Option<String>,
	#[serde(default)]
	timeout_ms: Option<u64>,
	#[serde(default)]
	permission_policy: Option<PermissionPolicy>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpServerConfigParams {
	name: String,
	#[serde(default)]
	config: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeServerParams {
	servers: Vec<ServerConfigParams>,
	#[serde(default)]
	default_server: Option<String>,
	#[serde(default)]
	default_agent: Option<String>,
	#[serde(default)]
	mcp_servers: Option<Vec<McpServerConfigParams>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateParams {
	prompt: String,
	#[serde(default)]
	agent_id: Option<String>,
	#[serde(default)]
	server_name: Option<String>,
	#[serde(default)]
	system_prompt: Option<String>,
	#[serde(default)]
	sampling: Option<SamplingParams>,
	#[serde(default)]
	session_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessageParams {
	role: String,
	content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatParams {
	messages: Vec<ChatMessageParams>,
	#[serde(default)]
	agent_id: Option<String>,
	#[serde(default)]
	server_name: Option<String>,
	#[serde(default)]
	sampling: Option<SamplingParams>,
	#[serde(default)]
	session_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamStartParams {
	prompt: String,
	#[serde(default)]
	agent_id: Option<String>,
	#[serde(default)]
	server_name: Option<String>,
	#[serde(default)]
	system_prompt: Option<String>,
	#[serde(default)]
	sampling: Option<SamplingParams>,
	#[serde(default)]
	session_id: Option<String>,
	#[serde(default)]
	stream_timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbedParams {
	input: Vec<String>,
	#[serde(default)]
	model: Option<String>,
	#[serde(default)]
	server: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OptionalServerParams {
	#[serde(default)]
	server: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerHealthParams {
	#[serde(default)]
	server: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionIdParams {
	session_id: String,
	#[serde(default)]
	server: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetSessionConfigParams {
	session_id: String,
	value: String,
	#[serde(default)]
	server: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetPermissionPolicyParams {
	policy: PermissionPolicy,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionResponseParams {
	request_id: u64,
	option_id: String,
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn serialize_generate_result(result: &GenerateResult) -> serde_json::Value {
	serde_json::json!({
		"content": result.content,
		"agentId": result.agent_id,
		"serverName": result.server_name,
		"sessionId": result.session_id,
		"usage": result.usage,
		"stopReason": result.stop_reason,
	})
}

fn serialize_embed_result(result: &EmbedResult) -> serde_json::Value {
	serde_json::json!({
		"embeddings": result.embeddings,
		"agentId": result.agent_id,
		"serverName": result.server_name,
		"usage": result.usage,
	})
}
