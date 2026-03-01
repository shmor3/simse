# ACP & MCP Rust Crates Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract ACP and MCP protocol implementations into two new pure Rust crates with transitional JSON-RPC wrappers for TS interop.

**Architecture:** Two Rust crates (`simse-acp/`, `simse-mcp/`) following the established simse pattern (lib.rs library API + main.rs JSON-RPC wrapper). The TS code in `src/ai/acp/` and `src/ai/mcp/` shrinks to thin JSON-RPC clients. Existing consumers (loop, tools, chain, agent) don't change.

**Tech Stack:** Rust (tokio, serde, serde_json, thiserror, tracing, uuid, reqwest), TypeScript (Bun)

**Reference files:**
- Design: `docs/plans/2026-03-01-acp-mcp-rust-design.md`
- Existing ACP TS: `src/ai/acp/` (acp-client.ts, acp-connection.ts, acp-results.ts, types.ts)
- Existing MCP TS: `src/ai/mcp/` (mcp-client.ts, mcp-server.ts, types.ts)
- Existing Rust pattern: `simse-vector/src/` (main.rs, transport.rs, server.rs, protocol.rs, error.rs)
- Existing TS client pattern: `src/ai/library/client.ts`, `src/ai/library/stacks.ts`

---

## Phase A: simse-acp Rust Crate

### Task 1: Scaffold simse-acp crate

**Files:**
- Create: `simse-acp/Cargo.toml`
- Create: `simse-acp/src/lib.rs`
- Create: `simse-acp/src/main.rs`
- Create: `simse-acp/src/error.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "simse-acp-engine"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "ACP (Agent Client Protocol) engine over JSON-RPC 2.0 / NDJSON stdio"

[lib]
name = "simse_acp_engine"
path = "src/lib.rs"

[[bin]]
name = "simse-acp-engine"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
uuid = { version = "1", features = ["v4"] }
tokio = { version = "1", features = ["full"] }
futures = "0.3"

[dev-dependencies]
tempfile = "3"
tokio-test = "0.4"
```

**Step 2: Create error.rs**

Follow the simse-vector error.rs pattern. Define `AcpError` enum with variants:
- `NotInitialized` — client not initialized
- `ConnectionFailed(String)` — failed to spawn/connect agent process
- `SessionError(String)` — session creation/management failure
- `Timeout { method: String, timeout_ms: u64 }` — request timeout
- `StreamError(String)` — streaming failure
- `PermissionDenied(String)` — permission rejected
- `CircuitBreakerOpen(String)` — circuit breaker tripped
- `ServerUnavailable(String)` — server not running
- `ProtocolError(String)` — JSON-RPC protocol error
- `Io(#[from] std::io::Error)` — I/O error
- `Serialization(String)` — JSON parse error

Implement `code() -> &str` and `to_json_rpc_error()` methods following the existing pattern. Error codes prefixed with `ACP_`.

**Step 3: Create lib.rs**

```rust
pub mod error;
```

**Step 4: Create main.rs**

Minimal placeholder following simse-vector pattern:

```rust
fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("simse-acp-engine ready (placeholder)");
}
```

**Step 5: Verify crate builds**

Run: `cd simse-acp && cargo build`
Expected: Compiles with no errors.

**Step 6: Commit**

```bash
git add simse-acp/
git commit -m "feat(acp): scaffold simse-acp Rust crate"
```

---

### Task 2: ACP protocol types

**Files:**
- Create: `simse-acp/src/protocol.rs`
- Modify: `simse-acp/src/lib.rs`

**Step 1: Write protocol types**

Define all ACP protocol types as Rust structs/enums with serde derives. Reference `src/ai/acp/types.ts` for exact field names. Use `#[serde(rename_all = "camelCase")]` throughout.

**JSON-RPC base types:**
```rust
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}
```

Plus error code constants: `INTERNAL_ERROR`, `METHOD_NOT_FOUND`, `INVALID_PARAMS`, `ACP_ERROR`.

**ACP protocol types (all with Serialize + Deserialize):**
- `InitializeParams` — `{ protocol_version, client_info: ClientInfo, capabilities }`
- `InitializeResult` — `{ protocol_version, agent_info: AgentInfo, agent_capabilities?, auth_methods? }`
- `ClientInfo` / `AgentInfo` — `{ name, version }`
- `AgentCapabilities` — `{ load_session?, prompt_capabilities?, session_capabilities?, mcp_capabilities? }`
- `SessionNewParams` — `{ cwd, mcp_servers }`
- `SessionInfo` — `{ session_id, models?, modes? }`
- `SessionListEntry` — `{ session_id, created_at?, last_active_at? }`
- `SessionPromptParams` — `{ session_id, prompt: Vec<ContentBlock>, metadata? }`
- `SessionPromptResult` — `{ content?, stop_reason?, metadata? }`
- `ContentBlock` — enum: `Text { text }`, `Resource { resource }`, `ResourceLink { uri, name, ... }`, `Data { data, mime_type? }`
- `SessionUpdate` — `{ session_update, content?, metadata?, ... }` (for notifications)
- `PermissionRequestParams` — `{ title?, description?, tool_call?, options: Vec<PermissionOption> }`
- `PermissionOption` — `{ option_id, kind, name, description? }`
- `PermissionResult` — `{ outcome: PermissionOutcome }`
- `PermissionOutcome` — `{ outcome: "selected"|"cancelled", option_id? }`
- `ToolCall` — `{ tool_call_id, title, kind, status }`
- `ToolCallUpdate` — `{ tool_call_id, status, content? }`
- `TokenUsage` — `{ prompt_tokens, completion_tokens, total_tokens }`
- `StopReason` — enum: `EndTurn`, `MaxTokens`, `MaxTurnRequests`, `Refusal`, `Cancelled`, `StopSequence`, `ToolUse`
- `ModelsInfo` — `{ available_models, current_model_id }`
- `ModesInfo` — `{ current_mode_id, available_modes }`
- `ModelInfo` — `{ model_id, name, description? }`
- `ModeInfo` — `{ id, name, description? }`
- `SetConfigOptionParams` — `{ session_id, config_option_id, group_id }`
- `SamplingParams` — `{ temperature?, max_tokens?, top_p?, top_k?, stop_sequences? }`

**Important serde notes:**
- Protocol uses `client_info` (snake_case) in initialize, but `sessionId` (camelCase) elsewhere
- `TokenUsage` must accept both `prompt_tokens` and `promptTokens` (use `#[serde(alias)]`)
- `ContentBlock` is a tagged enum with `#[serde(tag = "type")]`

**Step 2: Register module**

Add `pub mod protocol;` to `lib.rs`.

**Step 3: Verify**

Run: `cd simse-acp && cargo build`

**Step 4: Commit**

```bash
git commit -m "feat(acp): add ACP protocol types"
```

---

### Task 3: NDJSON transport layer

**Files:**
- Create: `simse-acp/src/transport.rs`
- Modify: `simse-acp/src/lib.rs`

Copy the `NdjsonTransport` implementation from simse-vector's `transport.rs` exactly. This handles writing JSON-RPC responses, errors, and notifications to stdout.

Three public methods: `write_response()`, `write_error()`, `write_notification()`.

This is for the transitional `main.rs` wrapper — the internal ACP client talks directly to agent subprocesses.

**Step 1: Create transport.rs** (copy from simse-vector, change module name)

**Step 2: Register module, verify build, commit**

```bash
git commit -m "feat(acp): add NDJSON transport layer"
```

---

### Task 4: Connection to agent subprocess

**Files:**
- Create: `simse-acp/src/connection.rs`
- Modify: `simse-acp/src/lib.rs`

The connection module manages a single child process running an ACP-compatible agent (e.g., Claude Code). It handles:
- Spawning the process with stdio pipes
- NDJSON buffer parsing from stdout
- Pending request tracking with timeouts
- Sending JSON-RPC requests and receiving responses
- Routing notifications to handlers
- Health checking (is process alive?)
- Cleanup on close

**Step 1: Write unit tests for NDJSON buffer parsing**

In `connection.rs`, test the buffer parsing logic:
- Complete line → parsed as JSON-RPC
- Partial line → buffered until newline
- Multiple lines in one read → all parsed
- Empty line → skipped
- Invalid JSON → logged and skipped

**Step 2: Implement connection**

Key struct:

```rust
pub struct AcpConnection {
    child: Option<tokio::process::Child>,
    stdin: Option<tokio::process::ChildStdin>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, PendingRequest>>>,
    notification_handlers: Arc<Mutex<HashMap<String, Vec<NotificationHandler>>>>,
    connected: AtomicBool,
    server_info: Mutex<Option<InitializeResult>>,
    permission_policy: Mutex<PermissionPolicy>,
}
```

Public API:
- `async fn new(config: ConnectionConfig) -> Result<Self>` — spawn process
- `async fn initialize(&self) -> Result<InitializeResult>` — send initialize request
- `async fn request<T: DeserializeOwned>(&self, method: &str, params: Value, timeout_ms: u64) -> Result<T>` — send request, await response
- `fn notify(&self, method: &str, params: Value)` — fire-and-forget notification
- `fn on_notification(&self, method: &str, handler: NotificationHandler) -> SubscriptionHandle` — register handler
- `async fn close(&self)` — kill process, reject pending
- `fn is_healthy(&self) -> bool` — check process liveness

The stdout reader runs as a tokio task, parsing NDJSON lines and routing to pending requests or notification handlers.

`ConnectionConfig`:
```rust
pub struct ConnectionConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: HashMap<String, String>,
    pub timeout_ms: u64,        // default 60_000
    pub init_timeout_ms: u64,   // default 30_000
}
```

**Step 3: Test spawn + initialize + request cycle**

Write integration test that spawns a mock agent process (simple echo server) and verifies the full lifecycle.

**Step 4: Verify and commit**

Run: `cd simse-acp && cargo test`

```bash
git commit -m "feat(acp): add connection to agent subprocess"
```

---

### Task 5: Permission handling

**Files:**
- Create: `simse-acp/src/permission.rs`
- Modify: `simse-acp/src/connection.rs`
- Modify: `simse-acp/src/lib.rs`

**Step 1: Define permission types and policy enum**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPolicy {
    AutoApprove,
    Prompt,
    Deny,
}
```

**Step 2: Implement permission handling in connection**

When the connection receives a JSON-RPC **request** (not notification) with method `session/request_permission`:

1. Parse `PermissionRequestParams` from params
2. Based on policy:
   - `AutoApprove`: Select first `allow_always` option, fallback to `allow_once`, fallback to first option
   - `Deny`: Select first `reject_once` option, fallback to `reject_always`, fallback to `{ outcome: "cancelled" }`
   - `Prompt`: Call registered permission callback, await response
3. Send JSON-RPC response with `PermissionResult`
4. During permission wait: suspend prompt request timeouts, emit permission activity

**Step 3: Write unit tests**

- Auto-approve selects correct option from list
- Deny selects correct rejection option
- Missing options handled gracefully (cancelled)
- Timeout suspension during permission wait

**Step 4: Verify and commit**

```bash
git commit -m "feat(acp): add permission handling"
```

---

### Task 6: Streaming state machine

**Files:**
- Create: `simse-acp/src/stream.rs`
- Modify: `simse-acp/src/lib.rs`

The streaming module implements the complex state machine for `generateStream`. This is the hardest part of the ACP crate.

**Step 1: Define stream chunk types**

```rust
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Delta { text: String },
    Complete { usage: Option<TokenUsage> },
    ToolCall { tool_call: ToolCall },
    ToolCallUpdate { update: ToolCallUpdate },
}
```

**Step 2: Implement AcpStream**

`AcpStream` implements `futures::Stream<Item = StreamChunk>`. Internally:

- Receives `session/update` notifications via a `tokio::sync::mpsc` channel
- Parses notification `sessionUpdate` field to determine chunk type
- Sliding-window timeout: resets on each chunk, fires if no activity for `stream_timeout_ms`
- Permission activity keepalive: resets timeout when permission is in progress
- Completes when the `session/prompt` response arrives
- Supports `AbortSignal`-equivalent via `tokio_util::sync::CancellationToken`

**Step 3: Write unit tests**

- Delta chunks yielded in order
- Complete chunk yielded on prompt response
- Timeout fires after inactivity
- Permission activity resets timeout
- Cancellation token stops stream

**Step 4: Verify and commit**

```bash
git commit -m "feat(acp): add streaming state machine"
```

---

### Task 7: Resilience — circuit breaker, health monitor, retry

**Files:**
- Create: `simse-acp/src/resilience.rs`
- Modify: `simse-acp/src/lib.rs`

**Step 1: Implement CircuitBreaker**

States: `Closed`, `Open(Instant)`, `HalfOpen`. Default thresholds: `failure_threshold: 5`, `reset_timeout_ms: 30_000`, `half_open_max_attempts: 1`.

Methods:
- `fn allow_request(&self) -> bool`
- `fn record_success(&self)`
- `fn record_failure(&self)`
- `fn state(&self) -> CircuitState`

**Step 2: Implement HealthMonitor**

Tracks consecutive failures, total calls, failure rate in sliding window (60s).

Methods:
- `fn record_success(&self)`
- `fn record_failure(&self, error: &str)`
- `fn snapshot(&self) -> HealthSnapshot`
- `fn status(&self) -> HealthStatus` — healthy/degraded (>=3 failures)/unhealthy (>=5)

**Step 3: Implement retry with exponential backoff**

```rust
pub async fn retry<T, F, Fut>(config: RetryConfig, f: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
```

Default config: `max_attempts: 3`, `base_delay_ms: 500`, `max_delay_ms: 15_000`, `backoff_multiplier: 2.0`, `jitter_factor: 0.25`.

Delay calculation: `min(base * multiplier^(attempt-1), max) + jitter`.

Only retry on transient errors (timeout, unavailable, connection reset).

**Step 4: Write unit tests**

- Circuit breaker state transitions (closed → open → half-open → closed/open)
- Health monitor status thresholds
- Retry backoff delay calculation
- Retry stops on non-transient errors

**Step 5: Verify and commit**

```bash
git commit -m "feat(acp): add resilience (circuit breaker, health, retry)"
```

---

### Task 8: AcpClient — full client orchestration

**Files:**
- Create: `simse-acp/src/client.rs`
- Modify: `simse-acp/src/lib.rs`

The main client that consumers interact with. Manages a pool of connections (one per configured server), resilience per server, session caching.

**Step 1: Define AcpConfig**

```rust
pub struct AcpConfig {
    pub servers: Vec<ServerEntry>,
    pub default_server: Option<String>,
    pub default_agent: Option<String>,
    pub mcp_servers: Vec<McpServerConfig>,
}

pub struct ServerEntry {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: HashMap<String, String>,
    pub default_agent: Option<String>,
    pub timeout_ms: Option<u64>,
    pub permission_policy: Option<PermissionPolicy>,
}
```

**Step 2: Implement AcpClient**

```rust
pub struct AcpClient {
    connections: HashMap<String, AcpConnection>,
    circuit_breakers: HashMap<String, CircuitBreaker>,
    health_monitors: HashMap<String, HealthMonitor>,
    session_cache: HashMap<String, SessionInfo>,
    config: AcpConfig,
}
```

Public API (matches design doc):
- `pub async fn new(config: AcpConfig) -> Result<Self>` — spawn all server connections, initialize each
- `pub async fn dispose(&mut self) -> Result<()>` — close all connections
- `pub async fn generate(&self, prompt: &str, options: GenerateOptions) -> Result<GenerateResult>` — create session, send prompt, return result
- `pub async fn chat(&self, messages: &[ChatMessage], options: ChatOptions) -> Result<GenerateResult>`
- `pub fn generate_stream(&self, prompt: &str, options: StreamOptions) -> AcpStream` — streaming generation
- `pub async fn embed(&self, input: &[&str], model: Option<&str>, server: Option<&str>) -> Result<EmbedResult>`
- `pub async fn list_agents(&self, server: Option<&str>) -> Result<Vec<AgentInfo>>`
- Session management: `list_sessions`, `load_session`, `delete_session`, `set_session_mode`, `set_session_model`
- Health: `is_available`, `server_names`
- `pub fn set_permission_policy(&self, policy: PermissionPolicy)`

**Key behavior:**
- `resolve_connection(server_name)` — find connection by name or default
- `with_resilience(server_name, operation, f)` — circuit breaker + retry wrapper
- On unhealthy connection during retry: close and reconnect
- Session creation is implicit (generate/chat create session on demand)
- Session info (models/modes) cached from session/new response

**Step 3: Write unit tests**

- Connection resolution (default server, named server, missing server)
- Generate request → response lifecycle (with mock connection)
- Session caching behavior
- Permission policy propagation

**Step 4: Verify and commit**

```bash
git commit -m "feat(acp): add AcpClient orchestration layer"
```

---

### Task 9: JSON-RPC wrapper (main.rs transitional server)

**Files:**
- Create: `simse-acp/src/server.rs`
- Modify: `simse-acp/src/main.rs`
- Modify: `simse-acp/src/lib.rs`

The transitional JSON-RPC server that the TS thin client communicates with. Follows the simse-vector server.rs pattern but is **async** (uses tokio) because AcpClient is async.

**Step 1: Implement AcpServer**

```rust
pub struct AcpServer {
    transport: NdjsonTransport,
    client: Option<AcpClient>,
}
```

Dispatch method names:
- `acp/initialize` → create AcpClient from params
- `acp/generate` → client.generate()
- `acp/chat` → client.chat()
- `acp/streamStart` → client.generate_stream(), spawn task to forward chunks as notifications
- `acp/embed` → client.embed()
- `acp/listAgents` → client.list_agents()
- `acp/listSessions` → client.list_sessions()
- `acp/loadSession` → client.load_session()
- `acp/deleteSession` → client.delete_session()
- `acp/setSessionMode` → client.set_session_mode()
- `acp/setSessionModel` → client.set_session_model()
- `acp/setPermissionPolicy` → client.set_permission_policy()
- `acp/permissionResponse` → forward to pending permission request
- `acp/serverHealth` → health snapshot
- `acp/dispose` → client.dispose()

**Streaming:** `acp/streamStart` creates a stream and spawns a tokio task that:
1. Iterates the stream
2. For each chunk, calls `transport.write_notification()` with the appropriate method:
   - `stream/delta { streamId, text }`
   - `stream/toolCall { streamId, toolCall }`
   - `stream/toolCallUpdate { streamId, update }`
   - `stream/complete { streamId, usage? }`
3. Returns `{ streamId }` immediately

**Permission callbacks:** When AcpClient's connection receives a permission request, the server writes a notification `permission/request { requestId, description, options }`. The TS client responds with `acp/permissionResponse { requestId, optionId }`.

**Step 2: Update main.rs**

```rust
use simse_acp_engine::server::AcpServer;
use simse_acp_engine::transport::NdjsonTransport;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let transport = NdjsonTransport::new();
    let mut server = AcpServer::new(transport);

    tracing::info!("simse-acp-engine ready");

    if let Err(e) = server.run().await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

**Note:** Unlike simse-vector's synchronous `server.run()`, this uses `server.run().await` because the ACP client is async. The server reads stdin lines synchronously but dispatches to async handlers via tokio.

**Step 3: Verify build, commit**

```bash
git commit -m "feat(acp): add JSON-RPC wrapper server"
```

---

### Task 10: Rust integration tests

**Files:**
- Create: `simse-acp/tests/integration.rs`

Write integration tests that spawn the `simse-acp-engine` binary and communicate via JSON-RPC, following the simse-vector `tests/integration.rs` pattern.

**Tests to write:**
1. `acp_initialize` — send `acp/initialize` with mock server config, verify response
2. `acp_generate` — full generate lifecycle (requires mock agent process)
3. `acp_stream` — stream start → delta notifications → complete
4. `acp_session_crud` — create, list, load, delete sessions
5. `acp_server_health` — verify health endpoint
6. `acp_permission_callback` — permission request → response round-trip
7. `acp_dispose` — clean shutdown
8. `unknown_method` — returns METHOD_NOT_FOUND

**Mock agent process:** Create a simple Rust binary in `tests/mock_agent.rs` (or use a test helper) that implements the minimal ACP agent protocol:
- Responds to `initialize` with agent info
- Responds to `session/new` with session ID
- Responds to `session/prompt` with canned text (optionally with streaming notifications)
- Responds to `session/request_permission` (for permission tests)

**Step 1: Create mock agent helper**

**Step 2: Write integration tests**

**Step 3: Verify all pass**

Run: `cd simse-acp && cargo test`

**Step 4: Commit**

```bash
git commit -m "test(acp): add Rust integration tests"
```

---

### Task 11: TS thin client for ACP

**Files:**
- Create: `src/ai/acp/acp-engine-client.ts` — JSON-RPC client to simse-acp engine
- Modify: `src/ai/acp/acp-client.ts` — replace internals, keep public interface
- Delete: `src/ai/acp/acp-connection.ts` — replaced by Rust
- Delete: `src/ai/acp/acp-results.ts` — parsing moved to Rust
- Modify: `src/ai/acp/acp-adapters.ts` — simplify to use thin client
- Modify: `src/ai/acp/index.ts` — update exports
- Modify: `src/lib.ts` — update exports (remove acp-connection, acp-results)

**Step 1: Create acp-engine-client.ts**

Follow the `library/client.ts` pattern exactly. Spawn `simse-acp-engine` binary, communicate via JSON-RPC 2.0 / NDJSON stdio.

```typescript
export interface AcpEngineClient {
    readonly request: <T>(method: string, params?: unknown) => Promise<T>;
    readonly onNotification: (method: string, handler: (params: unknown) => void) => () => void;
    readonly dispose: () => Promise<void>;
    readonly isHealthy: boolean;
}

export function createAcpEngineClient(options: { enginePath: string; logger?: Logger }): AcpEngineClient
```

Key addition over the vector client: `onNotification()` for streaming and permission callbacks.

**Step 2: Rewrite acp-client.ts**

Replace the internals of `createACPClient()`. The public interface (`ACPClient`) stays identical. Internally:
- Spawns `simse-acp-engine` via `createAcpEngineClient()`
- `initialize()` → `engineClient.request('acp/initialize', { servers, ... })`
- `generate()` → `engineClient.request('acp/generate', { prompt, ... })`
- `generateStream()` → calls `engineClient.request('acp/streamStart', { ... })`, then yields from notification handler
- `embed()` → `engineClient.request('acp/embed', { ... })`
- Permission handling: register notification handler for `permission/request`, forward to `onPermissionRequest` callback, respond with `acp/permissionResponse`
- All resilience (circuit breaker, retry, health) is now in Rust

The file should shrink from ~1,200 lines to ~300 lines.

**Step 3: Simplify acp-adapters.ts**

Remove direct imports from acp-connection.ts. The adapters just wrap the ACPClient interface (unchanged).

**Step 4: Delete acp-connection.ts and acp-results.ts**

**Step 5: Update index.ts and lib.ts exports**

Remove exports for `ACPConnection`, `ACPConnectionOptions`, `createACPConnection`, `extractToolCall`, `extractToolCallUpdate` (these were internal plumbing, now in Rust).

**Important:** If any consumer imports `createACPConnection` or extract functions directly, add re-exports that delegate to the engine or provide compatibility shims.

**Step 6: Verify typecheck**

Run: `bun run typecheck`

**Step 7: Commit**

```bash
git commit -m "refactor(acp): replace TS internals with thin client over Rust engine"
```

---

### Task 12: TS E2E tests for ACP

**Files:**
- Create: `tests/acp/acp-engine.test.ts`
- Modify: existing ACP test files to work with new client

**Step 1: Write E2E tests**

Tests that spawn the real `simse-acp-engine` binary and verify the full lifecycle through the TS thin client:
1. Initialize → verify serverNames
2. Generate → verify content returned
3. Stream → verify delta + complete chunks
4. Embed → verify embeddings array
5. Session CRUD → list, load, delete
6. Permission callback round-trip
7. Dispose → clean shutdown

These require a mock agent process (same one from Task 10, or a separate test fixture).

**Step 2: Verify existing tests still pass**

Run: `bun test`

All existing tests that mock ACPClient should still pass since the interface is unchanged. Tests that directly use `createACPConnection` need updating.

**Step 3: Commit**

```bash
git commit -m "test(acp): add TS E2E tests for ACP Rust engine"
```

---

## Phase B: simse-mcp Rust Crate

### Task 13: Scaffold simse-mcp crate

**Files:**
- Create: `simse-mcp/Cargo.toml`
- Create: `simse-mcp/src/lib.rs`
- Create: `simse-mcp/src/main.rs`
- Create: `simse-mcp/src/error.rs`

Same pattern as Task 1 but for MCP. Additional dependency: `reqwest` (for HTTP transport).

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
uuid = { version = "1", features = ["v4"] }
tokio = { version = "1", features = ["full"] }
futures = "0.3"
reqwest = { version = "0.12", features = ["json"] }
```

`McpError` variants:
- `NotInitialized`
- `ConnectionFailed(String)`
- `ServerNotConnected(String)`
- `ToolError { tool: String, message: String }`
- `ResourceError { uri: String, message: String }`
- `TransportConfigError(String)`
- `Timeout { method: String, timeout_ms: u64 }`
- `CircuitBreakerOpen(String)`
- `ProtocolError(String)`
- `Io(#[from] std::io::Error)`
- `Serialization(String)`

Error codes prefixed with `MCP_`.

**Commit:**
```bash
git commit -m "feat(mcp): scaffold simse-mcp Rust crate"
```

---

### Task 14: MCP protocol types

**Files:**
- Create: `simse-mcp/src/protocol.rs`
- Modify: `simse-mcp/src/lib.rs`

Define all MCP protocol types. Reference the MCP specification and `src/ai/mcp/types.ts`.

**JSON-RPC base types** (same as ACP: `JsonRpcRequest`, error codes).

**MCP-specific types:**
- `InitializeParams` — `{ protocol_version, capabilities, client_info }`
- `InitializeResult` — `{ protocol_version, capabilities, server_info }`
- `ToolInfo` — `{ name, description?, input_schema, annotations? }`
- `ToolAnnotations` — `{ title?, read_only_hint?, destructive_hint?, idempotent_hint?, open_world_hint? }`
- `ToolCallParams` — `{ name, arguments }`
- `ToolCallResult` — `{ content: Vec<ContentItem>, is_error? }`
- `ContentItem` — `{ type: "text", text }` (extensible)
- `ResourceInfo` — `{ uri, name, description?, mime_type? }`
- `ResourceTemplateInfo` — `{ uri_template, name, description?, mime_type? }`
- `ReadResourceParams` — `{ uri }`
- `ReadResourceResult` — `{ contents: Vec<ResourceContent> }`
- `ResourceContent` — `{ uri, text?, blob?, mime_type? }`
- `PromptInfo` — `{ name, description?, arguments? }`
- `PromptArgument` — `{ name, description?, required? }`
- `GetPromptParams` — `{ name, arguments }`
- `GetPromptResult` — `{ messages: Vec<PromptMessage> }`
- `PromptMessage` — `{ role, content }`
- `LoggingMessage` — `{ level, logger?, data }`
- `CompletionRef` — enum `ResourceRef { uri }` or `PromptRef { name }`
- `CompletionArg` — `{ name, value }`
- `CompletionResult` — `{ values, has_more?, total? }`
- `Root` — `{ uri, name? }`
- `ServerConnection` — `{ name, transport, command?, args?, env?, url? }`
- `McpClientConfig` — `{ servers, client_name?, client_version?, circuit_breaker? }`
- `McpServerConfig` — `{ name, version, transport }`

**Commit:**
```bash
git commit -m "feat(mcp): add MCP protocol types"
```

---

### Task 15: MCP transports (stdio + HTTP)

**Files:**
- Create: `simse-mcp/src/stdio_transport.rs`
- Create: `simse-mcp/src/http_transport.rs`
- Modify: `simse-mcp/src/lib.rs`

**Step 1: Define Transport trait**

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn request<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T>;
    fn on_notification(&self, method: &str, handler: NotificationHandler) -> SubscriptionHandle;
    async fn close(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
}
```

**Step 2: Implement StdioTransport**

Spawns a child process, communicates via JSON-RPC 2.0 / NDJSON stdio. Very similar to `AcpConnection` from Task 4 but without ACP-specific features (no permissions, no streaming state machine).

- Spawn process with command/args/env
- Send `initialize` request on connect
- Track pending requests by ID
- Route notifications to handlers
- Health check: process alive

**Step 3: Implement HttpTransport**

Connects to an HTTP MCP server using `reqwest`.

- URL validation (http/https only)
- POST requests with JSON-RPC body
- Parse JSON-RPC responses
- No notification support (HTTP is request-response only, unless using SSE)

**Step 4: Write unit tests**

- StdioTransport: spawn mock server, send request, verify response
- HttpTransport: URL validation (reject non-http), mock HTTP endpoint

**Step 5: Commit**

```bash
git commit -m "feat(mcp): add stdio and HTTP transports"
```

---

### Task 16: MCP client

**Files:**
- Create: `simse-mcp/src/client.rs`
- Modify: `simse-mcp/src/lib.rs`

**Step 1: Implement McpClient**

```rust
pub struct McpClient {
    connections: HashMap<String, Box<dyn Transport>>,
    connecting: Mutex<HashMap<String, tokio::sync::broadcast::Sender<Result<()>>>>,
    circuit_breakers: HashMap<String, CircuitBreaker>,
    health_monitors: HashMap<String, HealthMonitor>,
    notification_handlers: NotificationRegistry,
    config: McpClientConfig,
    roots: Mutex<Vec<Root>>,
}
```

Public API (matches design doc):
- `async fn new(config: McpClientConfig) -> Result<Self>`
- `async fn connect(&mut self, server_name: &str) -> Result<()>` — with deduplication
- `async fn connect_all(&mut self) -> Result<Vec<String>>`
- `async fn disconnect(&mut self, server_name: &str) -> Result<()>`
- `async fn disconnect_all(&mut self) -> Result<()>`
- `async fn list_tools(&self, server: Option<&str>) -> Result<Vec<ToolInfo>>`
- `async fn call_tool(&self, server: &str, tool: &str, args: Value) -> Result<ToolResult>`
- `async fn list_resources(&self, server: Option<&str>) -> Result<Vec<ResourceInfo>>`
- `async fn read_resource(&self, server: &str, uri: &str) -> Result<String>`
- `async fn list_resource_templates(&self, server: Option<&str>) -> Result<Vec<ResourceTemplateInfo>>`
- `async fn list_prompts(&self, server: Option<&str>) -> Result<Vec<PromptInfo>>`
- `async fn get_prompt(&self, server: &str, name: &str, args: Value) -> Result<String>`
- `async fn set_logging_level(&self, server: &str, level: &str) -> Result<()>`
- `fn on_tools_changed(&self, handler) -> SubscriptionHandle`
- `fn on_logging_message(&self, handler) -> SubscriptionHandle`
- `async fn complete(&self, server: &str, reference, argument) -> Result<CompletionResult>`
- `fn set_roots(&mut self, roots: Vec<Root>)`
- `fn is_available(&self, server: Option<&str>) -> bool`
- `fn connected_server_names(&self) -> Vec<String>`

**Key behaviors:**
- Connection deduplication via broadcast channel
- Retry on tool calls (2 attempts, 500ms base delay)
- Circuit breaker + health monitor per server
- List operations aggregate across all connected servers when `server` is None
- Notification handlers registered before connect, fired when server sends list-changed

**Step 2: Reuse resilience from simse-acp** — consider extracting into shared crate, or duplicate for now

**Step 3: Write unit tests**

- Connection deduplication (concurrent connect() calls)
- Tool call retry on transient error
- Multi-server tool aggregation
- Circuit breaker integration

**Step 4: Commit**

```bash
git commit -m "feat(mcp): add MCP client with connection pool and resilience"
```

---

### Task 17: MCP server

**Files:**
- Create: `simse-mcp/src/mcp_server.rs`
- Modify: `simse-mcp/src/lib.rs`

The MCP server hosts tools/resources/prompts for external clients over stdio.

**Step 1: Implement McpServer**

```rust
pub struct McpServer {
    config: McpServerConfig,
    tools: HashMap<String, RegisteredTool>,
    resources: HashMap<String, RegisteredResource>,
    prompts: HashMap<String, RegisteredPrompt>,
    transport: Option<StdioServerTransport>,
}

struct RegisteredTool {
    definition: ToolDefinition,
    handler: Box<dyn ToolHandler>,
}
```

Public API:
- `async fn new(config: McpServerConfig) -> Result<Self>`
- `async fn start(&mut self) -> Result<()>` — start listening on stdin
- `async fn stop(&mut self) -> Result<()>`
- `fn register_tool(&mut self, def: ToolDefinition, handler: impl ToolHandler + 'static)`
- `fn unregister_tool(&mut self, name: &str)`
- `fn send_tool_list_changed(&self)`
- `fn register_resource(&mut self, def: ResourceDefinition, handler: impl ResourceHandler + 'static)`
- `fn send_resource_list_changed(&self)`
- `fn register_prompt(&mut self, def: PromptDefinition, handler: impl PromptHandler + 'static)`

**StdioServerTransport:** Reads JSON-RPC requests from stdin, dispatches to registered handlers, writes responses to stdout. Handles:
- `initialize` → return server info + capabilities
- `tools/list` → return registered tools
- `tools/call` → invoke handler, return result
- `resources/list` → return registered resources
- `resources/read` → invoke handler, return content
- `prompts/list` → return registered prompts
- `prompts/get` → invoke handler, return messages
- `ping` → return `{}`
- `logging/setLevel` → set level
- `completion/complete` → placeholder
- `roots/list` → return roots

**Step 2: Define handler traits**

```rust
#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn execute(&self, args: Value) -> Result<ToolCallResult>;
}

#[async_trait]
pub trait ResourceHandler: Send + Sync {
    async fn read(&self, uri: &str) -> Result<String>;
}

#[async_trait]
pub trait PromptHandler: Send + Sync {
    async fn get(&self, args: Value) -> Result<Vec<PromptMessage>>;
}
```

**Step 3: Write unit tests**

- Tool registration and listing
- Tool call dispatches to correct handler
- Resource read dispatches correctly
- Unknown method returns METHOD_NOT_FOUND
- List-changed notifications sent

**Step 4: Commit**

```bash
git commit -m "feat(mcp): add MCP server with tool/resource/prompt registration"
```

---

### Task 18: MCP JSON-RPC wrapper (main.rs transitional server)

**Files:**
- Create: `simse-mcp/src/rpc_server.rs`
- Create: `simse-mcp/src/rpc_transport.rs`
- Modify: `simse-mcp/src/main.rs`
- Modify: `simse-mcp/src/lib.rs`

The transitional JSON-RPC server for TS interop. Wraps both McpClient and McpServer.

**Step 1: Create rpc_transport.rs**

Copy `NdjsonTransport` from simse-vector (same as Task 3).

**Step 2: Implement McpRpcServer**

Dispatch methods:
- `mcp/initialize` → create McpClient and/or McpServer from config
- `mcp/connect`, `mcp/connectAll`, `mcp/disconnect` → client methods
- `mcp/listTools`, `mcp/callTool` → client methods
- `mcp/listResources`, `mcp/readResource`, `mcp/listResourceTemplates` → client methods
- `mcp/listPrompts`, `mcp/getPrompt` → client methods
- `mcp/setLoggingLevel`, `mcp/complete`, `mcp/setRoots` → client methods
- `server/start`, `server/stop` → server methods
- `server/registerTool`, `server/unregisterTool` → server methods (tool handler callbacks to TS)
- `mcp/dispose` → cleanup

**Tool handler callbacks:** When `server/registerTool` is called, the Rust server registers a `CallbackToolHandler` that, when invoked by an external MCP client, sends a `tool/execute { requestId, toolName, args }` notification to TS and waits for `server/toolResult { requestId, content, isError }`.

**Notification forwarding:** MCP client notifications (tools-changed, resources-changed, logging) forwarded as NDJSON notifications to TS.

**Step 3: Update main.rs**

```rust
#[tokio::main]
async fn main() {
    // tracing setup...
    let transport = NdjsonTransport::new();
    let mut server = McpRpcServer::new(transport);
    tracing::info!("simse-mcp-engine ready");
    if let Err(e) = server.run().await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

**Step 4: Verify build, commit**

```bash
git commit -m "feat(mcp): add JSON-RPC wrapper server"
```

---

### Task 19: MCP Rust integration tests

**Files:**
- Create: `simse-mcp/tests/integration.rs`

**Tests:**
1. `mcp_initialize` — send `mcp/initialize`, verify response
2. `mcp_connect_and_list_tools` — connect to mock MCP server, list tools
3. `mcp_call_tool` — call tool on mock server, verify result
4. `mcp_multi_server` — connect to 2 mock servers, list tools from each
5. `mcp_server_register_tool` — register tool, verify listed by mock client
6. `mcp_server_tool_callback` — register tool, invoke from mock client, verify callback flow
7. `mcp_disconnect` — connect then disconnect, verify not available
8. `unknown_method` — returns METHOD_NOT_FOUND

**Mock MCP server:** Simple Rust binary implementing minimal MCP server protocol (initialize, tools/list, tools/call).

**Commit:**
```bash
git commit -m "test(mcp): add Rust integration tests"
```

---

### Task 20: TS thin client for MCP

**Files:**
- Create: `src/ai/mcp/mcp-engine-client.ts` — JSON-RPC client to simse-mcp engine
- Modify: `src/ai/mcp/mcp-client.ts` — replace internals, keep public interface
- Modify: `src/ai/mcp/mcp-server.ts` — replace internals, keep public interface
- Modify: `src/ai/mcp/index.ts` — update exports
- Modify: `src/lib.ts` — update exports
- Modify: `package.json` — remove `@modelcontextprotocol/sdk` dependency

**Step 1: Create mcp-engine-client.ts**

Same pattern as `acp-engine-client.ts` (Task 11). Spawn `simse-mcp-engine`, communicate via JSON-RPC + notifications.

**Step 2: Rewrite mcp-client.ts**

Replace internals of `createMCPClient()`. Public interface stays identical. Internally:
- `connect()` → `engineClient.request('mcp/connect', { serverName })`
- `listTools()` → `engineClient.request('mcp/listTools', { serverName })`
- `callTool()` → `engineClient.request('mcp/callTool', { serverName, toolName, args })`
- Notification handlers: register via `engineClient.onNotification('mcp/toolsChanged', ...)`
- All resilience is now in Rust

File shrinks from ~930 lines to ~200 lines.

**Step 3: Rewrite mcp-server.ts**

Replace internals of `createMCPServer()`. Public interface stays identical. Internally:
- `start()` → `engineClient.request('server/start', {})`
- Tool registration: `engineClient.request('server/registerTool', { name, description, inputSchema })`
- Tool handler callback: `engineClient.onNotification('tool/execute', handler)` → execute local handler → `engineClient.request('server/toolResult', { requestId, content, isError })`

File shrinks from ~1,060 lines to ~250 lines.

**Step 4: Remove @modelcontextprotocol/sdk from package.json**

**Step 5: Update exports**

**Step 6: Verify typecheck**

Run: `bun run typecheck`

**Step 7: Commit**

```bash
git commit -m "refactor(mcp): replace TS internals with thin client over Rust engine"
```

---

### Task 21: TS E2E tests for MCP

**Files:**
- Create: `tests/mcp/mcp-engine.test.ts`

**Tests:**
1. Connect to mock MCP server via engine, list tools
2. Call tool via engine, verify result
3. Register tool on MCP server, verify discoverable
4. Tool handler callback round-trip (external client calls registered tool)
5. Multi-server connection and disconnection
6. Notification forwarding (tools-changed fires handler)

**Commit:**
```bash
git commit -m "test(mcp): add TS E2E tests for MCP Rust engine"
```

---

## Phase C: Integration & Cleanup

### Task 22: Update build scripts and config

**Files:**
- Modify: `package.json` — add `build:acp-engine` and `build:mcp-engine` scripts, remove `@modelcontextprotocol/sdk`
- Modify: `CLAUDE.md` — add simse-acp and simse-mcp to architecture docs

**Step 1: Update package.json**

Add scripts:
```json
"build:acp-engine": "cd simse-acp && cargo build --release",
"build:mcp-engine": "cd simse-mcp && cargo build --release"
```

Remove from dependencies:
```json
"@modelcontextprotocol/sdk": "^1.12.0"
```

**Step 2: Update CLAUDE.md**

Add `simse-acp/` and `simse-mcp/` to the repository layout, module layout, and architecture sections.

**Step 3: Commit**

```bash
git commit -m "chore: update build scripts and docs for ACP/MCP Rust crates"
```

---

### Task 23: Final verification

**Step 1: Build all Rust crates**

```bash
cd simse-vector && cargo build && cargo test
cd ../simse-vfs && cargo build && cargo test
cd ../simse-acp && cargo build && cargo test
cd ../simse-mcp && cargo build && cargo test
```

**Step 2: Run all TS tests**

```bash
bun test
```

**Step 3: Typecheck and lint**

```bash
bun run typecheck
bun run lint
```

**Step 4: Verify no `@modelcontextprotocol/sdk` imports remain**

```bash
grep -r "modelcontextprotocol" src/ tests/
```

Expected: No results.

**Step 5: Commit any fixups**

```bash
git commit -m "chore: final verification and cleanup"
```
