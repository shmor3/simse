// ---------------------------------------------------------------------------
// MCP Client — manages connections to one or more external MCP servers
// ---------------------------------------------------------------------------
//
// Responsibilities:
//   - Creating and managing transports (stdio/HTTP) for configured servers
//   - Connection deduplication (concurrent connect() calls share one attempt)
//   - Aggregating tools, resources, prompts across all connected servers
//   - Tool calls with retry and circuit breaker protection
//   - Resource reads with retry and circuit breaker protection
//   - Notification routing (tools/list_changed, logging messages)
//   - Completion requests and workspace roots management
//   - Per-server health monitoring
//
// Resilience:
//   - CircuitBreaker: Closed/Open/HalfOpen state machine per server
//   - HealthMonitor: consecutive failure tracking with sliding window
//   - Retry: exponential backoff with deterministic jitter (2 attempts, 500ms)
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tokio::sync::{broadcast, Mutex};

use crate::error::McpError;
use crate::http_transport::{HttpTransport, HttpTransportConfig};
use crate::protocol::{
	CompletionArg, CompletionRef, CompletionResult, GetPromptParams, GetPromptResult,
	LoggingMessage, McpClientConfig, PromptInfo, ReadResourceParams, ReadResourceResult,
	ResourceInfo, ResourceTemplateInfo, Root, ServerConnection, ToolCallParams, ToolCallResult,
	ToolInfo, TransportType,
};
use crate::stdio_transport::{NotificationHandler, StdioTransport, StdioTransportConfig, SubscriptionHandle, Transport};
use simse_resilience::{CircuitBreaker, HealthMonitor, HealthSnapshot, HealthStatus};

// ===========================================================================
// Resilience — MCP-specific retry configuration and helpers
// ===========================================================================

/// MCP retry configuration with MCP-specific defaults (2 attempts, 5s max).
#[derive(Debug, Clone)]
pub struct RetryConfig {
	/// Maximum number of attempts (including the first). Default 2.
	pub max_attempts: u32,
	/// Base delay in milliseconds before the first retry. Default 500.
	pub base_delay_ms: u64,
	/// Maximum delay cap in milliseconds. Default 5,000.
	pub max_delay_ms: u64,
	/// Multiplier applied to the delay after each retry. Default 2.0.
	pub backoff_multiplier: f64,
	/// Jitter factor (0.0 - 1.0). Default 0.25.
	pub jitter_factor: f64,
}

impl Default for RetryConfig {
	fn default() -> Self {
		Self {
			max_attempts: 2,
			base_delay_ms: 500,
			max_delay_ms: 5_000,
			backoff_multiplier: 2.0,
			jitter_factor: 0.25,
		}
	}
}

impl RetryConfig {
	fn to_shared(&self) -> simse_resilience::RetryConfig {
		simse_resilience::RetryConfig {
			max_attempts: self.max_attempts,
			base_delay_ms: self.base_delay_ms,
			max_delay_ms: self.max_delay_ms,
			backoff_multiplier: self.backoff_multiplier,
			jitter_factor: self.jitter_factor,
		}
	}
}

/// Returns `true` if the error is likely transient and worth retrying.
pub fn is_transient(error: &McpError) -> bool {
	matches!(
		error,
		McpError::Timeout { .. } | McpError::ConnectionFailed(_) | McpError::Io(_)
	)
}

/// Execute an async closure with automatic retries and exponential backoff.
///
/// Only transient errors trigger a retry. Non-transient errors propagate
/// immediately.
pub async fn retry<T, F, Fut>(config: &RetryConfig, f: F) -> Result<T, McpError>
where
	F: Fn() -> Fut,
	Fut: std::future::Future<Output = Result<T, McpError>>,
{
	simse_resilience::retry(&config.to_shared(), is_transient, f).await
}

// ===========================================================================
// Transport Enum — wraps concrete transport types since Transport is not
// dyn-compatible (request<T> is generic)
// ===========================================================================

/// Enum wrapper for transport types, since the `Transport` trait has generic
/// methods and is not dyn-compatible.
pub enum TransportKind {
	Stdio(Box<StdioTransport>),
	Http(HttpTransport),
}

impl std::fmt::Debug for TransportKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Stdio(_) => f.debug_tuple("Stdio").finish(),
			Self::Http(_) => f.debug_tuple("Http").finish(),
		}
	}
}

impl TransportKind {
	/// Establish the connection and perform the MCP handshake.
	async fn connect(&mut self) -> Result<crate::protocol::McpInitializeResult, McpError> {
		match self {
			Self::Stdio(t) => t.connect().await,
			Self::Http(t) => t.connect().await,
		}
	}

	/// Send a JSON-RPC request and wait for the response.
	async fn request<T: serde::de::DeserializeOwned + Send>(
		&self,
		method: &str,
		params: serde_json::Value,
	) -> Result<T, McpError> {
		match self {
			Self::Stdio(t) => t.request(method, params).await,
			Self::Http(t) => t.request(method, params).await,
		}
	}

	/// Register a handler for incoming notifications.
	fn on_notification(&self, method: &str, handler: NotificationHandler) -> SubscriptionHandle {
		match self {
			Self::Stdio(t) => t.on_notification(method, handler),
			Self::Http(t) => t.on_notification(method, handler),
		}
	}

	/// Close the connection and clean up resources.
	async fn close(&mut self) -> Result<(), McpError> {
		match self {
			Self::Stdio(t) => t.close().await,
			Self::Http(t) => t.close().await,
		}
	}

}

// ===========================================================================
// MCP Client
// ===========================================================================

/// An active connection to a single MCP server.
struct ConnectedServer {
	/// The server configuration that was used to connect.
	_config: ServerConnection,
	/// The underlying transport.
	transport: TransportKind,
	/// Subscription handles for notification handlers (kept alive).
	_subscriptions: Vec<SubscriptionHandle>,
}

/// Type alias: in-flight connection deduplication map.
type ConnectingMap = HashMap<String, broadcast::Sender<Result<(), String>>>;

/// Type alias: tools-changed handler list.
type ToolsChangedHandlers = Vec<Box<dyn Fn() + Send + Sync>>;

/// Type alias: logging handler list.
type LoggingHandlers = Vec<Box<dyn Fn(LoggingMessage) + Send + Sync>>;

/// The MCP client manages connections to one or more external MCP servers.
///
/// It provides aggregated access to tools, resources, and prompts across all
/// connected servers, with per-server circuit breakers and health monitors.
pub struct McpClient {
	/// Active connections keyed by server name.
	connections: HashMap<String, ConnectedServer>,
	/// In-flight connection attempts for deduplication.
	connecting: Arc<Mutex<ConnectingMap>>,
	/// Per-server circuit breakers.
	circuit_breakers: HashMap<String, CircuitBreaker>,
	/// Per-server health monitors.
	health_monitors: HashMap<String, HealthMonitor>,
	/// Client configuration.
	config: McpClientConfig,
	/// Current workspace roots.
	roots: Vec<Root>,
	/// Handlers invoked when any server's tool list changes.
	tools_changed_handlers: Arc<RwLock<ToolsChangedHandlers>>,
	/// Handlers invoked when a logging message arrives.
	logging_handlers: Arc<RwLock<LoggingHandlers>>,
}

impl McpClient {
	/// Create a new MCP client with the given configuration.
	///
	/// No connections are established until [`connect`] or [`connect_all`] is called.
	pub fn new(config: McpClientConfig) -> Self {
		Self {
			connections: HashMap::new(),
			connecting: Arc::new(Mutex::new(HashMap::new())),
			circuit_breakers: HashMap::new(),
			health_monitors: HashMap::new(),
			config,
			roots: Vec::new(),
			tools_changed_handlers: Arc::new(RwLock::new(Vec::new())),
			logging_handlers: Arc::new(RwLock::new(Vec::new())),
		}
	}

	// -----------------------------------------------------------------------
	// Connection management
	// -----------------------------------------------------------------------

	/// Connect to a configured MCP server by name.
	///
	/// If a connection attempt is already in flight for this server, the caller
	/// joins the existing attempt (connection deduplication).
	pub async fn connect(&mut self, server_name: &str) -> Result<(), McpError> {
		// Check for in-flight connection attempt
		{
			let connecting = self.connecting.lock().await;
			if let Some(tx) = connecting.get(server_name) {
				let mut rx = tx.subscribe();
				drop(connecting);
				return match rx.recv().await {
					Ok(Ok(())) => Ok(()),
					Ok(Err(msg)) => Err(McpError::ConnectionFailed(msg)),
					Err(_) => Err(McpError::ConnectionFailed(
						"Connection attempt channel closed".into(),
					)),
				};
			}
		}

		// Register this connection attempt for deduplication
		let (tx, _) = broadcast::channel(1);
		{
			let mut connecting = self.connecting.lock().await;
			connecting.insert(server_name.to_string(), tx.clone());
		}

		let result = self.do_connect(server_name).await;

		// Notify any waiters
		let _ = tx.send(result.as_ref().map(|_| ()).map_err(|e| e.to_string()));

		// Remove from in-flight map
		{
			let mut connecting = self.connecting.lock().await;
			connecting.remove(server_name);
		}

		result
	}

	/// Internal: perform the actual connection to a server.
	async fn do_connect(&mut self, server_name: &str) -> Result<(), McpError> {
		let server_config = self
			.config
			.servers
			.iter()
			.find(|s| s.name == server_name)
			.ok_or_else(|| {
				McpError::ConnectionFailed(format!(
					"No MCP server configured with name \"{}\"",
					server_name
				))
			})?
			.clone();

		// Disconnect existing connection to prevent resource leaks
		if self.connections.contains_key(server_name) {
			self.disconnect(server_name).await?;
		}

		tracing::debug!("Connecting to MCP server \"{}\"", server_name);

		let mut transport = self.create_transport(&server_config)?;

		// Perform the MCP handshake
		let _init_result = transport.connect().await.map_err(|e| {
			McpError::ConnectionFailed(format!(
				"Connection to \"{}\" failed: {}",
				server_name, e
			))
		})?;

		// Register notification handlers
		let tools_handlers = Arc::clone(&self.tools_changed_handlers);
		let logging_handlers = Arc::clone(&self.logging_handlers);

		let tools_sub = transport.on_notification(
			"notifications/tools/list_changed",
			Box::new(move |_params| {
				let handlers = tools_handlers.read().unwrap();
				for handler in handlers.iter() {
					handler();
				}
			}),
		);

		let logging_sub = transport.on_notification(
			"notifications/message",
			Box::new(move |params| {
				if let Ok(msg) = serde_json::from_value::<LoggingMessage>(params) {
					let handlers = logging_handlers.read().unwrap();
					for handler in handlers.iter() {
						handler(msg.clone());
					}
				}
			}),
		);

		// Create circuit breaker and health monitor
		self.circuit_breakers
			.insert(server_name.to_string(), CircuitBreaker::default());
		self.health_monitors
			.insert(server_name.to_string(), HealthMonitor::default());

		self.connections.insert(
			server_name.to_string(),
			ConnectedServer {
				_config: server_config,
				transport,
				_subscriptions: vec![tools_sub, logging_sub],
			},
		);

		tracing::info!("Connected to MCP server \"{}\"", server_name);
		Ok(())
	}

	/// Create a transport from a server configuration.
	fn create_transport(
		&self,
		server_config: &ServerConnection,
	) -> Result<TransportKind, McpError> {
		let client_name = self
			.config
			.client_name
			.clone()
			.unwrap_or_else(|| "simse".into());
		let client_version = self
			.config
			.client_version
			.clone()
			.unwrap_or_else(|| "1.0.0".into());

		match server_config.transport {
			TransportType::Stdio => {
				let command = server_config.command.clone().ok_or_else(|| {
					McpError::TransportConfigError(format!(
						"Server \"{}\" uses stdio transport but has no \"command\" field",
						server_config.name
					))
				})?;

				let config = StdioTransportConfig {
					command,
					args: server_config.args.clone().unwrap_or_default(),
					cwd: None,
					env: server_config.env.clone().unwrap_or_default(),
					timeout_ms: 60_000,
					client_name,
					client_version,
				};

				Ok(TransportKind::Stdio(Box::new(StdioTransport::new(config))))
			}
			TransportType::Http => {
				let url = server_config.url.clone().ok_or_else(|| {
					McpError::TransportConfigError(format!(
						"Server \"{}\" uses http transport but has no \"url\" field",
						server_config.name
					))
				})?;

				let config = HttpTransportConfig {
					url,
					timeout_ms: 60_000,
					headers: HashMap::new(),
					client_name,
					client_version,
				};

				Ok(TransportKind::Http(HttpTransport::new(config)))
			}
		}
	}

	/// Connect to all configured servers in parallel.
	///
	/// Returns the names of servers that connected successfully.
	pub async fn connect_all(&mut self) -> Result<Vec<String>, McpError> {
		let server_names: Vec<String> = self.config.servers.iter().map(|s| s.name.clone()).collect();
		let mut connected = Vec::new();

		// Connect sequentially since we need &mut self
		// (parallel would require interior mutability on connections map)
		for name in &server_names {
			match self.connect(name).await {
				Ok(()) => {
					connected.push(name.clone());
				}
				Err(e) => {
					tracing::warn!("Failed to connect to MCP server \"{}\": {}", name, e);
				}
			}
		}

		Ok(connected)
	}

	/// Disconnect from a server.
	pub async fn disconnect(&mut self, server_name: &str) -> Result<(), McpError> {
		if let Some(mut conn) = self.connections.remove(server_name) {
			tracing::debug!("Disconnecting from MCP server \"{}\"", server_name);

			if let Err(e) = conn.transport.close().await {
				tracing::warn!("Error disconnecting from \"{}\": {}", server_name, e);
			}

			self.circuit_breakers.remove(server_name);
			self.health_monitors.remove(server_name);

			tracing::info!("Disconnected from MCP server \"{}\"", server_name);
		}
		Ok(())
	}

	/// Disconnect from all connected servers.
	pub async fn disconnect_all(&mut self) -> Result<(), McpError> {
		let names: Vec<String> = self.connections.keys().cloned().collect();
		for name in names {
			self.disconnect(&name).await?;
		}
		Ok(())
	}

	// -----------------------------------------------------------------------
	// Internal helpers
	// -----------------------------------------------------------------------

	/// Get a reference to a connected server's transport.
	fn require_connected(&self, server_name: &str) -> Result<&TransportKind, McpError> {
		let conn = self.connections.get(server_name).ok_or_else(|| {
			McpError::ServerNotConnected(server_name.to_string())
		})?;
		Ok(&conn.transport)
	}

	/// Get entries matching the server filter (None = all connected servers).
	fn get_target_servers<'a>(
		&'a self,
		server: Option<&'a str>,
	) -> Result<Vec<&'a str>, McpError> {
		if let Some(name) = server {
			self.require_connected(name)?;
			Ok(vec![name])
		} else {
			Ok(self.connections.keys().map(|s| s.as_str()).collect())
		}
	}

	// -----------------------------------------------------------------------
	// Tools
	// -----------------------------------------------------------------------

	/// List tools from one server or all connected servers.
	pub async fn list_tools(&self, server: Option<&str>) -> Result<Vec<ToolInfo>, McpError> {
		let servers = self.get_target_servers(server)?;
		let mut all_tools = Vec::new();

		for name in servers {
			let transport = self.require_connected(name)?;

			let retry_config = RetryConfig::default();
			let tools_response: ToolsListResult = retry(&retry_config, || async {
				transport
					.request("tools/list", serde_json::json!({}))
					.await
			})
			.await?;

			all_tools.extend(tools_response.tools);
		}

		Ok(all_tools)
	}

	/// Call a tool on a specific server.
	///
	/// Applies circuit breaker protection and retry with exponential backoff.
	pub async fn call_tool(
		&self,
		server: &str,
		tool: &str,
		args: serde_json::Value,
	) -> Result<ToolCallResult, McpError> {
		let transport = self.require_connected(server)?;

		// Check circuit breaker
		if let Some(cb) = self.circuit_breakers.get(server) {
			if !cb.allow_request() {
				return Err(McpError::CircuitBreakerOpen(format!(
					"Circuit breaker open for server \"{}\"",
					server
				)));
			}
		}

		let params = ToolCallParams {
			name: tool.to_string(),
			arguments: args,
		};

		let retry_config = RetryConfig::default();
		let params_json = serde_json::to_value(&params)
			.map_err(|e| McpError::Serialization(e.to_string()))?;

		let result: Result<ToolCallResult, McpError> = retry(&retry_config, || {
			let pj = params_json.clone();
			async move { transport.request("tools/call", pj).await }
		})
		.await;

		match &result {
			Ok(_) => {
				if let Some(cb) = self.circuit_breakers.get(server) {
					cb.record_success();
				}
				if let Some(hm) = self.health_monitors.get(server) {
					hm.record_success();
				}
			}
			Err(e) => {
				if let Some(cb) = self.circuit_breakers.get(server) {
					cb.record_failure();
				}
				if let Some(hm) = self.health_monitors.get(server) {
					hm.record_failure(&e.to_string());
				}
			}
		}

		result
	}

	// -----------------------------------------------------------------------
	// Resources
	// -----------------------------------------------------------------------

	/// List resources from one server or all connected servers.
	pub async fn list_resources(
		&self,
		server: Option<&str>,
	) -> Result<Vec<ResourceInfo>, McpError> {
		let servers = self.get_target_servers(server)?;
		let mut all_resources = Vec::new();

		for name in servers {
			let transport = self.require_connected(name)?;

			let retry_config = RetryConfig::default();
			let resources_response: ResourcesListResult = retry(&retry_config, || async {
				transport
					.request("resources/list", serde_json::json!({}))
					.await
			})
			.await?;

			all_resources.extend(resources_response.resources);
		}

		Ok(all_resources)
	}

	/// Read a resource from a specific server.
	///
	/// Returns the text content of the first resource content item.
	pub async fn read_resource(&self, server: &str, uri: &str) -> Result<String, McpError> {
		let transport = self.require_connected(server)?;

		// Check circuit breaker
		if let Some(cb) = self.circuit_breakers.get(server) {
			if !cb.allow_request() {
				return Err(McpError::CircuitBreakerOpen(format!(
					"Circuit breaker open for server \"{}\"",
					server
				)));
			}
		}

		let params = ReadResourceParams {
			uri: uri.to_string(),
		};
		let params_json = serde_json::to_value(&params)
			.map_err(|e| McpError::Serialization(e.to_string()))?;

		let retry_config = RetryConfig::default();
		let result: Result<ReadResourceResult, McpError> = retry(&retry_config, || {
			let pj = params_json.clone();
			async move { transport.request("resources/read", pj).await }
		})
		.await;

		match &result {
			Ok(_) => {
				if let Some(cb) = self.circuit_breakers.get(server) {
					cb.record_success();
				}
				if let Some(hm) = self.health_monitors.get(server) {
					hm.record_success();
				}
			}
			Err(e) => {
				if let Some(cb) = self.circuit_breakers.get(server) {
					cb.record_failure();
				}
				if let Some(hm) = self.health_monitors.get(server) {
					hm.record_failure(&e.to_string());
				}
			}
		}

		let read_result = result?;
		let first = read_result.contents.first();
		match first {
			Some(content) => {
				if let Some(text) = &content.text {
					Ok(text.clone())
				} else {
					Ok(serde_json::to_string(content)
						.map_err(|e| McpError::Serialization(e.to_string()))?)
				}
			}
			None => Ok(String::new()),
		}
	}

	/// List resource templates from one server or all connected servers.
	pub async fn list_resource_templates(
		&self,
		server: Option<&str>,
	) -> Result<Vec<ResourceTemplateInfo>, McpError> {
		let servers = self.get_target_servers(server)?;
		let mut all_templates = Vec::new();

		for name in servers {
			let transport = self.require_connected(name)?;

			let retry_config = RetryConfig::default();
			let result: Result<ResourceTemplatesListResult, McpError> =
				retry(&retry_config, || async {
					transport
						.request("resources/templates/list", serde_json::json!({}))
						.await
				})
				.await;

			match result {
				Ok(response) => all_templates.extend(response.resource_templates),
				Err(_) => {
					// Server may not support resource templates — skip silently
				}
			}
		}

		Ok(all_templates)
	}

	// -----------------------------------------------------------------------
	// Prompts
	// -----------------------------------------------------------------------

	/// List prompts from one server or all connected servers.
	pub async fn list_prompts(&self, server: Option<&str>) -> Result<Vec<PromptInfo>, McpError> {
		let servers = self.get_target_servers(server)?;
		let mut all_prompts = Vec::new();

		for name in servers {
			let transport = self.require_connected(name)?;

			let retry_config = RetryConfig::default();
			let prompts_response: PromptsListResult = retry(&retry_config, || async {
				transport
					.request("prompts/list", serde_json::json!({}))
					.await
			})
			.await?;

			all_prompts.extend(prompts_response.prompts);
		}

		Ok(all_prompts)
	}

	/// Get a prompt from a specific server.
	pub async fn get_prompt(
		&self,
		server: &str,
		name: &str,
		args: serde_json::Value,
	) -> Result<GetPromptResult, McpError> {
		let transport = self.require_connected(server)?;

		let params = GetPromptParams {
			name: name.to_string(),
			arguments: args,
		};
		let params_json = serde_json::to_value(&params)
			.map_err(|e| McpError::Serialization(e.to_string()))?;

		transport.request("prompts/get", params_json).await
	}

	// -----------------------------------------------------------------------
	// Logging & notifications
	// -----------------------------------------------------------------------

	/// Set the logging level on a specific server.
	pub async fn set_logging_level(&self, server: &str, level: &str) -> Result<(), McpError> {
		let transport = self.require_connected(server)?;

		transport
			.request::<serde_json::Value>(
				"logging/setLevel",
				serde_json::json!({ "level": level }),
			)
			.await?;

		Ok(())
	}

	/// Register a handler invoked whenever any server's tool list changes.
	///
	/// Returns a handler ID that can be used for identification.
	pub fn on_tools_changed(&self, handler: impl Fn() + Send + Sync + 'static) -> usize {
		let mut handlers = self.tools_changed_handlers.write().unwrap();
		handlers.push(Box::new(handler));
		handlers.len() - 1
	}

	/// Register a handler invoked whenever a logging message arrives.
	///
	/// Returns a handler ID that can be used for identification.
	pub fn on_logging_message(
		&self,
		handler: impl Fn(LoggingMessage) + Send + Sync + 'static,
	) -> usize {
		let mut handlers = self.logging_handlers.write().unwrap();
		handlers.push(Box::new(handler));
		handlers.len() - 1
	}

	// -----------------------------------------------------------------------
	// Completions & roots
	// -----------------------------------------------------------------------

	/// Request completions from a specific server.
	pub async fn complete(
		&self,
		server: &str,
		reference: CompletionRef,
		argument: CompletionArg,
	) -> Result<CompletionResult, McpError> {
		let transport = self.require_connected(server)?;

		let params = serde_json::json!({
			"ref": serde_json::to_value(&reference)
				.map_err(|e| McpError::Serialization(e.to_string()))?,
			"argument": serde_json::to_value(&argument)
				.map_err(|e| McpError::Serialization(e.to_string()))?,
		});

		let result: CompletionResponse = transport.request("completion/complete", params).await?;
		Ok(result.completion)
	}

	/// Set the workspace roots. Consumes self, returns the updated client.
	/// Does not notify servers — call `send_roots_list_changed()` separately
	/// if needed.
	pub fn set_roots(mut self, roots: Vec<Root>) -> Self {
		self.roots = roots;
		self
	}

	/// Get the current workspace roots.
	pub fn roots(&self) -> &[Root] {
		&self.roots
	}

	// -----------------------------------------------------------------------
	// Health
	// -----------------------------------------------------------------------

	/// Check if a specific server (or any server) is available.
	///
	/// - `Some(name)` — returns `true` if that server is connected.
	/// - `None` — returns `true` if any server is connected.
	pub fn is_available(&self, server: Option<&str>) -> bool {
		if let Some(name) = server {
			self.connections.contains_key(name)
		} else {
			!self.connections.is_empty()
		}
	}

	/// Return the names of all currently connected servers.
	pub fn connected_server_names(&self) -> Vec<String> {
		self.connections.keys().cloned().collect()
	}

	/// Return the number of active connections.
	pub fn connection_count(&self) -> usize {
		self.connections.len()
	}

	/// Return the health snapshot for a specific server.
	pub fn get_server_health(&self, server_name: &str) -> Option<HealthSnapshot> {
		self.health_monitors.get(server_name).map(|hm| hm.snapshot())
	}

	/// Return the health status for a specific server.
	pub fn get_server_health_status(&self, server_name: &str) -> Option<HealthStatus> {
		self.health_monitors.get(server_name).map(|hm| hm.status())
	}
}

// ---------------------------------------------------------------------------
// Helper response types for deserializing list results
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct ToolsListResult {
	#[serde(default)]
	tools: Vec<ToolInfo>,
}

#[derive(Debug, serde::Deserialize)]
struct ResourcesListResult {
	#[serde(default)]
	resources: Vec<ResourceInfo>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceTemplatesListResult {
	#[serde(default)]
	resource_templates: Vec<ResourceTemplateInfo>,
}

#[derive(Debug, serde::Deserialize)]
struct PromptsListResult {
	#[serde(default)]
	prompts: Vec<PromptInfo>,
}

#[derive(Debug, serde::Deserialize)]
struct CompletionResponse {
	completion: CompletionResult,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::{AtomicU32, Ordering};
	use std::sync::Mutex as StdMutex;

	// -----------------------------------------------------------------------
	// Retry tests (MCP-specific wrapper over simse-resilience)
	// -----------------------------------------------------------------------

	#[tokio::test]
	async fn retry_succeeds_on_first_attempt() {
		let config = RetryConfig::default();
		let call_count = Arc::new(AtomicU32::new(0));
		let cc = call_count.clone();

		let result = retry(&config, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Ok::<_, McpError>(42)
			}
		})
		.await;

		assert_eq!(result.unwrap(), 42);
		assert_eq!(call_count.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn retry_retries_transient_error_and_succeeds() {
		let config = RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1,
			max_delay_ms: 10,
			..Default::default()
		};
		let call_count = Arc::new(AtomicU32::new(0));
		let cc = call_count.clone();

		let result = retry(&config, || {
			let cc = cc.clone();
			async move {
				let attempt = cc.fetch_add(1, Ordering::SeqCst) + 1;
				if attempt < 2 {
					Err(McpError::Timeout {
						method: "test".into(),
						timeout_ms: 1000,
					})
				} else {
					Ok(99)
				}
			}
		})
		.await;

		assert_eq!(result.unwrap(), 99);
		assert_eq!(call_count.load(Ordering::SeqCst), 2);
	}

	#[tokio::test]
	async fn retry_stops_after_max_attempts() {
		let config = RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1,
			max_delay_ms: 10,
			..Default::default()
		};
		let call_count = Arc::new(AtomicU32::new(0));
		let cc = call_count.clone();

		let result: Result<i32, McpError> = retry(&config, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Err(McpError::ConnectionFailed("down".into()))
			}
		})
		.await;

		assert!(result.is_err());
		assert_eq!(call_count.load(Ordering::SeqCst), 3);
	}

	#[tokio::test]
	async fn retry_does_not_retry_non_transient_errors() {
		let config = RetryConfig {
			max_attempts: 5,
			base_delay_ms: 1,
			..Default::default()
		};
		let call_count = Arc::new(AtomicU32::new(0));
		let cc = call_count.clone();

		let result: Result<i32, McpError> = retry(&config, || {
			let cc = cc.clone();
			async move {
				cc.fetch_add(1, Ordering::SeqCst);
				Err(McpError::ProtocolError("invalid".into()))
			}
		})
		.await;

		assert!(result.is_err());
		assert_eq!(call_count.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn is_transient_classifies_errors_correctly() {
		assert!(is_transient(&McpError::Timeout {
			method: "m".into(),
			timeout_ms: 100,
		}));
		assert!(is_transient(&McpError::ConnectionFailed("x".into())));
		assert!(is_transient(&McpError::Io(std::io::Error::new(
			std::io::ErrorKind::Other,
			"io"
		))));

		assert!(!is_transient(&McpError::NotInitialized));
		assert!(!is_transient(&McpError::ProtocolError("x".into())));
		assert!(!is_transient(&McpError::Serialization("x".into())));
		assert!(!is_transient(&McpError::CircuitBreakerOpen("x".into())));
		assert!(!is_transient(&McpError::ServerNotConnected("x".into())));
		assert!(!is_transient(&McpError::TransportConfigError("x".into())));
	}

	// -----------------------------------------------------------------------
	// McpClient tests
	// -----------------------------------------------------------------------

	#[test]
	fn new_creates_client_with_config() {
		let config = McpClientConfig {
			servers: vec![ServerConnection {
				name: "test-server".into(),
				transport: TransportType::Stdio,
				command: Some("echo".into()),
				args: Some(vec!["hello".into()]),
				env: None,
				url: None,
			}],
			client_name: Some("test-client".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		assert_eq!(client.connection_count(), 0);
		assert!(client.connected_server_names().is_empty());
		assert!(!client.is_available(None));
	}

	#[test]
	fn connected_server_names_starts_empty() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		assert!(client.connected_server_names().is_empty());
		assert_eq!(client.connection_count(), 0);
	}

	#[test]
	fn is_available_returns_false_when_no_servers() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		assert!(!client.is_available(None));
		assert!(!client.is_available(Some("nonexistent")));
	}

	#[test]
	fn set_roots_stores_roots() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		assert!(client.roots().is_empty());

		let client = client.set_roots(vec![
			Root {
				uri: "file:///workspace".into(),
				name: Some("workspace".into()),
			},
			Root {
				uri: "file:///home".into(),
				name: None,
			},
		]);

		assert_eq!(client.roots().len(), 2);
		assert_eq!(client.roots()[0].uri, "file:///workspace");
		assert_eq!(client.roots()[0].name.as_deref(), Some("workspace"));
		assert_eq!(client.roots()[1].uri, "file:///home");
		assert!(client.roots()[1].name.is_none());
	}

	#[test]
	fn on_tools_changed_registers_handler() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		let called = Arc::new(AtomicU32::new(0));
		let called_clone = called.clone();

		let id = client.on_tools_changed(move || {
			called_clone.fetch_add(1, Ordering::SeqCst);
		});

		// Verify handler was registered (returns index)
		assert_eq!(id, 0);

		// Simulate calling handlers
		let handlers = client.tools_changed_handlers.read().unwrap();
		for handler in handlers.iter() {
			handler();
		}
		assert_eq!(called.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn on_logging_message_registers_handler() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		let received = Arc::new(StdMutex::new(Vec::new()));
		let received_clone = received.clone();

		let id = client.on_logging_message(move |msg| {
			received_clone.lock().unwrap().push(msg);
		});

		assert_eq!(id, 0);

		// Simulate calling handlers
		let msg = LoggingMessage {
			level: crate::protocol::LoggingLevel::Info,
			logger: Some("test-logger".into()),
			data: serde_json::json!("test message"),
		};
		let handlers = client.logging_handlers.read().unwrap();
		for handler in handlers.iter() {
			handler(msg.clone());
		}

		let guard = received.lock().unwrap();
		assert_eq!(guard.len(), 1);
		assert_eq!(guard[0].level, crate::protocol::LoggingLevel::Info);
	}

	#[tokio::test]
	async fn connect_fails_for_unknown_server() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let mut client = McpClient::new(config);
		let result = client.connect("nonexistent").await;
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, McpError::ConnectionFailed(_)));
		assert!(err.to_string().contains("nonexistent"));
	}

	#[test]
	fn create_transport_fails_for_stdio_without_command() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		let server_config = ServerConnection {
			name: "bad-server".into(),
			transport: TransportType::Stdio,
			command: None,
			args: None,
			env: None,
			url: None,
		};

		let result = client.create_transport(&server_config);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, McpError::TransportConfigError(_)));
		assert!(err.to_string().contains("command"));
	}

	#[test]
	fn create_transport_fails_for_http_without_url() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		let server_config = ServerConnection {
			name: "bad-server".into(),
			transport: TransportType::Http,
			command: None,
			args: None,
			env: None,
			url: None,
		};

		let result = client.create_transport(&server_config);
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, McpError::TransportConfigError(_)));
		assert!(err.to_string().contains("url"));
	}

	#[test]
	fn create_transport_succeeds_for_stdio() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		let server_config = ServerConnection {
			name: "good-server".into(),
			transport: TransportType::Stdio,
			command: Some("echo".into()),
			args: Some(vec!["hello".into()]),
			env: None,
			url: None,
		};

		let result = client.create_transport(&server_config);
		assert!(result.is_ok());
	}

	#[test]
	fn create_transport_succeeds_for_http() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		let server_config = ServerConnection {
			name: "good-server".into(),
			transport: TransportType::Http,
			command: None,
			args: None,
			env: None,
			url: Some("http://localhost:3000".into()),
		};

		let result = client.create_transport(&server_config);
		assert!(result.is_ok());
	}

	#[test]
	fn get_server_health_returns_none_for_unknown() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		assert!(client.get_server_health("unknown").is_none());
		assert!(client.get_server_health_status("unknown").is_none());
	}

	#[test]
	fn default_retry_config_has_mcp_specific_defaults() {
		let cfg = RetryConfig::default();
		assert_eq!(cfg.max_attempts, 2);
		assert_eq!(cfg.base_delay_ms, 500);
		assert_eq!(cfg.max_delay_ms, 5_000);
		assert!((cfg.backoff_multiplier - 2.0).abs() < f64::EPSILON);
		assert!((cfg.jitter_factor - 0.25).abs() < f64::EPSILON);
	}

	#[tokio::test]
	async fn disconnect_all_on_empty_client() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let mut client = McpClient::new(config);
		let result = client.disconnect_all().await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn require_connected_fails_for_unknown_server() {
		let config = McpClientConfig {
			servers: vec![],
			client_name: Some("test".into()),
			client_version: Some("1.0.0".into()),
		};

		let client = McpClient::new(config);
		let result = client.require_connected("unknown");
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, McpError::ServerNotConnected(_)));
	}
}
