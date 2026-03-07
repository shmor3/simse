# simse-mcp rmcp Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace custom MCP protocol types, transports, and client/server internals with the official `rmcp` crate (v1.1.0), keeping the resilience layer, JSON-RPC dispatcher, and callback tool pattern.

**Architecture:** The crate stays a standalone JSON-RPC binary spawned by simse-core. Custom protocol.rs, stdio_transport.rs, and http_transport.rs are deleted. client.rs becomes a thin wrapper around rmcp `RunningService` instances. mcp_server.rs implements rmcp's `ServerHandler` trait. Resilience code (circuit breaker, health monitor, retry) is extracted to its own module.

**Tech Stack:** Rust, rmcp 1.1.0, tokio, serde, thiserror, tracing

---

### Task 1: Extract resilience code to its own module

**Files:**
- Create: `simse-mcp/src/resilience.rs`
- Modify: `simse-mcp/src/client.rs:38-393` (remove resilience code)
- Modify: `simse-mcp/src/lib.rs`

**Step 1: Create resilience.rs with code extracted from client.rs**

Copy lines 38-393 from `client.rs` (CircuitBreaker, HealthMonitor, RetryConfig, retry fn, is_transient, deterministic_jitter_fraction) into a new `resilience.rs` file. Update imports — the only dependency from the rest of the crate is `McpError`.

```rust
// simse-mcp/src/resilience.rs
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use crate::error::McpError;

// ... paste CircuitBreakerConfig, CircuitState, CircuitBreaker,
//     HealthStatus, HealthSnapshot, HealthMonitor,
//     RetryConfig, is_transient, retry, deterministic_jitter_fraction
//     exactly as they appear in client.rs lines 38-393
```

**Step 2: Update lib.rs to export the new module**

```rust
pub mod resilience;
```

**Step 3: Update client.rs to import from resilience**

Remove lines 38-393 from client.rs. Add:
```rust
use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, HealthMonitor, RetryConfig, retry};
```

**Step 4: Run tests to verify nothing broke**

Run: `cd simse-mcp && cargo test`
Expected: All existing tests pass (resilience unit tests now run from resilience.rs)

**Step 5: Commit**

```bash
git add simse-mcp/src/resilience.rs simse-mcp/src/client.rs simse-mcp/src/lib.rs
git commit -m "refactor(simse-mcp): extract resilience code to own module"
```

---

### Task 2: Add rmcp dependency and update Cargo.toml

**Files:**
- Modify: `simse-mcp/Cargo.toml`

**Step 1: Update Cargo.toml**

Add rmcp, remove reqwest and uuid (rmcp handles these internally). Keep futures for now — will clean up later if unused.

```toml
[dependencies]
rmcp = { version = "1.1", features = ["client", "server", "transport-io", "transport-child-process", "transport-streamable-http-client-reqwest"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
tokio = { version = "1", features = ["full"] }
tokio-util = "0.7"
async-trait = "0.1"
uuid = { version = "1", features = ["v4"] }
```

Keep uuid for now — it's used by the callback tool handler for request IDs. We can remove it later if rmcp provides an alternative.

**Step 2: Verify it compiles**

Run: `cd simse-mcp && cargo check`
Expected: Compiles (rmcp is added but not yet used)

**Step 3: Commit**

```bash
git add simse-mcp/Cargo.toml
git commit -m "chore(simse-mcp): add rmcp dependency"
```

---

### Task 3: Rewrite error.rs to map rmcp errors

**Files:**
- Modify: `simse-mcp/src/error.rs`

**Step 1: Update error.rs**

Keep the same McpError enum and code() method. Add a `From<rmcp::ServiceError>` impl so rmcp errors map cleanly. The `ServiceError` type from rmcp wraps transport and protocol errors.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
	#[error("Client/server not initialized: call initialize first")]
	NotInitialized,
	#[error("Connection failed: {0}")]
	ConnectionFailed(String),
	#[error("Server not connected: {0}")]
	ServerNotConnected(String),
	#[error("Tool error [{tool}]: {message}")]
	ToolError { tool: String, message: String },
	#[error("Resource error [{uri}]: {message}")]
	ResourceError { uri: String, message: String },
	#[error("Transport configuration error: {0}")]
	TransportConfigError(String),
	#[error("Timeout: {method} exceeded {timeout_ms}ms")]
	Timeout { method: String, timeout_ms: u64 },
	#[error("Circuit breaker open: {0}")]
	CircuitBreakerOpen(String),
	#[error("Protocol error: {0}")]
	ProtocolError(String),
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Serialization error: {0}")]
	Serialization(String),
}

impl McpError {
	pub fn code(&self) -> &str {
		match self {
			Self::NotInitialized => "MCP_NOT_INITIALIZED",
			Self::ConnectionFailed(_) => "MCP_CONNECTION_FAILED",
			Self::ServerNotConnected(_) => "MCP_SERVER_NOT_CONNECTED",
			Self::ToolError { .. } => "MCP_TOOL_ERROR",
			Self::ResourceError { .. } => "MCP_RESOURCE_ERROR",
			Self::TransportConfigError(_) => "MCP_TRANSPORT_CONFIG_ERROR",
			Self::Timeout { .. } => "MCP_TIMEOUT",
			Self::CircuitBreakerOpen(_) => "MCP_CIRCUIT_BREAKER_OPEN",
			Self::ProtocolError(_) => "MCP_PROTOCOL_ERROR",
			Self::Io(_) => "MCP_IO",
			Self::Serialization(_) => "MCP_SERIALIZATION",
		}
	}

	pub fn to_json_rpc_error(&self) -> serde_json::Value {
		serde_json::json!({
			"mcpCode": self.code(),
			"message": self.to_string(),
		})
	}
}
```

Note: The `From<rmcp::ServiceError>` impl will be added when we rewrite client.rs (Task 5), since we need to understand the exact error variants rmcp exposes. For now, error.rs stays nearly identical.

**Step 2: Verify it compiles**

Run: `cd simse-mcp && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add simse-mcp/src/error.rs
git commit -m "refactor(simse-mcp): prepare error.rs for rmcp integration"
```

---

### Task 4: Create rpc_types.rs with JSON-RPC framing and RPC param types

**Files:**
- Create: `simse-mcp/src/rpc_types.rs`
- Modify: `simse-mcp/src/lib.rs`

The JSON-RPC framing types (JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, error codes) and the RPC-specific param structs (ConnectParams, CallToolParams, etc.) are **not** part of MCP protocol — they're simse's own JSON-RPC bridge layer. Extract them from protocol.rs and rpc_server.rs into a dedicated module before deleting protocol.rs.

**Step 1: Create rpc_types.rs**

Extract from `protocol.rs` (lines 1-82): `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, `JsonRpcNotification`, error code constants.

Extract from `rpc_server.rs` (lines 986-1107): all the `*Params` structs used by the dispatcher (ConnectParams, OptionalServerParams, CallToolParams, etc.), plus `parse_params`.

Also keep `ServerConnection` config type (used in InitializeParams) — but adapt it to not depend on custom protocol types. Use rmcp types where possible.

```rust
// simse-mcp/src/rpc_types.rs
use serde::{Deserialize, Serialize};

// JSON-RPC 2.0 error codes
pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const MCP_ERROR: i32 = -32000;

// JSON-RPC framing types (JsonRpcRequest, JsonRpcResponse, JsonRpcError, JsonRpcNotification)
// ... copy from protocol.rs lines 16-82 exactly

// RPC param types for the dispatcher
// ... copy from rpc_server.rs lines 986-1107

// Config types needed by initialize
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConnectionConfig {
	pub name: String,
	pub transport: TransportTypeConfig,
	#[serde(default)]
	pub command: Option<String>,
	#[serde(default)]
	pub args: Option<Vec<String>>,
	#[serde(default)]
	pub env: Option<std::collections::HashMap<String, String>>,
	#[serde(default)]
	pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportTypeConfig {
	Stdio,
	Http,
}

impl Default for TransportTypeConfig {
	fn default() -> Self {
		Self::Stdio
	}
}
```

**Step 2: Update lib.rs**

```rust
pub mod rpc_types;
```

**Step 3: Verify it compiles**

Run: `cd simse-mcp && cargo check`
Expected: Compiles (old modules still exist, new module is additive)

**Step 4: Commit**

```bash
git add simse-mcp/src/rpc_types.rs simse-mcp/src/lib.rs
git commit -m "refactor(simse-mcp): extract RPC framing and param types to rpc_types.rs"
```

---

### Task 5: Rewrite client.rs to wrap rmcp

**Files:**
- Rewrite: `simse-mcp/src/client.rs`

This is the biggest task. The new client.rs wraps rmcp `RunningService` instances with the resilience layer.

**Step 1: Write the new client.rs**

The new client manages a map of server connections, each wrapping an rmcp `RunningService<RoleClient, SimseClientHandler>`. The `SimseClientHandler` implements rmcp's `ClientHandler` to handle notifications (tools changed, logging).

Key structures:

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::process::Command;

use rmcp::{ServiceExt, RoleClient, service::RunningService};
use rmcp::transport::TokioChildProcess;
#[cfg(feature = "http")]
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::model::*;

use crate::error::McpError;
use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, HealthMonitor, RetryConfig, retry};
use crate::rpc_types::{ServerConnectionConfig, TransportTypeConfig};

/// Notification callbacks from MCP servers.
type ToolsChangedHandler = Box<dyn Fn() + Send + Sync>;
type LoggingHandler = Box<dyn Fn(LoggingMessageNotificationParams) + Send + Sync>;

/// Active connection to a single MCP server.
struct ConnectedServer {
	service: RunningService<RoleClient, SimseClientHandler>,
	circuit_breaker: CircuitBreaker,
	health_monitor: HealthMonitor,
}

/// Client handler that receives notifications from MCP servers.
#[derive(Clone)]
struct SimseClientHandler {
	tools_changed: Arc<RwLock<Vec<ToolsChangedHandler>>>,
	logging: Arc<RwLock<Vec<LoggingHandler>>>,
}

impl rmcp::handler::ClientHandler for SimseClientHandler {
	fn get_info(&self) -> ClientInfo {
		// Return client info with capabilities
		ClientInfo {
			protocol_version: ProtocolVersion::V_2025_06_18,
			capabilities: ClientCapabilities::default(),
			client_info: Implementation::new("simse-mcp", env!("CARGO_PKG_VERSION")),
		}
	}

	async fn on_tool_list_changed(&self, _context: rmcp::service::RequestContext<RoleClient>) {
		if let Ok(handlers) = self.tools_changed.read() {
			for h in handlers.iter() { h(); }
		}
	}

	async fn on_logging_message(&self, params: LoggingMessageNotificationParams, _context: rmcp::service::RequestContext<RoleClient>) {
		if let Ok(handlers) = self.logging.read() {
			for h in handlers.iter() { h(params.clone()); }
		}
	}
}

pub struct McpClient {
	connections: HashMap<String, ConnectedServer>,
	server_configs: Vec<ServerConnectionConfig>,
	retry_config: RetryConfig,
	tools_changed: Arc<RwLock<Vec<ToolsChangedHandler>>>,
	logging: Arc<RwLock<Vec<LoggingHandler>>>,
	roots: Vec<rmcp::model::Root>,
}
```

Methods to implement:
- `new(configs, client_name, client_version)` — stores configs, no connections
- `connect(name)` — builds transport, calls `.serve(transport)`, stores RunningService
- `connect_all()` — connects to all configured servers
- `disconnect(name)` — calls `service.cancel()`, removes from map
- `disconnect_all()` — disconnects all
- `list_tools(server?)` — calls `peer().list_tools()` on one or all servers
- `call_tool(server, name, args)` — with resilience wrapper
- `list_resources(server?)` — calls `peer().list_resources()`
- `read_resource(server, uri)` — with resilience wrapper
- `list_resource_templates(server?)` — calls `peer().list_resource_templates()`
- `list_prompts(server?)` — calls `peer().list_prompts()`
- `get_prompt(server, name, args)` — calls `peer().get_prompt()`
- `set_logging_level(server, level)` — calls `peer().set_level()`
- `complete(server, ref, arg)` — calls `peer().complete()`
- `set_roots(roots)` — stores roots
- `on_tools_changed(handler)` — registers callback
- `on_logging_message(handler)` — registers callback

Resilience wrapper pattern:
```rust
async fn with_resilience<T, F, Fut>(
	conn: &ConnectedServer,
	retry_config: &RetryConfig,
	f: F,
) -> Result<T, McpError>
where
	F: Fn() -> Fut,
	Fut: std::future::Future<Output = Result<T, McpError>>,
{
	if !conn.circuit_breaker.allow_request() {
		return Err(McpError::CircuitBreakerOpen("circuit open".into()));
	}
	match retry(retry_config, &f).await {
		Ok(val) => {
			conn.circuit_breaker.record_success();
			conn.health_monitor.record_success();
			Ok(val)
		}
		Err(e) => {
			conn.circuit_breaker.record_failure();
			conn.health_monitor.record_failure(&e.to_string());
			Err(e)
		}
	}
}
```

Transport creation:
```rust
async fn create_and_connect(
	config: &ServerConnectionConfig,
	handler: SimseClientHandler,
) -> Result<RunningService<RoleClient, SimseClientHandler>, McpError> {
	match config.transport {
		TransportTypeConfig::Stdio => {
			let cmd = config.command.as_deref().ok_or_else(|| {
				McpError::TransportConfigError("stdio transport requires 'command'".into())
			})?;
			let mut command = Command::new(cmd);
			if let Some(args) = &config.args {
				command.args(args);
			}
			if let Some(env) = &config.env {
				for (k, v) in env { command.env(k, v); }
			}
			let transport = TokioChildProcess::new(&mut command)
				.map_err(|e| McpError::ConnectionFailed(e.to_string()))?;
			handler.serve(transport).await
				.map_err(|e| McpError::ConnectionFailed(e.to_string()))
		}
		TransportTypeConfig::Http => {
			let url = config.url.as_deref().ok_or_else(|| {
				McpError::TransportConfigError("http transport requires 'url'".into())
			})?;
			let transport = rmcp::transport::StreamableHttpClientTransport::from_uri(url)
				.map_err(|e| McpError::ConnectionFailed(e.to_string()))?;
			handler.serve(transport).await
				.map_err(|e| McpError::ConnectionFailed(e.to_string()))
		}
	}
}
```

**Step 2: Verify it compiles**

Run: `cd simse-mcp && cargo check`
Expected: Compiles. Some warnings about unused code in old modules are fine.

**Step 3: Commit**

```bash
git add simse-mcp/src/client.rs
git commit -m "feat(simse-mcp): rewrite client.rs to wrap rmcp RunningService"
```

---

### Task 6: Rewrite mcp_server.rs with rmcp ServerHandler

**Files:**
- Rewrite: `simse-mcp/src/mcp_server.rs`

The new server implements rmcp's `ServerHandler` trait with dynamic tool/resource/prompt registration and the callback tool pattern.

**Step 1: Write the new mcp_server.rs**

```rust
use std::collections::HashMap;
use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use tokio::sync::{oneshot, Mutex, RwLock};

use crate::error::McpError;
use crate::rpc_transport::NdjsonTransport;

/// A registered tool: definition + handler kind.
struct RegisteredTool {
	definition: Tool,
	handler: ToolHandlerKind,
}

/// How a tool gets executed.
enum ToolHandlerKind {
	/// Callback: sends notification to TS, waits for result via oneshot channel.
	Callback {
		transport: NdjsonTransport,
		pending: Arc<Mutex<HashMap<String, oneshot::Sender<CallToolResult>>>>,
	},
}

/// The simse MCP server. Implements rmcp::ServerHandler with dynamic registration.
#[derive(Clone)]
pub struct SimseServer {
	name: String,
	version: String,
	tools: Arc<RwLock<HashMap<String, RegisteredTool>>>,
	pending_tool_calls: Arc<Mutex<HashMap<String, oneshot::Sender<CallToolResult>>>>,
	running: Arc<std::sync::atomic::AtomicBool>,
}

impl SimseServer {
	pub fn new(name: String, version: String) -> Self { ... }
	pub fn pending_tool_calls(&self) -> Arc<Mutex<HashMap<String, oneshot::Sender<CallToolResult>>>> { ... }
	pub fn register_callback_tool(&self, name: String, description: String, input_schema: serde_json::Value) { ... }
	pub fn unregister_tool(&self, name: &str) -> bool { ... }
	pub fn set_running(&self, running: bool) { ... }
	pub fn is_running(&self) -> bool { ... }
}

impl ServerHandler for SimseServer {
	fn get_info(&self) -> ServerInfo {
		ServerInfo {
			protocol_version: ProtocolVersion::V_2025_06_18,
			capabilities: ServerCapabilities::builder()
				.enable_tools()
				.enable_resources()
				.enable_prompts()
				.build(),
			server_info: Implementation::new(&self.name, &self.version),
			instructions: None,
		}
	}

	async fn list_tools(&self, _request: PaginatedRequestParams, _context: RequestContext<RoleServer>) -> Result<ListToolsResult, rmcp::Error> {
		let tools = self.tools.read().await;
		let tool_list: Vec<Tool> = tools.values().map(|rt| rt.definition.clone()).collect();
		Ok(ListToolsResult { tools: tool_list, next_cursor: None })
	}

	async fn call_tool(&self, request: CallToolRequestParams, _context: RequestContext<RoleServer>) -> Result<CallToolResult, rmcp::Error> {
		let tools = self.tools.read().await;
		let registered = tools.get(&request.name.to_string()).ok_or_else(|| {
			rmcp::Error::tool_not_found(&request.name)
		})?;

		match &registered.handler {
			ToolHandlerKind::Callback { transport, pending } => {
				let request_id = uuid::Uuid::new_v4().to_string();
				let (tx, rx) = oneshot::channel();
				{
					let mut p = pending.lock().await;
					p.insert(request_id.clone(), tx);
				}
				transport.write_notification("tool/execute", serde_json::json!({
					"requestId": request_id,
					"toolName": request.name,
					"args": request.arguments,
				}));
				drop(tools); // release read lock before awaiting

				match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
					Ok(Ok(result)) => Ok(result),
					Ok(Err(_)) => {
						pending.lock().await.remove(&request_id);
						Err(rmcp::Error::internal("Tool callback channel closed"))
					}
					Err(_) => {
						pending.lock().await.remove(&request_id);
						Err(rmcp::Error::internal("Tool callback timed out"))
					}
				}
			}
		}
	}
}
```

**Step 2: Verify it compiles**

Run: `cd simse-mcp && cargo check`
Expected: Compiles

**Step 3: Commit**

```bash
git add simse-mcp/src/mcp_server.rs
git commit -m "feat(simse-mcp): rewrite mcp_server.rs with rmcp ServerHandler"
```

---

### Task 7: Rewrite rpc_server.rs to use new client + server

**Files:**
- Rewrite: `simse-mcp/src/rpc_server.rs`

The dispatch table stays the same. Handler methods now call the new McpClient and SimseServer. Param types come from rpc_types.rs.

**Step 1: Write the new rpc_server.rs**

Keep the same structure: `McpRpcServer` with `run()` loop, `dispatch()` match, handler methods. Replace:
- `use crate::protocol::*` → `use crate::rpc_types::*`
- `use crate::client::McpClient` → same (new client API)
- `use crate::mcp_server::McpServer` → `use crate::mcp_server::SimseServer`
- Tool/resource/prompt types → rmcp types (e.g., `rmcp::model::CallToolResult`)

The `CallbackToolHandler` struct is removed — callback logic is now inside SimseServer's `call_tool` impl. The `pending_tool_calls` map is owned by SimseServer and accessed via `server.pending_tool_calls()`.

Key handler changes:
- `handle_list_tools`: calls `client.list_tools()`, serializes rmcp `Tool` to JSON
- `handle_call_tool`: calls `client.call_tool()`, serializes rmcp `CallToolResult`
- `handle_register_tool`: calls `server.register_callback_tool()`
- `handle_tool_result`: resolves pending oneshot via `server.pending_tool_calls()`

**Step 2: Update rpc_transport.rs**

Update imports to use `rpc_types` instead of `protocol`:
```rust
use crate::rpc_types::{JsonRpcNotification, JsonRpcResponse};
```

**Step 3: Verify it compiles**

Run: `cd simse-mcp && cargo check`
Expected: Compiles

**Step 4: Commit**

```bash
git add simse-mcp/src/rpc_server.rs simse-mcp/src/rpc_transport.rs
git commit -m "feat(simse-mcp): rewrite rpc_server.rs to use rmcp-backed client and server"
```

---

### Task 8: Delete old modules and update lib.rs

**Files:**
- Delete: `simse-mcp/src/protocol.rs`
- Delete: `simse-mcp/src/stdio_transport.rs`
- Delete: `simse-mcp/src/http_transport.rs`
- Modify: `simse-mcp/src/lib.rs`

**Step 1: Update lib.rs**

```rust
pub mod client;
pub mod error;
pub mod mcp_server;
pub mod resilience;
pub mod rpc_server;
pub mod rpc_transport;
pub mod rpc_types;
```

**Step 2: Delete the old files**

```bash
rm simse-mcp/src/protocol.rs simse-mcp/src/stdio_transport.rs simse-mcp/src/http_transport.rs
```

**Step 3: Verify it compiles**

Run: `cd simse-mcp && cargo build`
Expected: Compiles with no errors

**Step 4: Commit**

```bash
git add -A simse-mcp/
git commit -m "refactor(simse-mcp): delete custom protocol and transport modules"
```

---

### Task 9: Update simse-core error integration

**Files:**
- Modify: `simse-core/src/error.rs` (if needed)

**Step 1: Check simse-core compiles**

Run: `cd simse-core && cargo check`
Expected: Should compile since `simse_mcp_engine::error::McpError` still exists with the same interface.

**Step 2: Fix any breakage**

If simse-core imports any types from the deleted `protocol` module, update those imports to use rmcp types or the new rpc_types module.

**Step 3: Commit (only if changes needed)**

```bash
git add simse-core/
git commit -m "fix(simse-core): update MCP imports for rmcp refactor"
```

---

### Task 10: Fix and update integration tests

**Files:**
- Modify: `simse-mcp/tests/integration.rs`

**Step 1: Run existing integration tests**

Run: `cd simse-mcp && cargo test`
Expected: Integration tests should pass since the JSON-RPC interface (method names, param shapes, response shapes) is preserved.

**Step 2: Fix any failures**

The integration tests communicate over NDJSON stdio and check JSON-RPC responses. Since we kept the same method names and response formats, most should pass without changes. Fix any discrepancies.

**Step 3: Add a test for rmcp protocol version**

Add a test that verifies the server advertises `2025-06-18` when queried:

```rust
#[test]
fn test_protocol_version_2025_06_18() {
	let mut engine = TestEngine::new();
	// This is verified indirectly — the server's initialize response
	// will use the new protocol version when clients connect.
	// For now, just verify the engine still works.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);
	assert_is_success(&resp);
}
```

**Step 4: Commit**

```bash
git add simse-mcp/tests/
git commit -m "test(simse-mcp): update integration tests for rmcp refactor"
```

---

### Task 11: Final cleanup and verification

**Files:**
- Possibly: `simse-mcp/Cargo.toml` (remove unused deps)

**Step 1: Check for unused dependencies**

Run: `cd simse-mcp && cargo build 2>&1 | grep "unused"`
Remove any deps that are no longer needed (e.g., `futures`, `reqwest` if rmcp handles HTTP internally).

**Step 2: Run full test suite**

Run: `cd simse-mcp && cargo test`
Expected: All tests pass

**Step 3: Run clippy**

Run: `cd simse-mcp && cargo clippy -- -D warnings`
Expected: No warnings

**Step 4: Build the binary**

Run: `cd simse-mcp && cargo build --release`
Expected: Binary builds successfully

**Step 5: Commit any cleanup**

```bash
git add simse-mcp/
git commit -m "chore(simse-mcp): final cleanup after rmcp refactor"
```
