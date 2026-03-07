// ---------------------------------------------------------------------------
// MCP Stdio Transport — manages a child process running an MCP server
// ---------------------------------------------------------------------------
//
// Responsibilities:
//   - Spawning the MCP server process with stdio pipes
//   - NDJSON buffer parsing from stdout
//   - Pending request tracking with timeouts
//   - Sending JSON-RPC requests and receiving responses
//   - Routing notifications to registered handlers
//   - MCP initialize/initialized handshake on connect
//   - Health checking (is process alive?)
//   - Cleanup on close
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

use crate::mcp::error::McpError;
use crate::mcp::protocol::{
	ClientCapabilities, ImplementationInfo, JsonRpcNotification, McpInitializeParams,
	McpInitializeResult, RootCapabilities,
};

// ---------------------------------------------------------------------------
// Protocol version
// ---------------------------------------------------------------------------

/// The MCP protocol version used during the initialize handshake.
pub const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

// ---------------------------------------------------------------------------
// Transport trait
// ---------------------------------------------------------------------------

/// Handler invoked when a notification arrives from the MCP server.
pub type NotificationHandler = Box<dyn Fn(serde_json::Value) + Send + Sync>;

/// A handle that, when dropped, unregisters the associated notification handler.
///
/// Call [`SubscriptionHandle::unsubscribe`] explicitly or let the handle go out
/// of scope to remove the handler.
pub struct SubscriptionHandle {
	pub(crate) active: Arc<AtomicBool>,
}

impl SubscriptionHandle {
	/// Explicitly unsubscribe (same effect as dropping the handle).
	pub fn unsubscribe(&self) {
		self.active.store(false, Ordering::SeqCst);
	}

	/// Returns `true` if this subscription is still active.
	pub fn is_active(&self) -> bool {
		self.active.load(Ordering::SeqCst)
	}
}

impl Drop for SubscriptionHandle {
	fn drop(&mut self) {
		self.active.store(false, Ordering::SeqCst);
	}
}

/// A notification handler entry stored in the registry.
struct HandlerEntry {
	active: Arc<AtomicBool>,
	handler: NotificationHandler,
}

/// The `Transport` trait defines the interface for MCP transports.
///
/// Transports handle the low-level communication between an MCP client and
/// an MCP server, abstracting over the underlying mechanism (stdio, HTTP, etc.).
#[async_trait]
pub trait Transport: Send + Sync {
	/// Establish the connection and perform the MCP initialize/initialized
	/// handshake.
	async fn connect(&mut self) -> Result<McpInitializeResult, McpError>;

	/// Send a JSON-RPC request and wait for the response.
	async fn request<T: DeserializeOwned + Send>(
		&self,
		method: &str,
		params: serde_json::Value,
	) -> Result<T, McpError>;

	/// Register a handler for incoming notifications with the given method.
	/// Returns a [`SubscriptionHandle`] that removes the handler when dropped.
	///
	/// Not all transports support server-initiated notifications. If the
	/// transport does not support them, this returns a no-op handle.
	fn on_notification(&self, method: &str, handler: NotificationHandler) -> SubscriptionHandle;

	/// Close the connection and clean up resources.
	async fn close(&mut self) -> Result<(), McpError>;

	/// Returns `true` if the transport is currently connected.
	fn is_connected(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Stdio transport types
// ---------------------------------------------------------------------------

/// A pending JSON-RPC request awaiting a response.
struct PendingRequest {
	sender: Option<oneshot::Sender<Result<serde_json::Value, McpError>>>,
}

/// Configuration for spawning an MCP server as a child process.
#[derive(Debug, Clone)]
pub struct StdioTransportConfig {
	/// The command to run (e.g. path to the MCP server binary).
	pub command: String,
	/// Arguments to pass to the command.
	pub args: Vec<String>,
	/// Working directory for the child process.
	pub cwd: Option<String>,
	/// Additional environment variables for the child process.
	pub env: HashMap<String, String>,
	/// Default timeout for JSON-RPC requests (milliseconds). Default: 60 000.
	pub timeout_ms: u64,
	/// Client name sent during initialization. Default: `"simse"`.
	pub client_name: String,
	/// Client version sent during initialization. Default: `"1.0.0"`.
	pub client_version: String,
}

impl Default for StdioTransportConfig {
	fn default() -> Self {
		Self {
			command: String::new(),
			args: Vec::new(),
			cwd: None,
			env: HashMap::new(),
			timeout_ms: 60_000,
			client_name: "simse".into(),
			client_version: "1.0.0".into(),
		}
	}
}

// ---------------------------------------------------------------------------
// StdioTransport
// ---------------------------------------------------------------------------

/// Manages a single MCP server child process over JSON-RPC 2.0 / NDJSON stdio.
pub struct StdioTransport {
	/// Configuration for spawning the child process.
	config: StdioTransportConfig,
	/// Handle to the child process (taken on close).
	child: Mutex<Option<Child>>,
	/// Stdin pipe for writing requests/notifications.
	stdin: Arc<Mutex<Option<ChildStdin>>>,
	/// Monotonically increasing request ID counter.
	next_id: AtomicU64,
	/// In-flight requests awaiting responses.
	pending: Arc<Mutex<HashMap<u64, PendingRequest>>>,
	/// Notification handlers keyed by method name.
	notification_handlers: Arc<RwLock<HashMap<String, Vec<HandlerEntry>>>>,
	/// Whether the connection is alive and usable.
	connected: Arc<AtomicBool>,
	/// Default timeout for requests (ms).
	default_timeout_ms: u64,
	/// Handle to the stdout reader task so we can abort it on close.
	reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
	/// Handle to the stderr reader task.
	stderr_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl StdioTransport {
	/// Create a new `StdioTransport` with the given configuration.
	///
	/// The transport is not connected until [`connect`](Transport::connect)
	/// is called.
	pub fn new(config: StdioTransportConfig) -> Self {
		let default_timeout_ms = config.timeout_ms;
		Self {
			config,
			child: Mutex::new(None),
			stdin: Arc::new(Mutex::new(None)),
			next_id: AtomicU64::new(1),
			pending: Arc::new(Mutex::new(HashMap::new())),
			notification_handlers: Arc::new(RwLock::new(HashMap::new())),
			connected: Arc::new(AtomicBool::new(false)),
			default_timeout_ms,
			reader_handle: Mutex::new(None),
			stderr_handle: Mutex::new(None),
		}
	}

	// -------------------------------------------------------------------
	// Internal: spawn child process and start reader tasks
	// -------------------------------------------------------------------

	async fn spawn_child(&mut self) -> Result<(), McpError> {
		let mut cmd = Command::new(&self.config.command);
		cmd.args(&self.config.args)
			.stdin(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.kill_on_drop(true);

		if let Some(ref cwd) = self.config.cwd {
			cmd.current_dir(cwd);
		}

		for (k, v) in &self.config.env {
			cmd.env(k, v);
		}

		let mut child = cmd.spawn().map_err(|e| {
			McpError::ConnectionFailed(format!(
				"Failed to spawn '{}': {}",
				self.config.command, e
			))
		})?;

		let stdin = child.stdin.take();
		let stdout = child.stdout.take();
		let stderr = child.stderr.take();

		*self.stdin.lock().await = stdin;
		self.connected.store(true, Ordering::SeqCst);

		// Spawn stdout reader task.
		if let Some(stdout) = stdout {
			let pending_clone = Arc::clone(&self.pending);
			let handlers_clone = Arc::clone(&self.notification_handlers);
			let connected_clone = Arc::clone(&self.connected);
			let pending_for_exit = Arc::clone(&self.pending);

			let handle = tokio::spawn(async move {
				let mut reader = BufReader::new(stdout);
				let mut line_buf = String::new();

				loop {
					line_buf.clear();
					match reader.read_line(&mut line_buf).await {
						Ok(0) => {
							// EOF — child process closed stdout.
							tracing::debug!("MCP stdout EOF");
							break;
						}
						Ok(_) => {
							let trimmed = line_buf.trim();
							if trimmed.is_empty() {
								continue;
							}
							let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
								Ok(v) => v,
								Err(e) => {
									tracing::warn!("MCP: invalid JSON on stdout: {e}");
									continue;
								}
							};
							dispatch_message(&parsed, &pending_clone, &handlers_clone).await;
						}
						Err(e) => {
							tracing::warn!("MCP stdout read error: {e}");
							break;
						}
					}
				}

				// Mark connection as dead.
				connected_clone.store(false, Ordering::SeqCst);

				// Child stdout closed — reject all pending requests.
				let mut pending_guard = pending_for_exit.lock().await;
				for (_id, req) in pending_guard.drain() {
					if let Some(sender) = req.sender {
						let _ = sender.send(Err(McpError::ConnectionFailed(
							"MCP server closed stdout".into(),
						)));
					}
				}
			});
			*self.reader_handle.lock().await = Some(handle);
		}

		// Spawn stderr reader task — routes to tracing::warn.
		if let Some(stderr) = stderr {
			let handle = tokio::spawn(async move {
				let mut reader = BufReader::new(stderr);
				let mut line_buf = String::new();
				loop {
					line_buf.clear();
					match reader.read_line(&mut line_buf).await {
						Ok(0) => break,
						Ok(_) => {
							let text = line_buf.trim();
							if !text.is_empty() {
								tracing::warn!("MCP stderr: {text}");
							}
						}
						Err(e) => {
							tracing::warn!("MCP stderr read error: {e}");
							break;
						}
					}
				}
			});
			*self.stderr_handle.lock().await = Some(handle);
		}

		*self.child.lock().await = Some(child);
		Ok(())
	}

	// -------------------------------------------------------------------
	// Internal: send the MCP initialize handshake
	// -------------------------------------------------------------------

	async fn perform_handshake(&self) -> Result<McpInitializeResult, McpError> {
		let params = McpInitializeParams {
			protocol_version: MCP_PROTOCOL_VERSION.into(),
			capabilities: ClientCapabilities {
				roots: Some(RootCapabilities {
					list_changed: Some(true),
				}),
				sampling: None,
			},
			client_info: ImplementationInfo {
				name: self.config.client_name.clone(),
				version: self.config.client_version.clone(),
			},
		};

		// Step 1: Send `initialize` request and wait for server capabilities.
		let result: McpInitializeResult = self
			.request_internal("initialize", serde_json::to_value(&params).map_err(|e| {
				McpError::Serialization(e.to_string())
			})?)
			.await?;

		// Step 2: Send `notifications/initialized` notification (no response expected).
		self.send_notification("notifications/initialized", None)
			.await?;

		Ok(result)
	}

	// -------------------------------------------------------------------
	// Internal: send a request (used before trait methods are available)
	// -------------------------------------------------------------------

	async fn request_internal<T: DeserializeOwned>(
		&self,
		method: &str,
		params: serde_json::Value,
	) -> Result<T, McpError> {
		if !self.connected.load(Ordering::SeqCst) {
			return Err(McpError::ConnectionFailed(
				"MCP connection is not open".into(),
			));
		}

		let id = self.next_id.fetch_add(1, Ordering::SeqCst);
		let deadline = tokio::time::Instant::now()
			+ std::time::Duration::from_millis(self.default_timeout_ms);

		let (tx, rx) = oneshot::channel();

		// Register pending request.
		{
			let mut pending = self.pending.lock().await;
			pending.insert(
				id,
				PendingRequest {
					sender: Some(tx),
				},
			);
		}

		// Build and send the request.
		let request = serde_json::json!({
			"jsonrpc": "2.0",
			"id": id,
			"method": method,
			"params": params,
		});

		if let Err(e) = self.write_line(&request).await {
			// Remove the pending entry so the oneshot isn't leaked.
			let mut pending = self.pending.lock().await;
			pending.remove(&id);
			return Err(e);
		}

		// Await response with timeout.
		let timeout_duration = deadline.duration_since(tokio::time::Instant::now());
		match tokio::time::timeout(timeout_duration, rx).await {
			Ok(Ok(result)) => {
				let value = result?;
				serde_json::from_value(value).map_err(|e| {
					McpError::ProtocolError(format!(
						"Failed to deserialize response for '{}': {}",
						method, e
					))
				})
			}
			Ok(Err(_)) => {
				// oneshot sender dropped — connection likely closed.
				Err(McpError::ConnectionFailed(
					"Response channel closed".into(),
				))
			}
			Err(_) => {
				// Timeout — remove pending entry.
				let mut pending = self.pending.lock().await;
				pending.remove(&id);
				Err(McpError::Timeout {
					method: method.to_string(),
					timeout_ms: self.default_timeout_ms,
				})
			}
		}
	}

	// -------------------------------------------------------------------
	// Internal: write to stdin
	// -------------------------------------------------------------------

	async fn write_line(&self, value: &serde_json::Value) -> Result<(), McpError> {
		let mut stdin_guard = self.stdin.lock().await;
		let writer = stdin_guard
			.as_mut()
			.ok_or_else(|| McpError::ConnectionFailed("stdin not available".into()))?;

		let mut data =
			serde_json::to_vec(value).map_err(|e| McpError::Serialization(e.to_string()))?;
		data.push(b'\n');

		writer.write_all(&data).await.map_err(McpError::Io)?;
		writer.flush().await.map_err(McpError::Io)?;

		Ok(())
	}

	// -------------------------------------------------------------------
	// Internal: send a notification (fire-and-forget but awaitable)
	// -------------------------------------------------------------------

	async fn send_notification(
		&self,
		method: &str,
		params: Option<serde_json::Value>,
	) -> Result<(), McpError> {
		let notification = JsonRpcNotification::new(method, params);
		let json = serde_json::to_value(&notification)
			.map_err(|e| McpError::Serialization(e.to_string()))?;
		self.write_line(&json).await
	}
}

#[async_trait]
impl Transport for StdioTransport {
	async fn connect(&mut self) -> Result<McpInitializeResult, McpError> {
		if self.connected.load(Ordering::SeqCst) {
			return Err(McpError::ConnectionFailed(
				"Already connected".into(),
			));
		}

		self.spawn_child().await?;
		self.perform_handshake().await
	}

	async fn request<T: DeserializeOwned + Send>(
		&self,
		method: &str,
		params: serde_json::Value,
	) -> Result<T, McpError> {
		self.request_internal(method, params).await
	}

	fn on_notification(&self, method: &str, handler: NotificationHandler) -> SubscriptionHandle {
		let active = Arc::new(AtomicBool::new(true));
		let entry = HandlerEntry {
			active: Arc::clone(&active),
			handler,
		};

		let mut handlers = self
			.notification_handlers
			.write()
			.expect("notification_handlers lock poisoned");
		handlers
			.entry(method.to_string())
			.or_default()
			.push(entry);

		SubscriptionHandle { active }
	}

	async fn close(&mut self) -> Result<(), McpError> {
		self.connected.store(false, Ordering::SeqCst);

		// Reject all pending requests.
		{
			let mut pending = self.pending.lock().await;
			for (_id, req) in pending.drain() {
				if let Some(sender) = req.sender {
					let _ = sender.send(Err(McpError::ConnectionFailed(
						"Connection closed".into(),
					)));
				}
			}
		}

		// Clear notification handlers.
		{
			let mut handlers = self
				.notification_handlers
				.write()
				.expect("notification_handlers lock poisoned");
			handlers.clear();
		}

		// Close stdin to signal the child.
		{
			let mut stdin = self.stdin.lock().await;
			*stdin = None;
		}

		// Kill the child process.
		{
			let mut child_guard = self.child.lock().await;
			if let Some(ref mut child) = *child_guard {
				let _ = child.kill().await;
			}
			*child_guard = None;
		}

		// Abort reader tasks.
		{
			let mut handle = self.reader_handle.lock().await;
			if let Some(h) = handle.take() {
				h.abort();
			}
		}
		{
			let mut handle = self.stderr_handle.lock().await;
			if let Some(h) = handle.take() {
				h.abort();
			}
		}

		Ok(())
	}

	fn is_connected(&self) -> bool {
		self.connected.load(Ordering::SeqCst)
	}
}

// ---------------------------------------------------------------------------
// Internal: dispatch an incoming JSON-RPC message from the server
// ---------------------------------------------------------------------------

async fn dispatch_message(
	msg: &serde_json::Value,
	pending: &Mutex<HashMap<u64, PendingRequest>>,
	notification_handlers: &RwLock<HashMap<String, Vec<HandlerEntry>>>,
) {
	let has_id = msg.get("id").is_some();
	let has_method = msg.get("method").is_some();

	if has_id && !has_method {
		// Response to a pending request.
		let id = match msg.get("id").and_then(|v| v.as_u64()) {
			Some(id) => id,
			None => {
				tracing::warn!("MCP: response with non-u64 id");
				return;
			}
		};

		let mut pending_guard = pending.lock().await;
		if let Some(mut req) = pending_guard.remove(&id) {
			if let Some(error) = msg.get("error") {
				let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
				let message = error
					.get("message")
					.and_then(|m| m.as_str())
					.unwrap_or("Unknown error");
				if let Some(sender) = req.sender.take() {
					let _ = sender.send(Err(McpError::ProtocolError(format!(
						"MCP error {}: {}",
						code, message
					))));
				}
			} else {
				let result = msg
					.get("result")
					.cloned()
					.unwrap_or(serde_json::Value::Null);
				if let Some(sender) = req.sender.take() {
					let _ = sender.send(Ok(result));
				}
			}
		} else {
			tracing::debug!("MCP: response for unknown request id {id}");
		}
	} else if !has_id && has_method {
		// Notification from the server.
		let method = match msg.get("method").and_then(|v| v.as_str()) {
			Some(m) => m,
			None => return,
		};
		let params = msg
			.get("params")
			.cloned()
			.unwrap_or(serde_json::Value::Null);

		let mut handlers_guard = notification_handlers
			.write()
			.expect("notification_handlers lock poisoned");
		if let Some(entries) = handlers_guard.get_mut(method) {
			// Remove inactive entries first (handlers whose SubscriptionHandle was dropped).
			entries.retain(|e| e.active.load(Ordering::SeqCst));

			for entry in entries.iter() {
				(entry.handler)(params.clone());
			}
		}
	} else if has_id && has_method {
		// Server-initiated request — MCP servers don't typically send these
		// to clients in the same way ACP does, but log for debugging.
		tracing::debug!(
			"MCP: ignoring server-initiated request (method={}, id={})",
			msg.get("method").and_then(|v| v.as_str()).unwrap_or("<unknown>"),
			msg.get("id").and_then(|v| v.as_u64()).unwrap_or(0)
		);
	} else {
		tracing::debug!("MCP: ignoring message with no id and no method");
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -------------------------------------------------------------------
	// StdioTransportConfig defaults
	// -------------------------------------------------------------------

	#[test]
	fn test_stdio_config_defaults() {
		let config = StdioTransportConfig::default();
		assert!(config.command.is_empty());
		assert!(config.args.is_empty());
		assert!(config.cwd.is_none());
		assert!(config.env.is_empty());
		assert_eq!(config.timeout_ms, 60_000);
		assert_eq!(config.client_name, "simse");
		assert_eq!(config.client_version, "1.0.0");
	}

	#[test]
	fn test_stdio_config_custom() {
		let config = StdioTransportConfig {
			command: "node".into(),
			args: vec!["server.js".into()],
			cwd: Some("/tmp".into()),
			env: {
				let mut m = HashMap::new();
				m.insert("NODE_ENV".into(), "test".into());
				m
			},
			timeout_ms: 30_000,
			client_name: "test-client".into(),
			client_version: "2.0.0".into(),
		};
		assert_eq!(config.command, "node");
		assert_eq!(config.args, vec!["server.js"]);
		assert_eq!(config.cwd.as_deref(), Some("/tmp"));
		assert_eq!(config.env.get("NODE_ENV").unwrap(), "test");
		assert_eq!(config.timeout_ms, 30_000);
		assert_eq!(config.client_name, "test-client");
		assert_eq!(config.client_version, "2.0.0");
	}

	// -------------------------------------------------------------------
	// SubscriptionHandle
	// -------------------------------------------------------------------

	#[test]
	fn test_subscription_handle_deactivation_on_drop() {
		let active = Arc::new(AtomicBool::new(true));
		let handle = SubscriptionHandle {
			active: Arc::clone(&active),
		};

		assert!(active.load(Ordering::SeqCst));
		assert!(handle.is_active());

		drop(handle);

		assert!(!active.load(Ordering::SeqCst));
	}

	#[test]
	fn test_subscription_handle_explicit_unsubscribe() {
		let active = Arc::new(AtomicBool::new(true));
		let handle = SubscriptionHandle {
			active: Arc::clone(&active),
		};

		assert!(handle.is_active());
		handle.unsubscribe();
		assert!(!handle.is_active());
		assert!(!active.load(Ordering::SeqCst));
	}

	// -------------------------------------------------------------------
	// NDJSON dispatch tests
	// -------------------------------------------------------------------

	/// Helper: create standard test fixtures for dispatch_message.
	struct DispatchFixtures {
		pending: Arc<Mutex<HashMap<u64, PendingRequest>>>,
		handlers: Arc<RwLock<HashMap<String, Vec<HandlerEntry>>>>,
	}

	impl DispatchFixtures {
		fn new() -> Self {
			Self {
				pending: Arc::new(Mutex::new(HashMap::new())),
				handlers: Arc::new(RwLock::new(HashMap::new())),
			}
		}
	}

	/// Helper: dispatch a message using the given fixtures.
	async fn dispatch(msg: &str, f: &DispatchFixtures) {
		let parsed: serde_json::Value = serde_json::from_str(msg).unwrap();
		dispatch_message(&parsed, &f.pending, &f.handlers).await;
	}

	#[tokio::test]
	async fn test_dispatch_response_success() {
		let f = DispatchFixtures::new();

		let (tx, rx) = oneshot::channel();
		{
			let mut p = f.pending.lock().await;
			p.insert(
				1,
				PendingRequest {
					sender: Some(tx),
				},
			);
		}

		dispatch(
			r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
			&f,
		)
		.await;

		let result = rx.await.unwrap().unwrap();
		assert_eq!(result, serde_json::json!({"ok": true}));

		// Pending should be empty now.
		assert!(f.pending.lock().await.is_empty());
	}

	#[tokio::test]
	async fn test_dispatch_response_error() {
		let f = DispatchFixtures::new();

		let (tx, rx) = oneshot::channel();
		{
			let mut p = f.pending.lock().await;
			p.insert(
				2,
				PendingRequest {
					sender: Some(tx),
				},
			);
		}

		dispatch(
			r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"bad request"}}"#,
			&f,
		)
		.await;

		let result = rx.await.unwrap();
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("bad request"));
	}

	#[tokio::test]
	async fn test_dispatch_response_unknown_id() {
		let f = DispatchFixtures::new();

		// Should not panic — just logs a debug message.
		dispatch(
			r#"{"jsonrpc":"2.0","id":999,"result":{"ok":true}}"#,
			&f,
		)
		.await;

		assert!(f.pending.lock().await.is_empty());
	}

	#[tokio::test]
	async fn test_dispatch_notification() {
		let f = DispatchFixtures::new();

		let received = Arc::new(std::sync::Mutex::new(Vec::new()));
		let received_clone = Arc::clone(&received);

		// Register handler.
		{
			let active = Arc::new(AtomicBool::new(true));
			let entry = HandlerEntry {
				active: Arc::clone(&active),
				handler: Box::new(move |params| {
					let mut guard = received_clone.lock().unwrap();
					guard.push(params);
				}),
			};
			let mut handlers = f.handlers.write().unwrap();
			handlers
				.entry("notifications/tools/list_changed".into())
				.or_default()
				.push(entry);
		}

		dispatch(
			r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed","params":{"changed":true}}"#,
			&f,
		)
		.await;

		let guard = received.lock().unwrap();
		assert_eq!(guard.len(), 1);
		assert_eq!(guard[0], serde_json::json!({"changed": true}));
	}

	#[tokio::test]
	async fn test_dispatch_notification_no_params() {
		let f = DispatchFixtures::new();

		let received = Arc::new(std::sync::Mutex::new(Vec::new()));
		let received_clone = Arc::clone(&received);

		{
			let active = Arc::new(AtomicBool::new(true));
			let entry = HandlerEntry {
				active: Arc::clone(&active),
				handler: Box::new(move |params| {
					let mut guard = received_clone.lock().unwrap();
					guard.push(params);
				}),
			};
			let mut handlers = f.handlers.write().unwrap();
			handlers
				.entry("notifications/initialized".into())
				.or_default()
				.push(entry);
		}

		dispatch(
			r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
			&f,
		)
		.await;

		let guard = received.lock().unwrap();
		assert_eq!(guard.len(), 1);
		assert_eq!(guard[0], serde_json::Value::Null);
	}

	#[tokio::test]
	async fn test_dispatch_inactive_handler_cleaned_up() {
		let f = DispatchFixtures::new();

		let call_count = Arc::new(AtomicU64::new(0));
		let call_count_clone = Arc::clone(&call_count);

		// Register a handler that we will immediately deactivate.
		let active = Arc::new(AtomicBool::new(true));
		{
			let entry = HandlerEntry {
				active: Arc::clone(&active),
				handler: Box::new(move |_params| {
					call_count_clone.fetch_add(1, Ordering::SeqCst);
				}),
			};
			let mut handlers = f.handlers.write().unwrap();
			handlers
				.entry("test/method".into())
				.or_default()
				.push(entry);
		}

		// Deactivate the handler (simulating drop of SubscriptionHandle).
		active.store(false, Ordering::SeqCst);

		dispatch(
			r#"{"jsonrpc":"2.0","method":"test/method","params":{}}"#,
			&f,
		)
		.await;

		// Handler should not have been called.
		assert_eq!(call_count.load(Ordering::SeqCst), 0);

		// Entry should have been cleaned up.
		let handlers = f.handlers.read().unwrap();
		let entries = handlers.get("test/method").unwrap();
		assert!(entries.is_empty());
	}

	#[tokio::test]
	async fn test_dispatch_invalid_json_ignored() {
		let f = DispatchFixtures::new();

		// Message with no id and no method — should be silently ignored.
		dispatch(r#"{"jsonrpc":"2.0"}"#, &f).await;

		assert!(f.pending.lock().await.is_empty());
	}

	// -------------------------------------------------------------------
	// StdioTransport construction
	// -------------------------------------------------------------------

	#[test]
	fn test_stdio_transport_new_not_connected() {
		let transport = StdioTransport::new(StdioTransportConfig::default());
		assert!(!transport.is_connected());
	}

	#[test]
	fn test_stdio_transport_preserves_timeout() {
		let config = StdioTransportConfig {
			timeout_ms: 30_000,
			..Default::default()
		};
		let transport = StdioTransport::new(config);
		assert_eq!(transport.default_timeout_ms, 30_000);
	}

	// -------------------------------------------------------------------
	// MCP handshake params serialization
	// -------------------------------------------------------------------

	#[test]
	fn test_initialize_params_serialization() {
		let params = McpInitializeParams {
			protocol_version: MCP_PROTOCOL_VERSION.into(),
			capabilities: ClientCapabilities {
				roots: Some(RootCapabilities {
					list_changed: Some(true),
				}),
				sampling: None,
			},
			client_info: ImplementationInfo {
				name: "simse".into(),
				version: "1.0.0".into(),
			},
		};

		let json = serde_json::to_value(&params).unwrap();
		assert_eq!(json["protocolVersion"], MCP_PROTOCOL_VERSION);
		assert_eq!(json["clientInfo"]["name"], "simse");
		assert_eq!(json["capabilities"]["roots"]["listChanged"], true);
	}
}
