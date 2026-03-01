// ---------------------------------------------------------------------------
// ACP Connection — manages a child process running an ACP-compatible agent
// ---------------------------------------------------------------------------
//
// Responsibilities:
//   - Spawning the agent process with stdio pipes
//   - NDJSON buffer parsing from stdout
//   - Pending request tracking with timeouts
//   - Sending JSON-RPC requests and receiving responses
//   - Routing notifications to registered handlers
//   - Health checking (is process alive?)
//   - Cleanup on close
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

use crate::error::AcpError;
use crate::permission::resolve_permission;
use crate::protocol::{
	ClientInfo, InitializeParams, InitializeResult, JsonRpcNotification, JsonRpcResponse,
	PermissionPolicy, PermissionRequestParams, PermissionResult,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Handler invoked when a notification arrives from the agent process.
pub type NotificationHandler = Box<dyn Fn(serde_json::Value) + Send + Sync>;

/// A pending JSON-RPC request awaiting a response.
#[allow(dead_code)]
struct PendingRequest {
	sender: Option<oneshot::Sender<Result<serde_json::Value, AcpError>>>,
	deadline: tokio::time::Instant,
	method: String,
}

/// A handle that, when dropped, unregisters the associated notification handler.
///
/// Call [`SubscriptionHandle::unsubscribe`] explicitly or let the handle go out
/// of scope to remove the handler.
pub struct SubscriptionHandle {
	active: Arc<AtomicBool>,
}

impl SubscriptionHandle {
	/// Explicitly unsubscribe (same effect as dropping the handle).
	pub fn unsubscribe(&self) {
		self.active.store(false, Ordering::SeqCst);
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

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for spawning an ACP agent connection.
pub struct ConnectionConfig {
	/// The command to run (e.g. path to the ACP agent binary).
	pub command: String,
	/// Arguments to pass to the command.
	pub args: Vec<String>,
	/// Working directory for the child process.
	pub cwd: Option<String>,
	/// Additional environment variables for the child process.
	pub env: HashMap<String, String>,
	/// Default timeout for JSON-RPC requests (milliseconds). Default: 60 000.
	pub timeout_ms: u64,
	/// Timeout for the `initialize` handshake (milliseconds). Default: 30 000.
	pub init_timeout_ms: u64,
	/// Client name sent during initialization. Default: `"simse"`.
	pub client_name: String,
	/// Client version sent during initialization. Default: `"1.0.0"`.
	pub client_version: String,
}

impl Default for ConnectionConfig {
	fn default() -> Self {
		Self {
			command: String::new(),
			args: Vec::new(),
			cwd: None,
			env: HashMap::new(),
			timeout_ms: 60_000,
			init_timeout_ms: 30_000,
			client_name: "simse".into(),
			client_version: "1.0.0".into(),
		}
	}
}

// ---------------------------------------------------------------------------
// AcpConnection
// ---------------------------------------------------------------------------

/// Manages a single ACP agent child process over JSON-RPC 2.0 / NDJSON stdio.
pub struct AcpConnection {
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
	/// The result of the `initialize` handshake, if completed.
	server_info: Mutex<Option<InitializeResult>>,
	/// Current permission policy for tool-use requests.
	permission_policy: Arc<Mutex<PermissionPolicy>>,
	/// Pending permission requests awaiting external resolution (Prompt policy).
	pending_permissions: Arc<Mutex<HashMap<u64, PermissionRequestParams>>>,
	/// Default timeout for requests (ms).
	default_timeout_ms: u64,
	/// Timeout for the initialize handshake (ms).
	init_timeout_ms: u64,
	/// Client name (for initialize).
	client_name: String,
	/// Client version (for initialize).
	client_version: String,
	/// Handle to the stdout reader task so we can abort it on close.
	reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
	/// Handle to the stderr reader task.
	stderr_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl AcpConnection {
	// -------------------------------------------------------------------
	// Construction
	// -------------------------------------------------------------------

	/// Spawn the agent child process. This does **not** send the
	/// `initialize` handshake — call [`initialize`](Self::initialize)
	/// separately.
	pub async fn new(config: ConnectionConfig) -> Result<Self, AcpError> {
		let mut cmd = Command::new(&config.command);
		cmd.args(&config.args)
			.stdin(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.kill_on_drop(true);

		if let Some(ref cwd) = config.cwd {
			cmd.current_dir(cwd);
		}

		for (k, v) in &config.env {
			cmd.env(k, v);
		}

		// Strip CLAUDECODE env var so child ACP processes aren't blocked
		// by Claude Code's nested-session detection.
		cmd.env_remove("CLAUDECODE");

		let mut child = cmd.spawn().map_err(|e| {
			AcpError::ConnectionFailed(format!(
				"Failed to spawn '{}': {}",
				config.command, e
			))
		})?;

		let stdin = child.stdin.take();
		let stdout = child.stdout.take();
		let stderr = child.stderr.take();

		let pending: Arc<Mutex<HashMap<u64, PendingRequest>>> =
			Arc::new(Mutex::new(HashMap::new()));
		let notification_handlers: Arc<RwLock<HashMap<String, Vec<HandlerEntry>>>> =
			Arc::new(RwLock::new(HashMap::new()));
		let connected = Arc::new(AtomicBool::new(true));
		let permission_policy = Arc::new(Mutex::new(PermissionPolicy::default()));
		let pending_permissions: Arc<Mutex<HashMap<u64, PermissionRequestParams>>> =
			Arc::new(Mutex::new(HashMap::new()));

		// We create the connection first, then spawn the reader tasks that
		// share references into it via Arc clones of pending/handlers.
		let stdin = Arc::new(Mutex::new(stdin));

		let conn = Self {
			child: Mutex::new(Some(child)),
			stdin: Arc::clone(&stdin),
			next_id: AtomicU64::new(1),
			pending: Arc::clone(&pending),
			notification_handlers: Arc::clone(&notification_handlers),
			connected: Arc::clone(&connected),
			server_info: Mutex::new(None),
			permission_policy: Arc::clone(&permission_policy),
			pending_permissions: Arc::clone(&pending_permissions),
			default_timeout_ms: config.timeout_ms,
			init_timeout_ms: config.init_timeout_ms,
			client_name: config.client_name,
			client_version: config.client_version,
			reader_handle: Mutex::new(None),
			stderr_handle: Mutex::new(None),
		};

		// Spawn stdout reader task.
		if let Some(stdout) = stdout {
			let pending_clone = Arc::clone(&pending);
			let handlers_clone = Arc::clone(&notification_handlers);
			let connected_clone = Arc::clone(&connected);
			let pending_for_exit = Arc::clone(&pending);
			let stdin_clone = Arc::clone(&stdin);
			let policy_clone = Arc::clone(&permission_policy);
			let perms_clone = Arc::clone(&pending_permissions);
			let handle = tokio::spawn(async move {
				let mut reader = BufReader::new(stdout);
				let mut line_buf = String::new();

				loop {
					line_buf.clear();
					match reader.read_line(&mut line_buf).await {
						Ok(0) => {
							// EOF — child process closed stdout.
							tracing::debug!("ACP stdout EOF");
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
									tracing::warn!("ACP: invalid JSON on stdout: {e}");
									continue;
								}
							};
							Self::dispatch_message(
								&parsed,
								&pending_clone,
								&handlers_clone,
								&stdin_clone,
								&policy_clone,
								&perms_clone,
							)
							.await;
						}
						Err(e) => {
							tracing::warn!("ACP stdout read error: {e}");
							break;
						}
					}
				}

				// Mark connection as dead so is_healthy() / is_connected() see it.
				connected_clone.store(false, Ordering::SeqCst);

				// Child stdout closed — reject all pending requests.
				let mut pending_guard = pending_for_exit.lock().await;
				for (_id, req) in pending_guard.drain() {
					if let Some(sender) = req.sender {
						let _ = sender.send(Err(AcpError::ConnectionFailed(
							"ACP server closed stdout".into(),
						)));
					}
				}
			});
			*conn.reader_handle.lock().await = Some(handle);
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
								tracing::warn!("ACP stderr: {text}");
							}
						}
						Err(e) => {
							tracing::warn!("ACP stderr read error: {e}");
							break;
						}
					}
				}
			});
			*conn.stderr_handle.lock().await = Some(handle);
		}

		Ok(conn)
	}

	// -------------------------------------------------------------------
	// Initialize handshake
	// -------------------------------------------------------------------

	/// Send the `initialize` JSON-RPC request to the agent.
	///
	/// Returns the agent's [`InitializeResult`]. Subsequent calls return
	/// the cached result without re-sending the request.
	pub async fn initialize(&self) -> Result<InitializeResult, AcpError> {
		{
			let cached = self.server_info.lock().await;
			if let Some(ref info) = *cached {
				return Ok(info.clone());
			}
		}

		let params = InitializeParams {
			protocol_version: 1,
			client_info: ClientInfo {
				name: self.client_name.clone(),
				version: self.client_version.clone(),
			},
			capabilities: Some(serde_json::json!({})),
		};

		let result: InitializeResult =
			self.request("initialize", serde_json::to_value(&params).map_err(|e| {
				AcpError::Serialization(e.to_string())
			})?, self.init_timeout_ms).await?;

		*self.server_info.lock().await = Some(result.clone());
		Ok(result)
	}

	// -------------------------------------------------------------------
	// Send request
	// -------------------------------------------------------------------

	/// Send a JSON-RPC request and wait for the matching response.
	///
	/// The request will time out after `timeout_ms` milliseconds, or the
	/// connection's default timeout if `timeout_ms` is 0.
	pub async fn request<T: DeserializeOwned>(
		&self,
		method: &str,
		params: serde_json::Value,
		timeout_ms: u64,
	) -> Result<T, AcpError> {
		if !self.connected.load(Ordering::SeqCst) {
			return Err(AcpError::ConnectionFailed(
				"ACP connection is not open".into(),
			));
		}

		let effective_timeout = if timeout_ms > 0 {
			timeout_ms
		} else {
			self.default_timeout_ms
		};

		let id = self.next_id.fetch_add(1, Ordering::SeqCst);
		let deadline =
			tokio::time::Instant::now() + std::time::Duration::from_millis(effective_timeout);

		let (tx, rx) = oneshot::channel();

		// Register pending request.
		{
			let mut pending = self.pending.lock().await;
			pending.insert(
				id,
				PendingRequest {
					sender: Some(tx),
					deadline,
					method: method.to_string(),
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
					AcpError::ProtocolError(format!(
						"Failed to deserialize response for '{}': {}",
						method, e
					))
				})
			}
			Ok(Err(_)) => {
				// oneshot sender dropped — connection likely closed.
				Err(AcpError::ConnectionFailed(
					"Response channel closed".into(),
				))
			}
			Err(_) => {
				// Timeout — remove pending entry.
				let mut pending = self.pending.lock().await;
				pending.remove(&id);
				Err(AcpError::Timeout {
					method: method.to_string(),
					timeout_ms: effective_timeout,
				})
			}
		}
	}

	// -------------------------------------------------------------------
	// Send notification (fire-and-forget)
	// -------------------------------------------------------------------

	/// Send a JSON-RPC notification to the agent (no response expected).
	pub fn notify(&self, method: &str, params: serde_json::Value) {
		let notification = JsonRpcNotification::new(method, Some(params));
		let json = match serde_json::to_value(&notification) {
			Ok(v) => v,
			Err(e) => {
				tracing::warn!("ACP: failed to serialize notification: {e}");
				return;
			}
		};
		// Spawn a task to do the async write without blocking the caller.
		let stdin = Arc::clone(&self.stdin);
		tokio::spawn(async move {
			let mut guard = stdin.lock().await;
			if let Some(writer) = guard.as_mut() {
				let mut data = serde_json::to_vec(&json).unwrap_or_default();
				data.push(b'\n');
				if let Err(e) = AsyncWriteExt::write_all(writer, &data).await {
					tracing::warn!("ACP: failed to write notification: {e}");
				}
				let _ = AsyncWriteExt::flush(writer).await;
			}
		});
	}

	// -------------------------------------------------------------------
	// Notification subscription
	// -------------------------------------------------------------------

	/// Register a handler for incoming notifications with the given method
	/// name. Returns a [`SubscriptionHandle`] that removes the handler
	/// when dropped.
	pub fn on_notification(
		&self,
		method: &str,
		handler: NotificationHandler,
	) -> SubscriptionHandle {
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

	// -------------------------------------------------------------------
	// Close / cleanup
	// -------------------------------------------------------------------

	/// Close the connection: kill the child process and reject all pending
	/// requests.
	pub async fn close(&self) {
		self.connected.store(false, Ordering::SeqCst);

		// Reject all pending requests.
		{
			let mut pending = self.pending.lock().await;
			for (_id, req) in pending.drain() {
				if let Some(sender) = req.sender {
					let _ = sender.send(Err(AcpError::ConnectionFailed(
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
	}

	// -------------------------------------------------------------------
	// Health check
	// -------------------------------------------------------------------

	/// Returns `true` if the connection is marked connected and the child
	/// process has not exited.
	///
	/// This is a synchronous check — the reader task sets `connected` to
	/// `false` on stdout EOF, so no Mutex lock is needed here.
	pub fn is_healthy(&self) -> bool {
		self.connected.load(Ordering::SeqCst)
	}

	// -------------------------------------------------------------------
	// Accessors
	// -------------------------------------------------------------------

	/// Whether the connection is currently considered open.
	pub fn is_connected(&self) -> bool {
		self.connected.load(Ordering::SeqCst)
	}

	/// The result of the `initialize` handshake, if completed.
	pub async fn server_info(&self) -> Option<InitializeResult> {
		self.server_info.lock().await.clone()
	}

	/// Get the current permission policy.
	pub async fn permission_policy(&self) -> PermissionPolicy {
		self.permission_policy.lock().await.clone()
	}

	/// Set the permission policy.
	pub async fn set_permission_policy(&self, policy: PermissionPolicy) {
		*self.permission_policy.lock().await = policy;
	}

	/// Return all pending permission requests awaiting external resolution.
	///
	/// Only populated when the permission policy is `Prompt`. Each entry
	/// is the JSON-RPC request ID paired with the permission request params.
	pub async fn pending_permission_requests(
		&self,
	) -> Vec<(u64, PermissionRequestParams)> {
		let guard = self.pending_permissions.lock().await;
		guard.iter().map(|(id, p)| (*id, p.clone())).collect()
	}

	/// Respond to a pending permission request (Prompt policy).
	///
	/// The `request_id` must match an outstanding permission request. The
	/// `result` is sent as the JSON-RPC response to the agent process.
	pub async fn respond_to_permission(
		&self,
		request_id: u64,
		result: PermissionResult,
	) -> Result<(), AcpError> {
		// Remove from pending set.
		{
			let mut guard = self.pending_permissions.lock().await;
			if guard.remove(&request_id).is_none() {
				return Err(AcpError::ProtocolError(format!(
					"No pending permission request with id {request_id}"
				)));
			}
		}

		// Build and send the JSON-RPC response.
		let response = JsonRpcResponse::success(
			request_id,
			serde_json::to_value(&result)
				.map_err(|e| AcpError::Serialization(e.to_string()))?,
		);
		let value = serde_json::to_value(&response)
			.map_err(|e| AcpError::Serialization(e.to_string()))?;
		self.write_line(&value).await
	}

	// -------------------------------------------------------------------
	// Internal: write to stdin
	// -------------------------------------------------------------------

	async fn write_line(&self, value: &serde_json::Value) -> Result<(), AcpError> {
		let mut stdin_guard = self.stdin.lock().await;
		let writer = stdin_guard
			.as_mut()
			.ok_or_else(|| AcpError::ConnectionFailed("stdin not available".into()))?;

		let mut data = serde_json::to_vec(value)
			.map_err(|e| AcpError::Serialization(e.to_string()))?;
		data.push(b'\n');

		writer
			.write_all(&data)
			.await
			.map_err(|e| AcpError::Io(e))?;
		writer.flush().await.map_err(|e| AcpError::Io(e))?;

		Ok(())
	}

	// -------------------------------------------------------------------
	// Internal: dispatch an incoming JSON-RPC message
	// -------------------------------------------------------------------

	async fn dispatch_message(
		msg: &serde_json::Value,
		pending: &Mutex<HashMap<u64, PendingRequest>>,
		notification_handlers: &RwLock<HashMap<String, Vec<HandlerEntry>>>,
		stdin: &Mutex<Option<ChildStdin>>,
		permission_policy: &Mutex<PermissionPolicy>,
		pending_permissions: &Mutex<HashMap<u64, PermissionRequestParams>>,
	) {
		let has_id = msg.get("id").is_some();
		let has_method = msg.get("method").is_some();

		if has_id && !has_method {
			// Response to a pending request.
			let id = match msg.get("id").and_then(|v| v.as_u64()) {
				Some(id) => id,
				None => {
					tracing::warn!("ACP: response with non-u64 id");
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
						let _ = sender.send(Err(AcpError::ProtocolError(format!(
							"ACP error {}: {}",
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
				tracing::debug!("ACP: response for unknown request id {id}");
			}
		} else if has_id && has_method {
			// Server-initiated request (e.g. session/request_permission).
			let id = match msg.get("id").and_then(|v| v.as_u64()) {
				Some(id) => id,
				None => {
					tracing::warn!("ACP: server request with non-u64 id");
					return;
				}
			};
			let method = msg
				.get("method")
				.and_then(|v| v.as_str())
				.unwrap_or("<unknown>");

			if method == "session/request_permission" {
				Self::handle_permission_request(
					id,
					msg.get("params").cloned().unwrap_or(serde_json::Value::Null),
					stdin,
					permission_policy,
					pending_permissions,
				)
				.await;
			} else {
				tracing::debug!("ACP: unhandled server request '{method}' (id={id})");
			}
		} else if !has_id && has_method {
			// Notification from the agent.
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
		} else {
			tracing::debug!("ACP: ignoring message with no id and no method");
		}
	}

	// -------------------------------------------------------------------
	// Internal: handle an incoming permission request
	// -------------------------------------------------------------------

	async fn handle_permission_request(
		request_id: u64,
		raw_params: serde_json::Value,
		stdin: &Mutex<Option<ChildStdin>>,
		permission_policy: &Mutex<PermissionPolicy>,
		pending_permissions: &Mutex<HashMap<u64, PermissionRequestParams>>,
	) {
		let params: PermissionRequestParams = match serde_json::from_value(raw_params) {
			Ok(p) => p,
			Err(e) => {
				tracing::warn!("ACP: failed to parse permission request params: {e}");
				// Respond with a protocol error so the agent doesn't hang.
				let response = JsonRpcResponse::error(
					request_id,
					crate::protocol::INVALID_PARAMS,
					format!("Invalid permission request params: {e}"),
				);
				Self::send_json_rpc_response(stdin, &response).await;
				return;
			}
		};

		let policy = permission_policy.lock().await.clone();

		match resolve_permission(policy, &params) {
			Some(result) => {
				// AutoApprove or Deny — send the response immediately.
				let result_value = match serde_json::to_value(&result) {
					Ok(v) => v,
					Err(e) => {
						tracing::warn!("ACP: failed to serialize permission result: {e}");
						return;
					}
				};
				let response = JsonRpcResponse::success(request_id, result_value);
				Self::send_json_rpc_response(stdin, &response).await;
			}
			None => {
				// Prompt — store for external resolution.
				tracing::debug!(
					"ACP: permission request {request_id} queued for external resolution"
				);
				let mut guard = pending_permissions.lock().await;
				guard.insert(request_id, params);
			}
		}
	}

	// -------------------------------------------------------------------
	// Internal: send a JSON-RPC response to the child process
	// -------------------------------------------------------------------

	async fn send_json_rpc_response(
		stdin: &Mutex<Option<ChildStdin>>,
		response: &JsonRpcResponse,
	) {
		let json = match serde_json::to_value(response) {
			Ok(v) => v,
			Err(e) => {
				tracing::warn!("ACP: failed to serialize response: {e}");
				return;
			}
		};

		let mut stdin_guard = stdin.lock().await;
		if let Some(writer) = stdin_guard.as_mut() {
			let mut data = match serde_json::to_vec(&json) {
				Ok(d) => d,
				Err(e) => {
					tracing::warn!("ACP: failed to encode response: {e}");
					return;
				}
			};
			data.push(b'\n');
			if let Err(e) = writer.write_all(&data).await {
				tracing::warn!("ACP: failed to write permission response: {e}");
			}
			let _ = writer.flush().await;
		}
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -------------------------------------------------------------------
	// NDJSON parsing tests
	// -------------------------------------------------------------------

	/// Helper: create standard test fixtures for dispatch_message.
	struct DispatchFixtures {
		pending: Arc<Mutex<HashMap<u64, PendingRequest>>>,
		handlers: Arc<RwLock<HashMap<String, Vec<HandlerEntry>>>>,
		stdin: Arc<Mutex<Option<ChildStdin>>>,
		policy: Arc<Mutex<PermissionPolicy>>,
		perms: Arc<Mutex<HashMap<u64, PermissionRequestParams>>>,
	}

	impl DispatchFixtures {
		fn new() -> Self {
			Self {
				pending: Arc::new(Mutex::new(HashMap::new())),
				handlers: Arc::new(RwLock::new(HashMap::new())),
				stdin: Arc::new(Mutex::new(None)),
				policy: Arc::new(Mutex::new(PermissionPolicy::default())),
				perms: Arc::new(Mutex::new(HashMap::new())),
			}
		}
	}

	/// Helper: dispatch a message using the given fixtures.
	async fn dispatch(msg: &str, f: &DispatchFixtures) {
		let parsed: serde_json::Value = serde_json::from_str(msg).unwrap();
		AcpConnection::dispatch_message(
			&parsed,
			&f.pending,
			&f.handlers,
			&f.stdin,
			&f.policy,
			&f.perms,
		)
		.await;
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
					deadline: tokio::time::Instant::now()
						+ std::time::Duration::from_secs(10),
					method: "test".into(),
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
					deadline: tokio::time::Instant::now()
						+ std::time::Duration::from_secs(10),
					method: "test".into(),
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
		let msg = err.to_string();
		assert!(
			msg.contains("-32600") && msg.contains("bad request"),
			"unexpected error: {msg}"
		);
	}

	#[tokio::test]
	async fn test_dispatch_notification_routes_to_handler() {
		let f = DispatchFixtures::new();

		let received = Arc::new(std::sync::Mutex::new(Vec::<serde_json::Value>::new()));
		let received_clone = Arc::clone(&received);

		{
			let mut h = f.handlers.write().unwrap();
			h.entry("session/update".to_string())
				.or_default()
				.push(HandlerEntry {
					active: Arc::new(AtomicBool::new(true)),
					handler: Box::new(move |val| {
						received_clone.lock().unwrap().push(val);
					}),
				});
		}

		dispatch(
			r#"{"jsonrpc":"2.0","method":"session/update","params":{"delta":"hello"}}"#,
			&f,
		)
		.await;

		let received_guard = received.lock().unwrap();
		assert_eq!(received_guard.len(), 1);
		assert_eq!(received_guard[0]["delta"], "hello");
	}

	#[tokio::test]
	async fn test_dispatch_notification_inactive_handler_removed() {
		let f = DispatchFixtures::new();

		let call_count = Arc::new(AtomicU64::new(0));
		let call_count_clone = Arc::clone(&call_count);

		let active_flag = Arc::new(AtomicBool::new(true));
		let handle = SubscriptionHandle {
			active: Arc::clone(&active_flag),
		};

		{
			let mut h = f.handlers.write().unwrap();
			h.entry("test/notify".to_string())
				.or_default()
				.push(HandlerEntry {
					active: active_flag,
					handler: Box::new(move |_val| {
						call_count_clone.fetch_add(1, Ordering::SeqCst);
					}),
				});
		}

		// First dispatch — handler is active.
		dispatch(
			r#"{"jsonrpc":"2.0","method":"test/notify","params":{}}"#,
			&f,
		)
		.await;
		assert_eq!(call_count.load(Ordering::SeqCst), 1);

		// Unsubscribe.
		handle.unsubscribe();

		// Second dispatch — handler should be pruned.
		dispatch(
			r#"{"jsonrpc":"2.0","method":"test/notify","params":{}}"#,
			&f,
		)
		.await;
		assert_eq!(call_count.load(Ordering::SeqCst), 1); // not incremented
	}

	#[tokio::test]
	async fn test_dispatch_unknown_response_id_is_ignored() {
		let f = DispatchFixtures::new();

		// No pending request with id=99 — should not panic.
		dispatch(
			r#"{"jsonrpc":"2.0","id":99,"result":"orphan"}"#,
			&f,
		)
		.await;
	}

	#[tokio::test]
	async fn test_dispatch_empty_and_invalid_lines_skipped() {
		let f = DispatchFixtures::new();

		// Empty line — would be skipped by the reader loop (not reaching dispatch).
		// Invalid JSON — the reader loop skips it; dispatch only receives valid JSON.
		// Message with neither id nor method — should be silently ignored.
		dispatch(
			r#"{"jsonrpc":"2.0"}"#,
			&f,
		)
		.await;
		// No panic or error — just ignored.
	}

	// -----------------------------------------------------------------------
	// Permission dispatch tests
	// -----------------------------------------------------------------------

	/// Build a permission request JSON-RPC message.
	fn make_permission_request(id: u64, options_json: &str) -> String {
		format!(
			r#"{{"jsonrpc":"2.0","id":{id},"method":"session/request_permission","params":{{"title":"Run bash","options":{options_json}}}}}"#
		)
	}

	#[tokio::test]
	async fn test_dispatch_permission_auto_approve() {
		let f = DispatchFixtures::new();
		*f.policy.lock().await = PermissionPolicy::AutoApprove;

		let msg = make_permission_request(
			10,
			r#"[{"optionId":"rej","kind":"reject_once"},{"optionId":"once","kind":"allow_once"},{"optionId":"always","kind":"allow_always"}]"#,
		);
		dispatch(&msg, &f).await;

		// Should NOT be in pending_permissions (auto-resolved).
		assert!(f.perms.lock().await.is_empty());
	}

	#[tokio::test]
	async fn test_dispatch_permission_deny() {
		let f = DispatchFixtures::new();
		*f.policy.lock().await = PermissionPolicy::Deny;

		let msg = make_permission_request(
			11,
			r#"[{"optionId":"once","kind":"allow_once"},{"optionId":"rej","kind":"reject_once"}]"#,
		);
		dispatch(&msg, &f).await;

		// Should NOT be in pending_permissions (auto-resolved).
		assert!(f.perms.lock().await.is_empty());
	}

	#[tokio::test]
	async fn test_dispatch_permission_prompt_queues_request() {
		let f = DispatchFixtures::new();
		*f.policy.lock().await = PermissionPolicy::Prompt;

		let msg = make_permission_request(
			12,
			r#"[{"optionId":"once","kind":"allow_once"},{"optionId":"always","kind":"allow_always"}]"#,
		);
		dispatch(&msg, &f).await;

		// Should be in pending_permissions.
		let guard = f.perms.lock().await;
		assert_eq!(guard.len(), 1);
		assert!(guard.contains_key(&12));
		let params = &guard[&12];
		assert_eq!(params.options.len(), 2);
		assert_eq!(params.title.as_deref(), Some("Run bash"));
	}

	// -----------------------------------------------------------------------
	// NDJSON reader integration tests
	// -----------------------------------------------------------------------

	/// Helper to spawn a reader task matching the real stdout reader's behavior.
	fn spawn_reader_task(
		reader: impl tokio::io::AsyncRead + Unpin + Send + 'static,
		f: &DispatchFixtures,
	) -> tokio::task::JoinHandle<()> {
		let pending_clone = Arc::clone(&f.pending);
		let handlers_clone = Arc::clone(&f.handlers);
		let stdin_clone = Arc::clone(&f.stdin);
		let policy_clone = Arc::clone(&f.policy);
		let perms_clone = Arc::clone(&f.perms);
		tokio::spawn(async move {
			let mut buf_reader = BufReader::new(reader);
			let mut line = String::new();
			loop {
				line.clear();
				match buf_reader.read_line(&mut line).await {
					Ok(0) => break,
					Ok(_) => {
						let trimmed = line.trim();
						if trimmed.is_empty() {
							continue;
						}
						if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
							AcpConnection::dispatch_message(
								&parsed,
								&pending_clone,
								&handlers_clone,
								&stdin_clone,
								&policy_clone,
								&perms_clone,
							)
							.await;
						}
					}
					Err(_) => break,
				}
			}
		})
	}

	#[tokio::test]
	async fn test_ndjson_buffer_parsing_via_reader() {
		// Simulate the NDJSON buffer parsing that the stdout reader does
		// by feeding data through a tokio channel/pipe.
		use tokio::io::AsyncWriteExt;

		let (mut writer, reader) = tokio::io::duplex(4096);
		let f = DispatchFixtures::new();

		let (tx1, rx1) = oneshot::channel();
		let (tx2, rx2) = oneshot::channel();
		{
			let mut p = f.pending.lock().await;
			p.insert(
				1,
				PendingRequest {
					sender: Some(tx1),
					deadline: tokio::time::Instant::now()
						+ std::time::Duration::from_secs(10),
					method: "test1".into(),
				},
			);
			p.insert(
				2,
				PendingRequest {
					sender: Some(tx2),
					deadline: tokio::time::Instant::now()
						+ std::time::Duration::from_secs(10),
					method: "test2".into(),
				},
			);
		}

		let reader_task = spawn_reader_task(reader, &f);

		// Write two complete responses in one chunk (multiple lines in one read).
		let data = concat!(
			r#"{"jsonrpc":"2.0","id":1,"result":"first"}"#,
			"\n",
			r#"{"jsonrpc":"2.0","id":2,"result":"second"}"#,
			"\n",
		);
		writer.write_all(data.as_bytes()).await.unwrap();
		writer.flush().await.unwrap();

		// Give the reader a moment to process.
		tokio::time::sleep(std::time::Duration::from_millis(50)).await;

		let r1 = rx1.await.unwrap().unwrap();
		assert_eq!(r1, serde_json::json!("first"));

		let r2 = rx2.await.unwrap().unwrap();
		assert_eq!(r2, serde_json::json!("second"));

		// Close writer to end the reader task.
		drop(writer);
		let _ = reader_task.await;
	}

	#[tokio::test]
	async fn test_ndjson_partial_line_buffered() {
		// Verify that BufReader::read_line correctly handles partial lines.
		use tokio::io::AsyncWriteExt;

		let (mut writer, reader) = tokio::io::duplex(4096);
		let f = DispatchFixtures::new();

		let (tx, rx) = oneshot::channel();
		{
			let mut p = f.pending.lock().await;
			p.insert(
				1,
				PendingRequest {
					sender: Some(tx),
					deadline: tokio::time::Instant::now()
						+ std::time::Duration::from_secs(10),
					method: "partial".into(),
				},
			);
		}

		let reader_task = spawn_reader_task(reader, &f);

		// Write a partial line first.
		writer
			.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,")
			.await
			.unwrap();
		writer.flush().await.unwrap();

		// Small delay to simulate network chunking.
		tokio::time::sleep(std::time::Duration::from_millis(20)).await;

		// Complete the line.
		writer
			.write_all(b"\"result\":\"assembled\"}\n")
			.await
			.unwrap();
		writer.flush().await.unwrap();

		tokio::time::sleep(std::time::Duration::from_millis(50)).await;

		let result = rx.await.unwrap().unwrap();
		assert_eq!(result, serde_json::json!("assembled"));

		drop(writer);
		let _ = reader_task.await;
	}

	#[tokio::test]
	async fn test_ndjson_empty_lines_skipped() {
		use tokio::io::AsyncWriteExt;

		let (mut writer, reader) = tokio::io::duplex(4096);
		let f = DispatchFixtures::new();

		let (tx, rx) = oneshot::channel();
		{
			let mut p = f.pending.lock().await;
			p.insert(
				1,
				PendingRequest {
					sender: Some(tx),
					deadline: tokio::time::Instant::now()
						+ std::time::Duration::from_secs(10),
					method: "empty".into(),
				},
			);
		}

		let reader_task = spawn_reader_task(reader, &f);

		// Write empty lines interspersed with a real response.
		let data = "\n\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}\n\n";
		writer.write_all(data.as_bytes()).await.unwrap();
		writer.flush().await.unwrap();

		tokio::time::sleep(std::time::Duration::from_millis(50)).await;

		let result = rx.await.unwrap().unwrap();
		assert_eq!(result, serde_json::json!("ok"));

		drop(writer);
		let _ = reader_task.await;
	}

	#[tokio::test]
	async fn test_ndjson_invalid_json_skipped() {
		use tokio::io::AsyncWriteExt;

		let (mut writer, reader) = tokio::io::duplex(4096);
		let f = DispatchFixtures::new();

		let (tx, rx) = oneshot::channel();
		{
			let mut p = f.pending.lock().await;
			p.insert(
				1,
				PendingRequest {
					sender: Some(tx),
					deadline: tokio::time::Instant::now()
						+ std::time::Duration::from_secs(10),
					method: "invalid".into(),
				},
			);
		}

		let reader_task = spawn_reader_task(reader, &f);

		// Write invalid JSON followed by a valid response.
		let data = concat!(
			"this is not json\n",
			"{malformed\n",
			r#"{"jsonrpc":"2.0","id":1,"result":"after_invalid"}"#,
			"\n",
		);
		writer.write_all(data.as_bytes()).await.unwrap();
		writer.flush().await.unwrap();

		tokio::time::sleep(std::time::Duration::from_millis(50)).await;

		let result = rx.await.unwrap().unwrap();
		assert_eq!(result, serde_json::json!("after_invalid"));

		drop(writer);
		let _ = reader_task.await;
	}

	#[tokio::test]
	async fn test_request_response_lifecycle_with_mock() {
		// Simulate a full request/response lifecycle using a duplex stream
		// as a mock child process.
		use tokio::io::AsyncWriteExt;

		let (client_writer, server_reader) = tokio::io::duplex(4096);
		let (mut server_writer, client_reader) = tokio::io::duplex(4096);

		let f = DispatchFixtures::new();

		// Spawn a reader on the "client reader" side (simulates stdout reading).
		let reader_task = spawn_reader_task(client_reader, &f);

		// Spawn a "mock server" that reads requests and sends responses.
		let server_task = tokio::spawn(async move {
			let mut buf_reader = BufReader::new(server_reader);
			let mut line = String::new();
			loop {
				line.clear();
				match buf_reader.read_line(&mut line).await {
					Ok(0) => break,
					Ok(_) => {
						let trimmed = line.trim();
						if trimmed.is_empty() {
							continue;
						}
						if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
							// Echo back with a result.
							if let Some(id) = parsed.get("id").and_then(|v| v.as_u64()) {
								let method = parsed
									.get("method")
									.and_then(|v| v.as_str())
									.unwrap_or("?");
								let response = serde_json::json!({
									"jsonrpc": "2.0",
									"id": id,
									"result": { "echo": method }
								});
								let mut data = serde_json::to_vec(&response).unwrap();
								data.push(b'\n');
								let _ = server_writer.write_all(&data).await;
								let _ = server_writer.flush().await;
							}
						}
					}
					Err(_) => break,
				}
			}
		});

		// Simulate sending a request.
		let id = 1u64;
		let (tx, rx) = oneshot::channel();
		{
			let mut p = f.pending.lock().await;
			p.insert(
				id,
				PendingRequest {
					sender: Some(tx),
					deadline: tokio::time::Instant::now()
						+ std::time::Duration::from_secs(10),
					method: "test/echo".into(),
				},
			);
		}

		// Write the request to the "server reader" via the client_writer.
		let request = serde_json::json!({
			"jsonrpc": "2.0",
			"id": id,
			"method": "test/echo",
			"params": {}
		});
		let mut data = serde_json::to_vec(&request).unwrap();
		data.push(b'\n');

		// We need to write to the server side (client_writer goes to server_reader).
		let mut client_out = client_writer;
		client_out.write_all(&data).await.unwrap();
		client_out.flush().await.unwrap();

		// Wait for the response.
		let result = tokio::time::timeout(
			std::time::Duration::from_secs(2),
			rx,
		)
		.await
		.expect("timed out waiting for response")
		.unwrap()
		.unwrap();

		assert_eq!(result, serde_json::json!({"echo": "test/echo"}));

		// Clean up.
		drop(client_out);
		let _ = reader_task.await;
		let _ = server_task.await;
	}
}
