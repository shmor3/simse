// ---------------------------------------------------------------------------
// AcpClient — orchestration layer for multi-server ACP connections
// ---------------------------------------------------------------------------
//
// The main client that consumers interact with. Manages:
//   - A pool of connections (one per configured server)
//   - Per-server circuit breakers and health monitors
//   - Session caching (models/modes from session/new)
//   - Resilient request execution with retry + circuit breaker
//   - Agent discovery (synthetic, from config)
//   - Streaming via AcpStream
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::connection::{AcpConnection, ConnectionConfig};
use crate::error::AcpError;
use crate::protocol::{
	AgentInfoEntry, ContentBlock, PermissionPolicy, SamplingParams, SessionInfo,
	SessionListEntry, SessionPromptResult, SetConfigOptionParams, StopReason,
	StreamChunk, TokenUsage,
};
use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, HealthMonitor, RetryConfig, retry};
use crate::stream::{AcpStream, create_stream, parse_session_update};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Top-level configuration for an [`AcpClient`].
#[derive(Debug, Clone)]
pub struct AcpConfig {
	/// Server definitions — one connection will be created per entry.
	pub servers: Vec<ServerEntry>,
	/// Default server to use when none is specified. Falls back to the
	/// first server if `None`.
	pub default_server: Option<String>,
	/// Default agent ID to use when none is specified at call site.
	pub default_agent: Option<String>,
	/// MCP server configurations to pass to `session/new`.
	pub mcp_servers: Vec<McpServerEntry>,
}

/// Configuration for a single ACP server.
#[derive(Debug, Clone)]
pub struct ServerEntry {
	/// Unique name identifying this server.
	pub name: String,
	/// Command to spawn (e.g. path to the ACP agent binary).
	pub command: String,
	/// Arguments passed to the command.
	pub args: Vec<String>,
	/// Working directory for the child process.
	pub cwd: Option<String>,
	/// Additional environment variables.
	pub env: HashMap<String, String>,
	/// Default agent ID for this server. Falls back to the server name.
	pub default_agent: Option<String>,
	/// Request timeout in milliseconds. Overrides the connection default.
	pub timeout_ms: Option<u64>,
	/// Permission policy for tool-use requests on this server.
	pub permission_policy: Option<PermissionPolicy>,
}

/// MCP server entry passed through to `session/new`.
#[derive(Debug, Clone)]
pub struct McpServerEntry {
	/// Unique name for the MCP server.
	pub name: String,
	/// Opaque configuration forwarded to the ACP agent.
	pub config: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Options types
// ---------------------------------------------------------------------------

/// Options for a [`generate`](AcpClient::generate) call.
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
	/// Agent to target. Resolution: option → server default → global default → server name.
	pub agent_id: Option<String>,
	/// Server name. Falls back to default_server → first server.
	pub server_name: Option<String>,
	/// System prompt prepended as a text content block.
	pub system_prompt: Option<String>,
	/// Sampling parameters (temperature, max_tokens, etc.).
	pub sampling: Option<SamplingParams>,
	/// Existing session ID to reuse instead of creating a new one.
	pub session_id: Option<String>,
}

/// A single chat message.
#[derive(Debug, Clone)]
pub struct ChatMessage {
	/// Role: "system", "user", or "assistant".
	pub role: String,
	/// Content blocks.
	pub content: Vec<ContentBlock>,
}

/// Options for a [`chat`](AcpClient::chat) call.
#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
	/// Agent to target.
	pub agent_id: Option<String>,
	/// Server name.
	pub server_name: Option<String>,
	/// Sampling parameters.
	pub sampling: Option<SamplingParams>,
	/// Existing session ID to reuse.
	pub session_id: Option<String>,
}

/// Options for a [`generate_stream`](AcpClient::generate_stream) call.
#[derive(Debug, Clone, Default)]
pub struct StreamOptions {
	/// Agent to target.
	pub agent_id: Option<String>,
	/// Server name.
	pub server_name: Option<String>,
	/// System prompt prepended as a text content block.
	pub system_prompt: Option<String>,
	/// Sampling parameters.
	pub sampling: Option<SamplingParams>,
	/// Existing session ID to reuse.
	pub session_id: Option<String>,
	/// Sliding-window stream timeout in milliseconds. Default: 120,000.
	pub stream_timeout_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Result of a [`generate`](AcpClient::generate) or [`chat`](AcpClient::chat) call.
#[derive(Debug, Clone)]
pub struct GenerateResult {
	/// Extracted text content from the response.
	pub content: String,
	/// The agent ID that handled the request.
	pub agent_id: String,
	/// The server name that handled the request.
	pub server_name: String,
	/// Session ID used for this request.
	pub session_id: String,
	/// Token usage statistics, if provided.
	pub usage: Option<TokenUsage>,
	/// Stop reason, if provided.
	pub stop_reason: Option<StopReason>,
}

/// Result of an [`embed`](AcpClient::embed) call.
#[derive(Debug, Clone)]
pub struct EmbedResult {
	/// Embedding vectors, one per input text.
	pub embeddings: Vec<Vec<f64>>,
	/// The agent ID that handled the request.
	pub agent_id: String,
	/// The server name that handled the request.
	pub server_name: String,
	/// Token usage statistics, if provided.
	pub usage: Option<TokenUsage>,
}

// ---------------------------------------------------------------------------
// AcpClient
// ---------------------------------------------------------------------------

/// The main ACP client — manages connections, resilience, and sessions.
pub struct AcpClient {
	/// One connection per configured server, wrapped in `Arc` so spawned
	/// tasks (e.g. streaming prompt requests) can share the reference.
	connections: HashMap<String, Arc<AcpConnection>>,
	/// Per-server circuit breakers for fault isolation.
	circuit_breakers: HashMap<String, CircuitBreaker>,
	/// Per-server health monitors with sliding-window stats.
	health_monitors: HashMap<String, HealthMonitor>,
	/// Session cache: session_id → SessionInfo (models/modes).
	session_cache: Mutex<HashMap<String, SessionInfo>>,
	/// The client configuration.
	config: AcpConfig,
	/// Default stream timeout in milliseconds (120s).
	stream_timeout_ms: u64,
}

/// Default stream timeout: 120 seconds.
const DEFAULT_STREAM_TIMEOUT_MS: u64 = 120_000;

impl AcpClient {
	// -------------------------------------------------------------------
	// Lifecycle
	// -------------------------------------------------------------------

	/// Create a new `AcpClient`, spawning and initializing all configured
	/// server connections.
	///
	/// At least one server must initialize successfully. Servers that fail
	/// to initialize are logged and skipped.
	pub async fn new(config: AcpConfig) -> Result<Self, AcpError> {
		let mut connections = HashMap::new();
		let mut circuit_breakers = HashMap::new();
		let mut health_monitors = HashMap::new();

		let mut last_error: Option<AcpError> = None;

		for entry in &config.servers {
			let conn_config = ConnectionConfig {
				command: entry.command.clone(),
				args: entry.args.clone(),
				cwd: entry.cwd.clone(),
				env: entry.env.clone(),
				timeout_ms: entry.timeout_ms.unwrap_or(60_000),
				init_timeout_ms: 30_000,
				client_name: "simse".into(),
				client_version: "1.0.0".into(),
			};

			match AcpConnection::new(conn_config).await {
				Ok(conn) => {
					match conn.initialize().await {
						Ok(result) => {
							tracing::info!(
								"ACP server \"{}\" initialized: {} v{}",
								entry.name,
								result.agent_info.name,
								result.agent_info.version,
							);

							// Set permission policy if configured.
							if let Some(ref policy) = entry.permission_policy {
								conn.set_permission_policy(policy.clone()).await;
							}

							connections.insert(entry.name.clone(), Arc::new(conn));
							circuit_breakers.insert(
								entry.name.clone(),
								CircuitBreaker::new(CircuitBreakerConfig::default()),
							);
							health_monitors.insert(
								entry.name.clone(),
								HealthMonitor::default(),
							);
						}
						Err(e) => {
							tracing::warn!(
								"ACP server \"{}\" failed to initialize: {e}",
								entry.name,
							);
							// Close the connection we just spawned.
							conn.close().await;
							last_error = Some(e);
						}
					}
				}
				Err(e) => {
					tracing::warn!(
						"ACP server \"{}\" failed to spawn: {e}",
						entry.name,
					);
					last_error = Some(e);
				}
			}
		}

		if connections.is_empty() {
			return Err(last_error.unwrap_or_else(|| {
				AcpError::ServerUnavailable("No ACP servers configured".into())
			}));
		}

		Ok(Self {
			connections,
			circuit_breakers,
			health_monitors,
			session_cache: Mutex::new(HashMap::new()),
			config,
			stream_timeout_ms: DEFAULT_STREAM_TIMEOUT_MS,
		})
	}

	/// Close all connections and release resources.
	pub async fn dispose(&self) -> Result<(), AcpError> {
		for (name, conn) in &self.connections {
			tracing::debug!("Closing ACP connection \"{name}\"");
			conn.close().await;
		}
		Ok(())
	}

	// -------------------------------------------------------------------
	// Generation
	// -------------------------------------------------------------------

	/// Generate a response from a text prompt.
	///
	/// Creates a new session (or reuses an existing one), sends the prompt,
	/// and returns the extracted text content.
	pub async fn generate(
		&self,
		prompt: &str,
		options: GenerateOptions,
	) -> Result<GenerateResult, AcpError> {
		let (server_name, conn) = self.resolve_connection(options.server_name.as_deref())?;
		let agent_id = self.resolve_agent_id(server_name, options.agent_id.as_deref());
		let server_name = server_name.to_string();

		let conn_ref = conn;
		let prompt_owned = prompt.to_string();
		let system_prompt = options.system_prompt.clone();
		let sampling = options.sampling.clone();
		let session_id_opt = options.session_id.clone();

		self.with_resilience(&server_name, || async {
			let session_id = self
				.ensure_session(&server_name, conn_ref, session_id_opt.as_deref())
				.await?;

			let content = build_text_content(&prompt_owned, system_prompt.as_deref());
			let metadata = build_sampling_metadata(sampling.as_ref());

			let result = conn_ref
				.request::<SessionPromptResult>(
					"session/prompt",
					serde_json::json!({
						"sessionId": session_id,
						"prompt": content,
						"metadata": metadata,
					}),
					0,
				)
				.await?;

			let text = extract_content_text(&result.content);
			let usage = extract_token_usage(&result.metadata);

			Ok(GenerateResult {
				content: text,
				agent_id: agent_id.clone(),
				server_name: server_name.clone(),
				session_id,
				usage,
				stop_reason: result.stop_reason,
			})
		})
		.await
	}

	/// Send a multi-turn chat conversation and get a response.
	///
	/// All messages are combined into content blocks for a single
	/// `session/prompt` call. System/assistant messages are prefixed.
	pub async fn chat(
		&self,
		messages: &[ChatMessage],
		options: ChatOptions,
	) -> Result<GenerateResult, AcpError> {
		if messages.is_empty() {
			return Err(AcpError::ProtocolError(
				"Cannot send empty message list".into(),
			));
		}

		let (server_name, conn) = self.resolve_connection(options.server_name.as_deref())?;
		let agent_id = self.resolve_agent_id(server_name, options.agent_id.as_deref());
		let server_name = server_name.to_string();

		let conn_ref = conn;
		let sampling = options.sampling.clone();
		let session_id_opt = options.session_id.clone();

		// Build content blocks from messages.
		let mut content: Vec<ContentBlock> = Vec::new();
		for msg in messages {
			let prefix = match msg.role.as_str() {
				"system" => "[System] ",
				"assistant" => "[Assistant] ",
				_ => "",
			};
			for block in &msg.content {
				match block {
					ContentBlock::Text { text } => {
						content.push(ContentBlock::Text {
							text: format!("{prefix}{text}"),
						});
					}
					other => content.push(other.clone()),
				}
			}
		}

		let content_owned = content;

		self.with_resilience(&server_name, || async {
			let session_id = self
				.ensure_session(&server_name, conn_ref, session_id_opt.as_deref())
				.await?;

			let metadata = build_sampling_metadata(sampling.as_ref());

			let result = conn_ref
				.request::<SessionPromptResult>(
					"session/prompt",
					serde_json::json!({
						"sessionId": session_id,
						"prompt": content_owned,
						"metadata": metadata,
					}),
					0,
				)
				.await?;

			let text = extract_content_text(&result.content);
			let usage = extract_token_usage(&result.metadata);

			Ok(GenerateResult {
				content: text,
				agent_id: agent_id.clone(),
				server_name: server_name.clone(),
				session_id,
				usage,
				stop_reason: result.stop_reason,
			})
		})
		.await
	}

	/// Start a streaming generation from a text prompt.
	///
	/// Returns an [`AcpStream`] that yields [`StreamChunk`] values as
	/// they arrive from the agent. The prompt is sent asynchronously and
	/// notifications feed into the stream via a channel.
	pub async fn generate_stream(
		&self,
		prompt: &str,
		options: StreamOptions,
	) -> Result<AcpStream, AcpError> {
		let (server_name, conn) = self.resolve_connection(options.server_name.as_deref())?;
		let _agent_id = self.resolve_agent_id(server_name, options.agent_id.as_deref());
		let server_name = server_name.to_string();

		let session_id = self
			.ensure_session(&server_name, conn, options.session_id.as_deref())
			.await?;

		let timeout_ms = options.stream_timeout_ms.unwrap_or(self.stream_timeout_ms);
		let permission_active = Arc::new(AtomicBool::new(false));
		let cancellation = CancellationToken::new();

		let (mut stream, tx) = create_stream(
			timeout_ms,
			Arc::clone(&permission_active),
			cancellation.clone(),
		);

		// Register notification handler for session/update.
		let tx_clone = tx.clone();
		let session_id_for_handler = session_id.clone();
		let subscription = conn.on_notification(
			"session/update",
			Box::new(move |params: serde_json::Value| {
				// Only process updates for our session.
				if let Some(sid) = params.get("sessionId").and_then(|v| v.as_str()) {
					if sid != session_id_for_handler {
						return;
					}
				}

				if let Some(chunk) = parse_session_update(&params) {
					let tx_inner = tx_clone.clone();
					// Use try_send to avoid blocking the notification handler.
					let _ = tx_inner.try_send(chunk);
				}
			}),
		);

		// Keep the subscription handle alive for the lifetime of the stream.
		// Without this, the handler is deactivated when generate_stream()
		// returns and no session/update notifications are routed to the stream.
		stream.keep_alive(Box::new(subscription));

		// Send the prompt asynchronously — the response completing will
		// trigger the Complete chunk.
		let content = build_text_content(prompt, options.system_prompt.as_deref());
		let metadata = build_sampling_metadata(options.sampling.as_ref());
		let prompt_params = serde_json::json!({
			"sessionId": session_id,
			"prompt": content,
			"metadata": metadata,
		});

		// Spawn the prompt request so chunks stream as notifications arrive.
		// Clone the Arc so the spawned task can own a reference to the
		// connection without requiring a 'static borrow.
		let conn_arc = Arc::clone(conn);
		let tx_for_complete = tx;

		tokio::spawn(async move {
			let result = conn_arc
				.request::<SessionPromptResult>("session/prompt", prompt_params, 0)
				.await;

			match result {
				Ok(prompt_result) => {
					let usage = extract_token_usage(&prompt_result.metadata);
					let _ = tx_for_complete
						.send(StreamChunk::Complete { usage })
						.await;
				}
				Err(_) => {
					let _ = tx_for_complete
						.send(StreamChunk::Complete { usage: None })
						.await;
				}
			}
		});

		Ok(stream)
	}

	/// Generate embeddings from input texts.
	///
	/// ACP does not define a native embedding protocol. This sends the
	/// texts as a JSON payload and attempts to parse embeddings from the
	/// response.
	pub async fn embed(
		&self,
		input: &[&str],
		model: Option<&str>,
		server: Option<&str>,
	) -> Result<EmbedResult, AcpError> {
		let (server_name, conn) = self.resolve_connection(server)?;
		let agent_id = model
			.map(|s| s.to_string())
			.unwrap_or_else(|| self.resolve_agent_id(server_name, None));
		let server_name = server_name.to_string();

		let conn_ref = conn;
		let input_owned: Vec<String> = input.iter().map(|s| s.to_string()).collect();

		self.with_resilience(&server_name, || async {
			let session_id = self
				.ensure_session(&server_name, conn_ref, None)
				.await?;

			let content = vec![ContentBlock::Text {
				text: serde_json::json!({
					"texts": input_owned,
					"action": "embed",
				})
				.to_string(),
			}];

			let result = conn_ref
				.request::<SessionPromptResult>(
					"session/prompt",
					serde_json::json!({
						"sessionId": session_id,
						"prompt": content,
					}),
					0,
				)
				.await?;

			// Try to extract embeddings from response content blocks.
			if let Some(ref blocks) = result.content {
				for block in blocks {
					if let ContentBlock::Text { text } = block {
						if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
							if let Some(arr) = parsed.as_array() {
								let embeddings: Vec<Vec<f64>> = arr
									.iter()
									.filter_map(|v| {
										v.as_array().map(|inner| {
											inner
												.iter()
												.filter_map(|n| n.as_f64())
												.collect()
										})
									})
									.collect();
								if !embeddings.is_empty() {
									return Ok(EmbedResult {
										embeddings,
										agent_id: agent_id.clone(),
										server_name: server_name.clone(),
										usage: extract_token_usage(&result.metadata),
									});
								}
							}
							if let Some(emb_val) = parsed.get("embeddings") {
								if let Some(arr) = emb_val.as_array() {
									let embeddings: Vec<Vec<f64>> = arr
										.iter()
										.filter_map(|v| {
											v.as_array().map(|inner| {
												inner
													.iter()
													.filter_map(|n| n.as_f64())
													.collect()
											})
										})
										.collect();
									return Ok(EmbedResult {
										embeddings,
										agent_id: agent_id.clone(),
										server_name: server_name.clone(),
										usage: extract_token_usage(&result.metadata),
									});
								}
							}
						}
					}
				}
			}

			Err(AcpError::ProtocolError(
				"ACP server returned no embeddings in response".into(),
			))
		})
		.await
	}

	// -------------------------------------------------------------------
	// Agent discovery
	// -------------------------------------------------------------------

	/// List available agents. ACP has no native agent listing — returns
	/// synthetic info from the server configuration.
	pub async fn list_agents(
		&self,
		server: Option<&str>,
	) -> Result<Vec<AgentInfoEntry>, AcpError> {
		if let Some(name) = server {
			let entry = self
				.config
				.servers
				.iter()
				.find(|s| s.name == name)
				.ok_or_else(|| {
					AcpError::ServerUnavailable(format!("Server \"{name}\" not configured"))
				})?;

			Ok(vec![AgentInfoEntry {
				id: entry
					.default_agent
					.clone()
					.unwrap_or_else(|| entry.name.clone()),
				name: Some(entry.name.clone()),
				description: Some(format!("ACP agent on server \"{}\"", entry.name)),
				metadata: None,
			}])
		} else {
			Ok(self
				.config
				.servers
				.iter()
				.map(|entry| AgentInfoEntry {
					id: entry
						.default_agent
						.clone()
						.unwrap_or_else(|| entry.name.clone()),
					name: Some(entry.name.clone()),
					description: Some(format!("ACP agent on server \"{}\"", entry.name)),
					metadata: None,
				})
				.collect())
		}
	}

	// -------------------------------------------------------------------
	// Session management
	// -------------------------------------------------------------------

	/// List sessions on a server.
	pub async fn list_sessions(
		&self,
		server: Option<&str>,
	) -> Result<Vec<SessionListEntry>, AcpError> {
		let (_name, conn) = self.resolve_connection(server)?;
		let result = conn
			.request::<serde_json::Value>("session/list", serde_json::json!({}), 0)
			.await?;

		let sessions: Vec<SessionListEntry> = result
			.get("sessions")
			.and_then(|v| serde_json::from_value(v.clone()).ok())
			.unwrap_or_default();

		Ok(sessions)
	}

	/// Load (resume) an existing session.
	pub async fn load_session(
		&self,
		session_id: &str,
		server: Option<&str>,
	) -> Result<SessionInfo, AcpError> {
		let (_name, conn) = self.resolve_connection(server)?;
		let info = conn
			.request::<SessionInfo>(
				"session/load",
				serde_json::json!({ "sessionId": session_id }),
				0,
			)
			.await?;

		// Update cache.
		self.session_cache
			.lock()
			.await
			.insert(session_id.to_string(), info.clone());

		Ok(info)
	}

	/// Delete a session.
	pub async fn delete_session(
		&self,
		session_id: &str,
		server: Option<&str>,
	) -> Result<(), AcpError> {
		let (_name, conn) = self.resolve_connection(server)?;
		conn.request::<serde_json::Value>(
			"session/delete",
			serde_json::json!({ "sessionId": session_id }),
			0,
		)
		.await?;

		// Remove from cache.
		self.session_cache
			.lock()
			.await
			.remove(session_id);

		Ok(())
	}

	/// Set the mode for a session (best-effort).
	pub async fn set_session_mode(
		&self,
		session_id: &str,
		mode_id: &str,
		server: Option<&str>,
	) -> Result<(), AcpError> {
		let (_name, conn) = self.resolve_connection(server)?;
		let params = SetConfigOptionParams {
			session_id: session_id.to_string(),
			config_option_id: "mode".to_string(),
			group_id: mode_id.to_string(),
		};
		// Best-effort: agent may not support config options.
		let _ = conn
			.request::<serde_json::Value>(
				"session/set_config_option",
				serde_json::to_value(&params)
					.map_err(|e| AcpError::Serialization(e.to_string()))?,
				0,
			)
			.await;
		Ok(())
	}

	/// Set the model for a session (best-effort).
	pub async fn set_session_model(
		&self,
		session_id: &str,
		model_id: &str,
		server: Option<&str>,
	) -> Result<(), AcpError> {
		let (_name, conn) = self.resolve_connection(server)?;
		let params = SetConfigOptionParams {
			session_id: session_id.to_string(),
			config_option_id: "model".to_string(),
			group_id: model_id.to_string(),
		};
		// Best-effort.
		let _ = conn
			.request::<serde_json::Value>(
				"session/set_config_option",
				serde_json::to_value(&params)
					.map_err(|e| AcpError::Serialization(e.to_string()))?,
				0,
			)
			.await;
		Ok(())
	}

	// -------------------------------------------------------------------
	// Health & availability
	// -------------------------------------------------------------------

	/// Check if a server is available (circuit breaker is not open and
	/// the connection is healthy).
	pub fn is_available(&self, server: Option<&str>) -> bool {
		let name = match self.resolve_server_name(server) {
			Some(n) => n,
			None => return false,
		};

		// Check circuit breaker.
		if let Some(cb) = self.circuit_breakers.get(name) {
			if !cb.allow_request() {
				return false;
			}
		}

		// Check connection health.
		if let Some(conn) = self.connections.get(name) {
			conn.is_healthy()
		} else {
			false
		}
	}

	/// Return the names of all configured servers.
	pub fn server_names(&self) -> Vec<String> {
		self.config.servers.iter().map(|s| s.name.clone()).collect()
	}

	/// Return the default server name (if any).
	pub fn default_server_name(&self) -> Option<&str> {
		self.config
			.default_server
			.as_deref()
			.or_else(|| self.config.servers.first().map(|s| s.name.as_str()))
	}

	/// Return the default agent ID (if configured).
	pub fn default_agent(&self) -> Option<&str> {
		self.config.default_agent.as_deref()
	}

	// -------------------------------------------------------------------
	// Policy
	// -------------------------------------------------------------------

	/// Set the permission policy on all active connections.
	pub async fn set_permission_policy(&self, policy: PermissionPolicy) {
		for conn in self.connections.values() {
			conn.set_permission_policy(policy.clone()).await;
		}
	}

	// -------------------------------------------------------------------
	// Internal: connection resolution
	// -------------------------------------------------------------------

	/// Resolve which server name to use.
	fn resolve_server_name<'a>(&'a self, server_name: Option<&'a str>) -> Option<&'a str> {
		server_name.or_else(|| {
			self.config
				.default_server
				.as_deref()
				.or_else(|| self.config.servers.first().map(|s| s.name.as_str()))
		})
	}

	/// Resolve a server name to its connection. Returns the canonical
	/// name and a reference to the `Arc<AcpConnection>`.
	fn resolve_connection<'a>(
		&'a self,
		server_name: Option<&'a str>,
	) -> Result<(&'a str, &'a Arc<AcpConnection>), AcpError> {
		let name = self.resolve_server_name(server_name).ok_or_else(|| {
			AcpError::ServerUnavailable("No ACP servers configured".into())
		})?;

		let conn = self.connections.get(name).ok_or_else(|| {
			AcpError::ServerUnavailable(format!(
				"ACP server \"{name}\" is not connected"
			))
		})?;

		Ok((name, conn))
	}

	/// Resolve the agent ID using the resolution chain:
	/// option → server default → global default → server name.
	fn resolve_agent_id(&self, server_name: &str, step_agent_id: Option<&str>) -> String {
		if let Some(id) = step_agent_id {
			return id.to_string();
		}

		let server_default = self
			.config
			.servers
			.iter()
			.find(|s| s.name == server_name)
			.and_then(|s| s.default_agent.as_deref());

		if let Some(id) = server_default {
			return id.to_string();
		}

		if let Some(ref id) = self.config.default_agent {
			return id.clone();
		}

		server_name.to_string()
	}

	// -------------------------------------------------------------------
	// Internal: session management
	// -------------------------------------------------------------------

	/// Ensure a session exists — return a cached session ID or create a
	/// new one.
	async fn ensure_session(
		&self,
		_server_name: &str,
		conn: &AcpConnection,
		session_id: Option<&str>,
	) -> Result<String, AcpError> {
		// If a session ID is provided and cached, reuse it.
		if let Some(sid) = session_id {
			let cache = self.session_cache.lock().await;
			if cache.contains_key(sid) {
				return Ok(sid.to_string());
			}
			drop(cache);
			// Even if not cached, trust the caller's session ID.
			return Ok(sid.to_string());
		}

		// Create a new session.
		let mcp_servers: Vec<serde_json::Value> = self
			.config
			.mcp_servers
			.iter()
			.map(|s| {
				serde_json::json!({
					"name": s.name,
					"config": s.config,
				})
			})
			.collect();

		let result = conn
			.request::<SessionInfo>(
				"session/new",
				serde_json::json!({
					"cwd": std::env::current_dir()
						.map(|p| p.to_string_lossy().into_owned())
						.unwrap_or_else(|_| ".".into()),
					"mcpServers": mcp_servers,
				}),
				0,
			)
			.await?;

		let session_id = result.session_id.clone();

		// Cache the session info.
		self.session_cache
			.lock()
			.await
			.insert(session_id.clone(), result);

		// Set permission mode based on server policy (best-effort).
		let policy = conn.permission_policy().await;
		let mode_id = match policy {
			PermissionPolicy::AutoApprove => "bypassPermissions",
			PermissionPolicy::Deny => "plan",
			PermissionPolicy::Prompt => "default",
		};

		let config_params = serde_json::json!({
			"sessionId": session_id,
			"configOptionId": "mode",
			"groupId": mode_id,
		});
		// Fire-and-forget.
		let _ = conn
			.request::<serde_json::Value>("session/set_config_option", config_params, 0)
			.await;

		Ok(session_id)
	}

	// -------------------------------------------------------------------
	// Internal: resilience wrapper
	// -------------------------------------------------------------------

	/// Execute an operation with circuit breaker + retry.
	async fn with_resilience<T, F, Fut>(&self, server_name: &str, f: F) -> Result<T, AcpError>
	where
		F: Fn() -> Fut,
		Fut: std::future::Future<Output = Result<T, AcpError>>,
	{
		// Check circuit breaker.
		if let Some(cb) = self.circuit_breakers.get(server_name) {
			if !cb.allow_request() {
				return Err(AcpError::CircuitBreakerOpen(format!(
					"Circuit breaker open for server \"{server_name}\""
				)));
			}
		}

		let config = RetryConfig::default();

		let result = retry(&config, &f).await;

		match &result {
			Ok(_) => {
				if let Some(cb) = self.circuit_breakers.get(server_name) {
					cb.record_success();
				}
				if let Some(hm) = self.health_monitors.get(server_name) {
					hm.record_success();
				}
			}
			Err(e) => {
				if let Some(cb) = self.circuit_breakers.get(server_name) {
					cb.record_failure();
				}
				if let Some(hm) = self.health_monitors.get(server_name) {
					hm.record_failure(&e.to_string());
				}
			}
		}

		result
	}
}

// ---------------------------------------------------------------------------
// Free functions — content helpers
// ---------------------------------------------------------------------------

/// Build text content blocks from a prompt and optional system prompt.
fn build_text_content(prompt: &str, system_prompt: Option<&str>) -> Vec<ContentBlock> {
	let mut blocks = Vec::new();
	if let Some(sys) = system_prompt {
		blocks.push(ContentBlock::Text {
			text: sys.to_string(),
		});
	}
	blocks.push(ContentBlock::Text {
		text: prompt.to_string(),
	});
	blocks
}

/// Build sampling metadata from sampling parameters.
fn build_sampling_metadata(sampling: Option<&SamplingParams>) -> Option<serde_json::Value> {
	let sampling = sampling?;
	let mut meta = serde_json::Map::new();

	if let Some(temp) = sampling.temperature {
		meta.insert("temperature".into(), serde_json::json!(temp));
	}
	if let Some(max) = sampling.max_tokens {
		meta.insert("max_tokens".into(), serde_json::json!(max));
	}
	if let Some(tp) = sampling.top_p {
		meta.insert("top_p".into(), serde_json::json!(tp));
	}
	if let Some(tk) = sampling.top_k {
		meta.insert("top_k".into(), serde_json::json!(tk));
	}
	if let Some(ref seqs) = sampling.stop_sequences {
		meta.insert("stop_sequences".into(), serde_json::json!(seqs));
	}

	if meta.is_empty() {
		None
	} else {
		Some(serde_json::Value::Object(meta))
	}
}

/// Extract text content from response content blocks.
fn extract_content_text(content: &Option<Vec<ContentBlock>>) -> String {
	let blocks = match content {
		Some(ref b) => b,
		None => return String::new(),
	};

	let mut text = String::new();
	for block in blocks {
		if let ContentBlock::Text { text: t } = block {
			text.push_str(t);
		}
	}
	text
}

/// Extract token usage from response metadata.
fn extract_token_usage(metadata: &Option<serde_json::Value>) -> Option<TokenUsage> {
	let meta = metadata.as_ref()?;

	// Try direct "usage" field.
	if let Some(usage) = meta.get("usage") {
		if let Ok(u) = serde_json::from_value::<TokenUsage>(usage.clone()) {
			return Some(u);
		}
	}

	// Try top-level fields.
	let prompt = meta
		.get("promptTokens")
		.or_else(|| meta.get("prompt_tokens"))
		.and_then(|v| v.as_u64())
		.unwrap_or(0);
	let completion = meta
		.get("completionTokens")
		.or_else(|| meta.get("completion_tokens"))
		.and_then(|v| v.as_u64())
		.unwrap_or(0);
	let total = meta
		.get("totalTokens")
		.or_else(|| meta.get("total_tokens"))
		.and_then(|v| v.as_u64())
		.unwrap_or(0);

	if prompt > 0 || completion > 0 || total > 0 {
		Some(TokenUsage {
			prompt_tokens: prompt,
			completion_tokens: completion,
			total_tokens: total,
		})
	} else {
		None
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -------------------------------------------------------------------
	// Configuration and resolution tests (no subprocess needed)
	// -------------------------------------------------------------------

	fn make_config(servers: Vec<(&str, Option<&str>)>) -> AcpConfig {
		AcpConfig {
			servers: servers
				.into_iter()
				.map(|(name, default_agent)| ServerEntry {
					name: name.to_string(),
					command: "echo".to_string(),
					args: vec![],
					cwd: None,
					env: HashMap::new(),
					default_agent: default_agent.map(|s| s.to_string()),
					timeout_ms: None,
					permission_policy: None,
				})
				.collect(),
			default_server: None,
			default_agent: None,
			mcp_servers: vec![],
		}
	}

	// -------------------------------------------------------------------
	// GenerateOptions / StreamOptions / ChatOptions defaults
	// -------------------------------------------------------------------

	#[test]
	fn generate_options_default() {
		let opts = GenerateOptions::default();
		assert!(opts.agent_id.is_none());
		assert!(opts.server_name.is_none());
		assert!(opts.system_prompt.is_none());
		assert!(opts.sampling.is_none());
		assert!(opts.session_id.is_none());
	}

	#[test]
	fn stream_options_default() {
		let opts = StreamOptions::default();
		assert!(opts.agent_id.is_none());
		assert!(opts.server_name.is_none());
		assert!(opts.system_prompt.is_none());
		assert!(opts.sampling.is_none());
		assert!(opts.session_id.is_none());
		assert!(opts.stream_timeout_ms.is_none());
	}

	#[test]
	fn chat_options_default() {
		let opts = ChatOptions::default();
		assert!(opts.agent_id.is_none());
		assert!(opts.server_name.is_none());
		assert!(opts.sampling.is_none());
		assert!(opts.session_id.is_none());
	}

	// -------------------------------------------------------------------
	// server_names and default accessors
	// -------------------------------------------------------------------

	#[test]
	fn server_names_returns_configured_names() {
		let config = make_config(vec![("alpha", None), ("beta", None)]);
		// We can't call AcpClient::new without real processes, so test
		// the config-based accessors directly by constructing manually.
		let names: Vec<String> = config.servers.iter().map(|s| s.name.clone()).collect();
		assert_eq!(names, vec!["alpha", "beta"]);
	}

	#[test]
	fn default_server_name_uses_config() {
		let mut config = make_config(vec![("alpha", None), ("beta", None)]);
		config.default_server = Some("beta".to_string());

		let default = config
			.default_server
			.as_deref()
			.or_else(|| config.servers.first().map(|s| s.name.as_str()));
		assert_eq!(default, Some("beta"));
	}

	#[test]
	fn default_server_name_falls_back_to_first() {
		let config = make_config(vec![("alpha", None), ("beta", None)]);

		let default = config
			.default_server
			.as_deref()
			.or_else(|| config.servers.first().map(|s| s.name.as_str()));
		assert_eq!(default, Some("alpha"));
	}

	// -------------------------------------------------------------------
	// Agent ID resolution
	// -------------------------------------------------------------------

	#[test]
	fn resolve_agent_id_uses_step_agent() {
		let config = make_config(vec![("server1", Some("default-agent"))]);
		// step_agent_id takes priority.
		let resolved = resolve_agent_id_helper(&config, "server1", Some("step-agent"));
		assert_eq!(resolved, "step-agent");
	}

	#[test]
	fn resolve_agent_id_uses_server_default() {
		let config = make_config(vec![("server1", Some("server-default"))]);
		let resolved = resolve_agent_id_helper(&config, "server1", None);
		assert_eq!(resolved, "server-default");
	}

	#[test]
	fn resolve_agent_id_uses_global_default() {
		let mut config = make_config(vec![("server1", None)]);
		config.default_agent = Some("global-agent".to_string());
		let resolved = resolve_agent_id_helper(&config, "server1", None);
		assert_eq!(resolved, "global-agent");
	}

	#[test]
	fn resolve_agent_id_falls_back_to_server_name() {
		let config = make_config(vec![("server1", None)]);
		let resolved = resolve_agent_id_helper(&config, "server1", None);
		assert_eq!(resolved, "server1");
	}

	/// Helper that replicates the AcpClient::resolve_agent_id logic.
	fn resolve_agent_id_helper(
		config: &AcpConfig,
		server_name: &str,
		step_agent_id: Option<&str>,
	) -> String {
		if let Some(id) = step_agent_id {
			return id.to_string();
		}
		let server_default = config
			.servers
			.iter()
			.find(|s| s.name == server_name)
			.and_then(|s| s.default_agent.as_deref());
		if let Some(id) = server_default {
			return id.to_string();
		}
		if let Some(ref id) = config.default_agent {
			return id.clone();
		}
		server_name.to_string()
	}

	// -------------------------------------------------------------------
	// Content helpers
	// -------------------------------------------------------------------

	#[test]
	fn build_text_content_without_system() {
		let blocks = build_text_content("hello", None);
		assert_eq!(blocks.len(), 1);
		match &blocks[0] {
			ContentBlock::Text { text } => assert_eq!(text, "hello"),
			_ => panic!("expected Text block"),
		}
	}

	#[test]
	fn build_text_content_with_system() {
		let blocks = build_text_content("hello", Some("system instructions"));
		assert_eq!(blocks.len(), 2);
		match &blocks[0] {
			ContentBlock::Text { text } => assert_eq!(text, "system instructions"),
			_ => panic!("expected Text block"),
		}
		match &blocks[1] {
			ContentBlock::Text { text } => assert_eq!(text, "hello"),
			_ => panic!("expected Text block"),
		}
	}

	#[test]
	fn build_sampling_metadata_none_when_no_params() {
		assert!(build_sampling_metadata(None).is_none());
	}

	#[test]
	fn build_sampling_metadata_none_when_empty_params() {
		let params = SamplingParams::default();
		assert!(build_sampling_metadata(Some(&params)).is_none());
	}

	#[test]
	fn build_sampling_metadata_includes_set_fields() {
		let params = SamplingParams {
			temperature: Some(0.7),
			max_tokens: Some(1024),
			top_p: None,
			top_k: None,
			stop_sequences: None,
		};
		let meta = build_sampling_metadata(Some(&params)).unwrap();
		assert_eq!(meta["temperature"], 0.7);
		assert_eq!(meta["max_tokens"], 1024);
		assert!(meta.get("top_p").is_none());
	}

	// -------------------------------------------------------------------
	// extract_content_text
	// -------------------------------------------------------------------

	#[test]
	fn extract_content_text_empty_when_none() {
		assert_eq!(extract_content_text(&None), "");
	}

	#[test]
	fn extract_content_text_concatenates_text_blocks() {
		let content = Some(vec![
			ContentBlock::Text {
				text: "hello ".into(),
			},
			ContentBlock::Text {
				text: "world".into(),
			},
		]);
		assert_eq!(extract_content_text(&content), "hello world");
	}

	#[test]
	fn extract_content_text_skips_non_text_blocks() {
		let content = Some(vec![
			ContentBlock::Text {
				text: "text".into(),
			},
			ContentBlock::Resource {
				resource: crate::protocol::ResourceData {
					uri: "file:///a.txt".into(),
					mime_type: None,
					text: None,
					blob: None,
				},
			},
			ContentBlock::Text {
				text: " more".into(),
			},
		]);
		assert_eq!(extract_content_text(&content), "text more");
	}

	// -------------------------------------------------------------------
	// extract_token_usage
	// -------------------------------------------------------------------

	#[test]
	fn extract_token_usage_none_when_no_metadata() {
		assert!(extract_token_usage(&None).is_none());
	}

	#[test]
	fn extract_token_usage_from_usage_field() {
		let meta = Some(serde_json::json!({
			"usage": {
				"promptTokens": 10,
				"completionTokens": 20,
				"totalTokens": 30,
			}
		}));
		let usage = extract_token_usage(&meta).unwrap();
		assert_eq!(usage.prompt_tokens, 10);
		assert_eq!(usage.completion_tokens, 20);
		assert_eq!(usage.total_tokens, 30);
	}

	#[test]
	fn extract_token_usage_from_top_level_fields() {
		let meta = Some(serde_json::json!({
			"promptTokens": 5,
			"completionTokens": 15,
			"totalTokens": 20,
		}));
		let usage = extract_token_usage(&meta).unwrap();
		assert_eq!(usage.prompt_tokens, 5);
		assert_eq!(usage.completion_tokens, 15);
		assert_eq!(usage.total_tokens, 20);
	}

	#[test]
	fn extract_token_usage_none_when_all_zeros() {
		let meta = Some(serde_json::json!({ "unrelated": true }));
		assert!(extract_token_usage(&meta).is_none());
	}

	// -------------------------------------------------------------------
	// AcpConfig construction
	// -------------------------------------------------------------------

	#[test]
	fn acp_config_with_mcp_servers() {
		let config = AcpConfig {
			servers: vec![],
			default_server: None,
			default_agent: None,
			mcp_servers: vec![McpServerEntry {
				name: "mcp1".to_string(),
				config: serde_json::json!({"url": "http://localhost:3000"}),
			}],
		};
		assert_eq!(config.mcp_servers.len(), 1);
		assert_eq!(config.mcp_servers[0].name, "mcp1");
	}
}
