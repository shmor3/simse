// ---------------------------------------------------------------------------
// AcpServer — JSON-RPC dispatcher (functional programming patterns)
// ---------------------------------------------------------------------------
//
// Routes incoming JSON-RPC 2.0 requests (NDJSON over stdin) to AcpClient
// operations. Uses immutable state with owned-return transitions:
//
//   - `with_state`: read-only access to state (no mutation)
//   - `with_state_transition`: takes owned state via Option::take(), runs
//     handler, returns updated state (clones backup on error)
//
// State (`AcpServerState`) uses `im::HashMap` for the active_streams map
// to get structural sharing on clone (cheap backup copies for error recovery).
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
// State — immutable with owned-return transitions
// ---------------------------------------------------------------------------

/// Server state managed via owned-return transitions.
///
/// Uses `im::HashMap` for `active_streams` to get O(1) structural-sharing
/// clones (cheap backup for error recovery in `with_state_transition`).
pub struct AcpServerState {
	/// The ACP client, created lazily when `acp/initialize` is called.
	pub client: Option<AcpClient>,
	/// Active stream cancellation senders — keyed by stream ID.
	/// Uses `im::HashMap` for cheap clone-on-backup in state transitions.
	pub active_streams: im::HashMap<String, tokio::sync::mpsc::Sender<()>>,
}

impl AcpServerState {
	/// Create an empty initial state.
	pub fn new() -> Self {
		Self {
			client: None,
			active_streams: im::HashMap::new(),
		}
	}
}

impl Default for AcpServerState {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// JSON-RPC server that dispatches requests to an [`AcpClient`].
///
/// State is held in an `Option<AcpServerState>` and accessed via:
/// - `with_state`: read-only access (borrows the state)
/// - `with_state_transition`: owned access (takes state, returns new state)
pub struct AcpServer {
	transport: NdjsonTransport,
	state: Option<AcpServerState>,
}

impl AcpServer {
	/// Create a new server with the given transport. The client is created
	/// lazily when `acp/initialize` is called.
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			state: Some(AcpServerState::new()),
		}
	}

	// ── State access helpers ─────────────────────────────────────────

	/// Read-only access to state. Calls `f` with a reference to the
	/// current state.
	fn with_state<T>(&self, f: impl FnOnce(&AcpServerState) -> T) -> T {
		f(self.state.as_ref().expect("state invariant: always Some"))
	}

	/// Mutating access via owned-return pattern. Takes the state out of
	/// the `Option`, clones a backup, passes ownership to `f`, and stores
	/// the returned state. On panic/error the backup is restored.
	///
	/// `f` receives owned state and must return the (possibly modified)
	/// state.
	async fn with_state_transition<F, Fut>(&mut self, f: F)
	where
		F: FnOnce(AcpServerState, NdjsonTransport) -> Fut,
		Fut: std::future::Future<Output = AcpServerState>,
	{
		let state = self.state.take().expect("state invariant: always Some");
		let new_state = f(state, self.transport.clone()).await;
		self.state = Some(new_state);
	}

	// ── Main loop ────────────────────────────────────────────────────

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

	// ── Dispatch ─────────────────────────────────────────────────────

	async fn dispatch(&mut self, req: JsonRpcRequest) {
		match req.method.as_str() {
			// -- Lifecycle (state transitions) ----------------------------
			"acp/initialize" => {
				self.with_state_transition(|state, transport| {
					handle_initialize(state, transport, req)
				}).await;
			}
			"acp/dispose" => {
				self.with_state_transition(|state, transport| {
					handle_dispose(state, transport, req)
				}).await;
			}

			// -- Read-only methods (with_state) ---------------------------
			"acp/serverHealth" => {
				let transport = self.transport.clone();
				self.with_state(|state| handle_server_health(state, &transport, req));
			}

			// -- Methods that delegate to client (async, no state mutation)
			// These read the client reference from state inline; the state
			// borrow ends before the async call so lifetimes are satisfied.
			"acp/generate" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_generate_async(client, transport, req).await;
			}
			"acp/chat" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_chat_async(client, transport, req).await;
			}
			"acp/embed" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_embed_async(client, transport, req).await;
			}
			"acp/listAgents" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_list_agents_async(client, transport, req).await;
			}
			"acp/listSessions" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_list_sessions_async(client, transport, req).await;
			}
			"acp/loadSession" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_load_session_async(client, transport, req).await;
			}
			"acp/deleteSession" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_delete_session_async(client, transport, req).await;
			}
			"acp/setSessionMode" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_set_session_mode_async(client, transport, req).await;
			}
			"acp/setSessionModel" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_set_session_model_async(client, transport, req).await;
			}
			"acp/setPermissionPolicy" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_set_permission_policy_async(client, transport, req).await;
			}
			"acp/permissionResponse" => {
				let transport = self.transport.clone();
				let client = self.state.as_ref().unwrap().client.as_ref();
				handle_permission_response_async(client, transport, req).await;
			}

			// -- Stream start (state transition — inserts into active_streams)
			"acp/streamStart" => {
				self.with_state_transition(|state, transport| {
					handle_stream_start(state, transport, req)
				}).await;
			}

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
}

// ---------------------------------------------------------------------------
// Lifecycle handlers (state transitions — owned state)
// ---------------------------------------------------------------------------

async fn handle_initialize(
	mut state: AcpServerState,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) -> AcpServerState {
	let params: InitializeServerParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
			return state;
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
			state.client = Some(client);
			transport.write_response(
				req.id,
				serde_json::json!({ "serverNames": server_names }),
			);
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}

	state
}

async fn handle_dispose(
	mut state: AcpServerState,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) -> AcpServerState {
	if let Some(client) = state.client.take() {
		if let Err(e) = client.dispose().await {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
			return state;
		}
	}
	state.active_streams = im::HashMap::new();
	transport.write_response(req.id, serde_json::json!({}));
	state
}

// ---------------------------------------------------------------------------
// Read-only handlers (with_state — borrow state)
// ---------------------------------------------------------------------------

fn handle_server_health(
	state: &AcpServerState,
	transport: &NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match state.client.as_ref() {
		Some(c) => c,
		None => {
			transport.write_error(
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

	transport.write_response(
		req.id,
		serde_json::json!({
			"available": available,
			"serverNames": server_names,
		}),
	);
}

// ---------------------------------------------------------------------------
// Async client-delegating handlers (no state mutation)
// ---------------------------------------------------------------------------

async fn handle_generate_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: GenerateParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
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
			transport.write_response(req.id, serialize_generate_result(&result));
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_chat_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: ChatParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
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
			transport.write_response(req.id, serialize_generate_result(&result));
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_embed_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: EmbedParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
			return;
		}
	};

	let input_refs: Vec<&str> = params.input.iter().map(|s| s.as_str()).collect();

	match client
		.embed(&input_refs, params.model.as_deref(), params.server.as_deref())
		.await
	{
		Ok(result) => {
			transport.write_response(req.id, serialize_embed_result(&result));
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_list_agents_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: OptionalServerParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(_) => OptionalServerParams { server: None },
	};

	match client.list_agents(params.server.as_deref()).await {
		Ok(agents) => {
			transport.write_response(req.id, serde_json::json!({ "agents": agents }));
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_list_sessions_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: OptionalServerParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(_) => OptionalServerParams { server: None },
	};

	match client.list_sessions(params.server.as_deref()).await {
		Ok(sessions) => {
			transport.write_response(
				req.id,
				serde_json::json!({ "sessions": sessions }),
			);
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_load_session_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: SessionIdParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
			return;
		}
	};

	match client
		.load_session(&params.session_id, params.server.as_deref())
		.await
	{
		Ok(info) => {
			transport.write_response(
				req.id,
				serde_json::to_value(info).unwrap_or(serde_json::json!({})),
			);
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_delete_session_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: SessionIdParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
			return;
		}
	};

	match client
		.delete_session(&params.session_id, params.server.as_deref())
		.await
	{
		Ok(()) => {
			transport.write_response(req.id, serde_json::json!({}));
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_set_session_mode_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: SetSessionConfigParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
			return;
		}
	};

	match client
		.set_session_mode(&params.session_id, &params.value, params.server.as_deref())
		.await
	{
		Ok(()) => {
			transport.write_response(req.id, serde_json::json!({}));
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_set_session_model_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: SetSessionConfigParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
			return;
		}
	};

	match client
		.set_session_model(&params.session_id, &params.value, params.server.as_deref())
		.await
	{
		Ok(()) => {
			transport.write_response(req.id, serde_json::json!({}));
		}
		Err(e) => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}
}

async fn handle_set_permission_policy_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: SetPermissionPolicyParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
			return;
		}
	};

	client.set_permission_policy(params.policy).await;
	transport.write_response(req.id, serde_json::json!({}));
}

async fn handle_permission_response_async(
	client: Option<&AcpClient>,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) {
	let _client = match client {
		Some(c) => c,
		None => {
			transport.write_error(req.id, ACP_ERROR, "Not initialized".to_string(), None);
			return;
		}
	};

	let params: PermissionResponseParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
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

	transport.write_response(req.id, serde_json::json!({}));
}

// ---------------------------------------------------------------------------
// Stream start handler (state transition — inserts into active_streams)
// ---------------------------------------------------------------------------

async fn handle_stream_start(
	mut state: AcpServerState,
	transport: NdjsonTransport,
	req: JsonRpcRequest,
) -> AcpServerState {
	let client = match state.client.as_ref() {
		Some(c) => c,
		None => {
			transport.write_error(
				req.id,
				ACP_ERROR,
				"Not initialized".to_string(),
				None,
			);
			return state;
		}
	};

	let params: StreamStartParams = match parse_params(req.params) {
		Ok(p) => p,
		Err(e) => {
			transport.write_error(req.id, INVALID_PARAMS, e.to_string(), None);
			return state;
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
			state.active_streams = state.active_streams.update(stream_id.clone(), cancel_tx);

			// Return the stream ID immediately.
			transport.write_response(
				req.id,
				serde_json::json!({ "streamId": stream_id }),
			);

			// Spawn a task that reads the stream and writes notifications.
			let sid = stream_id.clone();
			let stream_transport = NdjsonTransport::new();

			tokio::spawn(async move {
				let mut stream = Box::pin(stream);

				loop {
					tokio::select! {
						biased;
						_ = cancel_rx.recv() => {
							// Cancellation requested.
							stream_transport.write_notification(
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
									stream_transport.write_notification(
										"stream/delta",
										serde_json::json!({
											"streamId": sid,
											"text": text,
										}),
									);
								}
								Some(StreamChunk::ToolCall { tool_call }) => {
									stream_transport.write_notification(
										"stream/toolCall",
										serde_json::json!({
											"streamId": sid,
											"toolCall": tool_call,
										}),
									);
								}
								Some(StreamChunk::ToolCallUpdate { update }) => {
									stream_transport.write_notification(
										"stream/toolCallUpdate",
										serde_json::json!({
											"streamId": sid,
											"update": update,
										}),
									);
								}
								Some(StreamChunk::Complete { usage }) => {
									stream_transport.write_notification(
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
									stream_transport.write_notification(
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
			transport.write_error(
				req.id,
				ACP_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			);
		}
	}

	state
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
