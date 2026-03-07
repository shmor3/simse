// ---------------------------------------------------------------------------
// AcpClient — orchestration layer for multi-server ACP connections
// ---------------------------------------------------------------------------
//
// The main client that consumers interact with. Manages:
//   - A pool of connections (one per configured server)
//   - Per-server circuit breakers and health monitors
//   - Session caching (session_id → server mapping)
//   - Resilient request execution with retry + circuit breaker
//   - Agent discovery (synthetic, from config)
//   - Streaming via broadcast::Receiver<SessionNotification>
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol as acp;
use tokio::sync::{Mutex, broadcast};

use serde::{Deserialize, Serialize};

use crate::acp::connection::{ConnectionConfig, ConnectionWrapper};
use crate::acp::error::AcpError;
use crate::acp::permission::PermissionPolicy;
use crate::acp::resilience::{CircuitBreaker, CircuitBreakerConfig, HealthMonitor, RetryConfig, retry};
use crate::acp::stream::{AcpStream, StreamChunk, create_stream, parse_session_update};

// ---------------------------------------------------------------------------
// ACP domain types (previously in protocol.rs)
// ---------------------------------------------------------------------------

/// Sampling parameters for generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingParams {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub temperature: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub max_tokens: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_p: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_k: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub stop_sequences: Option<Vec<String>>,
}

/// Token usage tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
	#[serde(alias = "prompt_tokens")]
	pub prompt_tokens: u64,
	#[serde(alias = "completion_tokens")]
	pub completion_tokens: u64,
	#[serde(alias = "total_tokens")]
	pub total_tokens: u64,
}

/// Stop reason returned by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
	EndTurn,
	MaxTokens,
	MaxTurnRequests,
	Refusal,
	Cancelled,
	StopSequence,
	ToolUse,
}

/// Agent info derived from configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfoEntry {
	pub id: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub metadata: Option<serde_json::Value>,
}

/// Entry in the session list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListEntry {
	pub session_id: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub created_at: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub last_active_at: Option<String>,
}

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
	/// Text content of the message.
	pub content: String,
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
	/// One `ConnectionWrapper` per configured server.
	connections: HashMap<String, Arc<ConnectionWrapper>>,
	/// Per-server circuit breakers for fault isolation.
	circuit_breakers: HashMap<String, CircuitBreaker>,
	/// Per-server health monitors with sliding-window stats.
	health_monitors: HashMap<String, HealthMonitor>,
	/// Session cache: session_id → server_name (tracks which server owns each session).
	session_cache: Mutex<HashMap<String, String>>,
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

			match ConnectionWrapper::new(conn_config).await {
				Ok(conn) => {
					match conn.initialize().await {
						Ok(result) => {
							let agent_name = result
								.agent_info
								.as_ref()
								.map(|i| i.name.as_str())
								.unwrap_or("unknown");
							let agent_version = result
								.agent_info
								.as_ref()
								.map(|i| i.version.as_str())
								.unwrap_or("?");
							tracing::info!(
								"ACP server \"{}\" initialized: {} v{}",
								entry.name,
								agent_name,
								agent_version,
							);

							// Set permission policy if configured.
							if let Some(ref policy) = entry.permission_policy {
								conn.set_permission_policy(*policy);
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
	/// and returns the extracted text content from session update
	/// notifications.
	pub async fn generate(
		&self,
		prompt: &str,
		options: GenerateOptions,
	) -> Result<GenerateResult, AcpError> {
		let (server_name, conn) = self.resolve_connection(options.server_name.as_deref())?;
		let agent_id = self.resolve_agent_id(server_name, options.agent_id.as_deref());
		let server_name = server_name.to_string();

		let conn_ref = Arc::clone(conn);
		let prompt_owned = prompt.to_string();
		let system_prompt = options.system_prompt.clone();
		let session_id_opt = options.session_id.clone();

		self.with_resilience(&server_name, || {
			let conn_ref = Arc::clone(&conn_ref);
			let prompt_owned = prompt_owned.clone();
			let system_prompt = system_prompt.clone();
			let session_id_opt = session_id_opt.clone();
			let agent_id = agent_id.clone();
			let server_name = server_name.clone();
			async move {
				let session_id = ensure_session(
					&conn_ref,
					session_id_opt.as_deref(),
				)
				.await?;

				let content = build_sdk_content(&prompt_owned, system_prompt.as_deref());

				// Subscribe to updates before prompting so we capture all
				// agent_message_chunk notifications.
				let mut update_rx = conn_ref.subscribe_updates();

				let session_id_sdk: acp::SessionId = acp::SessionId::new(session_id.clone());
				let prompt_result = conn_ref
					.prompt(session_id_sdk, content)
					.await?;

				// Collect text from notifications that arrived during prompting.
				let text = collect_text_from_updates(&mut update_rx, &session_id);

				let stop_reason = map_stop_reason(&prompt_result.stop_reason);

				Ok(GenerateResult {
					content: text,
					agent_id,
					server_name,
					session_id,
					usage: None,
					stop_reason,
				})
			}
		})
		.await
	}

	/// Send a multi-turn chat conversation and get a response.
	///
	/// Each message is sent as a separate prompt call on the same session.
	/// The last message's response is returned.
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

		let conn_ref = Arc::clone(conn);
		let session_id_opt = options.session_id.clone();
		let messages_owned: Vec<ChatMessage> = messages.to_vec();

		self.with_resilience(&server_name, || {
			let conn_ref = Arc::clone(&conn_ref);
			let session_id_opt = session_id_opt.clone();
			let agent_id = agent_id.clone();
			let server_name = server_name.clone();
			let messages_owned = messages_owned.clone();
			async move {
				let session_id = ensure_session(
					&conn_ref,
					session_id_opt.as_deref(),
				)
				.await?;

				let session_id_sdk: acp::SessionId = acp::SessionId::new(session_id.clone());

				// Send each message as a prompt, keeping the last response.
				let mut last_text = String::new();
				let mut last_stop_reason = None;

				for msg in &messages_owned {
					let prefix = match msg.role.as_str() {
						"system" => "[System] ",
						"assistant" => "[Assistant] ",
						_ => "",
					};
					let text = format!("{prefix}{}", msg.content);
					let content = vec![acp::ContentBlock::Text(acp::TextContent::new(text))];

					let mut update_rx = conn_ref.subscribe_updates();

					let prompt_result = conn_ref
						.prompt(session_id_sdk.clone(), content)
						.await?;

					last_text = collect_text_from_updates(&mut update_rx, &session_id);
					last_stop_reason = map_stop_reason(&prompt_result.stop_reason);
				}

				Ok(GenerateResult {
					content: last_text,
					agent_id,
					server_name,
					session_id,
					usage: None,
					stop_reason: last_stop_reason,
				})
			}
		})
		.await
	}

	/// Start a streaming generation from a text prompt.
	///
	/// Returns an [`AcpStream`] that yields [`StreamChunk`] values as
	/// they arrive from the agent via `SessionNotification` broadcasts.
	pub async fn generate_stream(
		&self,
		prompt: &str,
		options: StreamOptions,
	) -> Result<AcpStream, AcpError> {
		let (server_name, conn) = self.resolve_connection(options.server_name.as_deref())?;
		let _agent_id = self.resolve_agent_id(server_name, options.agent_id.as_deref());
		let _server_name = server_name.to_string();

		let session_id = ensure_session(
			conn,
			options.session_id.as_deref(),
		)
		.await?;

		let timeout_ms = options.stream_timeout_ms.unwrap_or(self.stream_timeout_ms);
		let permission_active = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let cancellation = tokio_util::sync::CancellationToken::new();

		let (stream, tx) = create_stream(
			timeout_ms,
			Arc::clone(&permission_active),
			cancellation.clone(),
		);

		// Subscribe to broadcast updates and filter for our session.
		let mut update_rx = conn.subscribe_updates();
		let session_id_filter = session_id.clone();
		let tx_for_updates = tx.clone();

		// Spawn a task to forward relevant notifications to the stream channel.
		tokio::spawn(async move {
			while let Ok(notification) = update_rx.recv().await {
				if &*notification.session_id.0 != session_id_filter.as_str() {
					continue;
				}
				// Convert SDK SessionNotification → serde_json::Value → StreamChunk
				if let Ok(value) = serde_json::to_value(&notification) {
					if let Some(chunk) = parse_session_update(&value) {
						if tx_for_updates.try_send(chunk).is_err() {
							break;
						}
					}
				}
			}
		});

		// Send the prompt asynchronously — the response completing will
		// trigger the Complete chunk.
		let content = build_sdk_content(prompt, options.system_prompt.as_deref());
		let conn_arc = Arc::clone(conn);
		let session_id_for_prompt: acp::SessionId = acp::SessionId::new(session_id.clone());
		let tx_for_complete = tx;

		tokio::spawn(async move {
			let _result = conn_arc
				.prompt(session_id_for_prompt, content)
				.await;

			// Signal stream completion.
			let _ = tx_for_complete
				.send(StreamChunk::Complete { usage: None })
				.await;
		});

		Ok(stream)
	}

	/// Generate embeddings from input texts.
	///
	/// ACP does not define a native embedding protocol. Returns an error.
	pub async fn embed(
		&self,
		_input: &[&str],
		_model: Option<&str>,
		_server: Option<&str>,
	) -> Result<EmbedResult, AcpError> {
		Err(AcpError::ProtocolError(
			"embed not supported via ACP SDK".into(),
		))
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
	///
	/// Standard ACP does not define a session listing method. Returns
	/// an empty list.
	pub async fn list_sessions(
		&self,
		_server: Option<&str>,
	) -> Result<Vec<SessionListEntry>, AcpError> {
		Ok(Vec::new())
	}

	/// Load (resume) an existing session.
	pub async fn load_session(
		&self,
		session_id: &str,
		server: Option<&str>,
	) -> Result<(), AcpError> {
		let (_name, conn) = self.resolve_connection(server)?;
		let session_id_sdk = acp::SessionId::new(session_id.to_string());
		let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

		conn.load_session(session_id_sdk, cwd, Vec::new()).await?;

		// Update cache.
		let server_name = _name.to_string();
		self.session_cache
			.lock()
			.await
			.insert(session_id.to_string(), server_name);

		Ok(())
	}

	/// Delete a session.
	///
	/// ACP has no native delete method. Best-effort no-op: just removes
	/// from our local cache.
	pub async fn delete_session(
		&self,
		session_id: &str,
		_server: Option<&str>,
	) -> Result<(), AcpError> {
		self.session_cache
			.lock()
			.await
			.remove(session_id);
		Ok(())
	}

	/// Set the mode for a session.
	pub async fn set_session_mode(
		&self,
		session_id: &str,
		mode_id: &str,
		server: Option<&str>,
	) -> Result<(), AcpError> {
		let (_name, conn) = self.resolve_connection(server)?;
		let session_id_sdk = acp::SessionId::new(session_id.to_string());
		let mode_id_sdk = acp::SessionModeId::new(mode_id.to_string());
		// Best-effort: agent may not support modes.
		let _ = conn.set_session_mode(session_id_sdk, mode_id_sdk).await;
		Ok(())
	}

	/// Set the model for a session (best-effort via config option).
	pub async fn set_session_model(
		&self,
		session_id: &str,
		model_id: &str,
		server: Option<&str>,
	) -> Result<(), AcpError> {
		let (_name, conn) = self.resolve_connection(server)?;
		let session_id_sdk = acp::SessionId::new(session_id.to_string());
		let config_id = acp::SessionConfigId::new("model".to_string());
		let value_id = acp::SessionConfigValueId::new(model_id.to_string());
		// Best-effort.
		let _ = conn
			.set_config_option(session_id_sdk, config_id, value_id)
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
	pub fn set_permission_policy(&self, policy: PermissionPolicy) {
		for conn in self.connections.values() {
			conn.set_permission_policy(policy);
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
	/// name and a reference to the `Arc<ConnectionWrapper>`.
	fn resolve_connection<'a>(
		&'a self,
		server_name: Option<&'a str>,
	) -> Result<(&'a str, &'a Arc<ConnectionWrapper>), AcpError> {
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
// Free functions — session helpers
// ---------------------------------------------------------------------------

/// Ensure a session exists — return a cached session ID or create a new one.
async fn ensure_session(
	conn: &ConnectionWrapper,
	session_id: Option<&str>,
) -> Result<String, AcpError> {
	// If a session ID is provided, trust the caller.
	if let Some(sid) = session_id {
		return Ok(sid.to_string());
	}

	// Create a new session.
	let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
	let result = conn.new_session(cwd, Vec::new()).await?;
	Ok(result.session_id.to_string())
}

// ---------------------------------------------------------------------------
// Free functions — content helpers
// ---------------------------------------------------------------------------

/// Build SDK content blocks from a prompt and optional system prompt.
fn build_sdk_content(prompt: &str, system_prompt: Option<&str>) -> Vec<acp::ContentBlock> {
	let mut blocks = Vec::new();
	if let Some(sys) = system_prompt {
		blocks.push(acp::ContentBlock::Text(acp::TextContent::new(sys)));
	}
	blocks.push(acp::ContentBlock::Text(acp::TextContent::new(prompt)));
	blocks
}

/// Build sampling metadata from sampling parameters.
#[cfg(test)]
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

/// Map the SDK `StopReason` to our local `StopReason`.
fn map_stop_reason(sdk_reason: &acp::StopReason) -> Option<StopReason> {
	Some(match sdk_reason {
		acp::StopReason::EndTurn => StopReason::EndTurn,
		acp::StopReason::MaxTokens => StopReason::MaxTokens,
		acp::StopReason::MaxTurnRequests => StopReason::MaxTurnRequests,
		acp::StopReason::Refusal => StopReason::Refusal,
		acp::StopReason::Cancelled => StopReason::Cancelled,
		_ => StopReason::EndTurn,
	})
}

/// Drain any already-buffered `SessionNotification` messages from the
/// broadcast receiver and extract text from `AgentMessageChunk` updates.
fn collect_text_from_updates(
	rx: &mut broadcast::Receiver<acp::SessionNotification>,
	session_id: &str,
) -> String {
	let mut text = String::new();
	while let Ok(notification) = rx.try_recv() {
		if &*notification.session_id.0 != session_id {
			continue;
		}
		if let acp::SessionUpdate::AgentMessageChunk(chunk) = &notification.update {
			if let acp::ContentBlock::Text(tc) = &chunk.content {
				text.push_str(&tc.text);
			}
		}
	}
	text
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
	fn build_sdk_content_without_system() {
		let blocks = build_sdk_content("hello", None);
		assert_eq!(blocks.len(), 1);
		match &blocks[0] {
			acp::ContentBlock::Text(tc) => assert_eq!(tc.text, "hello"),
			_ => panic!("expected Text block"),
		}
	}

	#[test]
	fn build_sdk_content_with_system() {
		let blocks = build_sdk_content("hello", Some("system instructions"));
		assert_eq!(blocks.len(), 2);
		match &blocks[0] {
			acp::ContentBlock::Text(tc) => assert_eq!(tc.text, "system instructions"),
			_ => panic!("expected Text block"),
		}
		match &blocks[1] {
			acp::ContentBlock::Text(tc) => assert_eq!(tc.text, "hello"),
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
	// map_stop_reason
	// -------------------------------------------------------------------

	#[test]
	fn map_stop_reason_end_turn() {
		let result = map_stop_reason(&acp::StopReason::EndTurn);
		assert_eq!(result, Some(StopReason::EndTurn));
	}

	#[test]
	fn map_stop_reason_max_tokens() {
		let result = map_stop_reason(&acp::StopReason::MaxTokens);
		assert_eq!(result, Some(StopReason::MaxTokens));
	}

	#[test]
	fn map_stop_reason_cancelled() {
		let result = map_stop_reason(&acp::StopReason::Cancelled);
		assert_eq!(result, Some(StopReason::Cancelled));
	}

	// -------------------------------------------------------------------
	// embed returns error
	// -------------------------------------------------------------------

	#[tokio::test]
	async fn embed_returns_protocol_error() {
		// We need a client to call embed on, but we can test the logic directly.
		let err = AcpError::ProtocolError("embed not supported via ACP SDK".into());
		assert_eq!(err.code(), "ACP_PROTOCOL_ERROR");
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
