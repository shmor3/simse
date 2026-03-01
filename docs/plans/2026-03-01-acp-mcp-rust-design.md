# ACP & MCP Rust Crates Design

## Overview

Extract the ACP (Agent Client Protocol) and MCP (Model Context Protocol) implementations from TypeScript into two new pure Rust crates. Each crate provides a library API (`lib.rs`) as the permanent interface and a transitional JSON-RPC binary wrapper (`main.rs`) for interop with existing TS code during migration. The long-term goal is a fully Rust codebase with no TypeScript/Bun.

## Repository Structure

```
simse-acp/                    # NEW -- Pure Rust crate
  Cargo.toml
  src/
    lib.rs                    # Library API (permanent interface)
    main.rs                   # Transitional JSON-RPC subprocess wrapper
    client.rs                 # ACP client: session mgmt, generate, embed
    connection.rs             # JSON-RPC 2.0 transport to agent process
    stream.rs                 # Streaming state machine (chunks, timeouts)
    permission.rs             # Permission request handling
    resilience.rs             # Circuit breaker + retry + health monitor
    protocol.rs               # ACP protocol message types
    transport.rs              # NDJSON stdio framing (for main.rs wrapper)
    server.rs                 # JSON-RPC dispatcher (for main.rs wrapper)
    error.rs                  # Error types
  tests/
    integration.rs

simse-mcp/                    # NEW -- Pure Rust crate
  Cargo.toml
  src/
    lib.rs                    # Library API (permanent interface)
    main.rs                   # Transitional JSON-RPC subprocess wrapper
    client.rs                 # MCP client: connect to external servers
    server.rs                 # MCP server: host tools/resources/prompts
    tool.rs                   # Tool registry + execution
    resource.rs               # Resource handling
    prompt.rs                 # Prompt templates
    stdio_transport.rs        # Stdio client/server transport
    http_transport.rs         # HTTP/SSE client transport
    protocol.rs               # MCP protocol message types
    transport.rs              # NDJSON framing (for main.rs wrapper)
    rpc_server.rs             # JSON-RPC dispatcher (for main.rs wrapper)
    error.rs
  tests/
    integration.rs

src/ai/acp/                   # MODIFIED -- thin TS client (~250 lines)
  client.ts                   # Spawns simse-acp, delegates via JSON-RPC
  types.ts                    # TS type definitions (kept)
  index.ts                    # Barrel export

src/ai/mcp/                   # MODIFIED -- thin TS client (~300 lines)
  client.ts                   # Spawns simse-mcp, delegates via JSON-RPC
  server.ts                   # Thin wrapper for tool handler callbacks
  types.ts                    # TS type definitions (kept)
  index.ts                    # Barrel export
```

### Deleted TS Files

**ACP (replaced by Rust):**
- `acp-connection.ts` -- replaced by `connection.rs`
- `acp-results.ts` -- parsing moves to Rust

**MCP (replaced by Rust):**
- `mcp-client.ts` (bulk) -- replaced by `client.rs`
- `mcp-server.ts` (bulk) -- replaced by `server.rs`

**Removed dependency:** `@modelcontextprotocol/sdk` removed from `package.json`.

**Kept:** `acp-adapters.ts` simplified to wrap the thin client.

## simse-acp Rust Crate

### Library API

```rust
pub struct AcpClient { ... }

impl AcpClient {
    // Lifecycle
    pub async fn new(config: AcpConfig) -> Result<Self>
    pub async fn dispose(&mut self) -> Result<()>

    // Generation
    pub async fn generate(&self, prompt: &str, options: GenerateOptions) -> Result<GenerateResult>
    pub async fn chat(&self, messages: &[ChatMessage], options: ChatOptions) -> Result<GenerateResult>
    pub fn generate_stream(&self, prompt: &str, options: StreamOptions) -> impl Stream<Item = StreamChunk>
    pub async fn embed(&self, input: &[&str], model: Option<&str>, server: Option<&str>) -> Result<EmbedResult>

    // Agent discovery
    pub async fn list_agents(&self, server: Option<&str>) -> Result<Vec<AgentInfo>>

    // Session management
    pub async fn list_sessions(&self, server: Option<&str>) -> Result<Vec<SessionListEntry>>
    pub async fn load_session(&self, session_id: &str, server: Option<&str>) -> Result<SessionInfo>
    pub async fn delete_session(&self, session_id: &str, server: Option<&str>) -> Result<()>
    pub async fn set_session_mode(&self, session_id: &str, mode_id: &str, server: Option<&str>) -> Result<()>
    pub async fn set_session_model(&self, session_id: &str, model_id: &str, server: Option<&str>) -> Result<()>

    // Server health
    pub fn is_available(&self, server: Option<&str>) -> bool
    pub fn server_names(&self) -> &[String]
}
```

### Internals

- **Connection pool** -- one `AcpConnection` per configured server, manages subprocess spawning
- **Circuit breaker** -- per-server failure isolation with configurable thresholds
- **Health monitor** -- sliding-window success/failure tracking
- **Retry with backoff** -- exponential backoff + jitter, transient error detection
- **Streaming state machine** -- chunk buffering, sliding-window timeouts, permission activity keepalives
- **Permission handling** -- intercepts `session/request_permission`, applies policy (auto-approve/prompt/deny)
- **NDJSON framing** -- buffer management for partial lines from agent subprocess

### Dependencies

- `tokio` -- async runtime, process spawning, timers, channels
- `serde` / `serde_json` -- JSON serialization
- `futures` -- Stream trait
- `tracing` -- structured logging

## simse-mcp Rust Crate

### Library API

```rust
// ---- MCP Client ----
pub struct McpClient { ... }

impl McpClient {
    pub async fn new(config: McpClientConfig) -> Result<Self>
    pub async fn connect(&mut self, server_name: &str) -> Result<()>
    pub async fn connect_all(&mut self) -> Result<Vec<String>>
    pub async fn disconnect(&mut self, server_name: &str) -> Result<()>
    pub async fn disconnect_all(&mut self) -> Result<()>

    // Tools
    pub async fn list_tools(&self, server: Option<&str>) -> Result<Vec<ToolInfo>>
    pub async fn call_tool(&self, server: &str, tool: &str, args: Value) -> Result<ToolResult>

    // Resources
    pub async fn list_resources(&self, server: Option<&str>) -> Result<Vec<ResourceInfo>>
    pub async fn read_resource(&self, server: &str, uri: &str) -> Result<String>
    pub async fn list_resource_templates(&self, server: Option<&str>) -> Result<Vec<ResourceTemplateInfo>>

    // Prompts
    pub async fn list_prompts(&self, server: Option<&str>) -> Result<Vec<PromptInfo>>
    pub async fn get_prompt(&self, server: &str, name: &str, args: Value) -> Result<String>

    // Logging & notifications
    pub async fn set_logging_level(&self, server: &str, level: &str) -> Result<()>
    pub fn on_tools_changed(&self, handler: impl Fn() + Send + 'static) -> SubscriptionHandle
    pub fn on_logging_message(&self, handler: impl Fn(LoggingMessage) + Send + 'static) -> SubscriptionHandle

    // Completions & roots
    pub async fn complete(&self, server: &str, reference: CompletionRef, argument: CompletionArg) -> Result<CompletionResult>
    pub fn set_roots(&mut self, roots: Vec<Root>)

    // Health
    pub fn is_available(&self, server: Option<&str>) -> bool
    pub fn connected_server_names(&self) -> Vec<String>
}

// ---- MCP Server ----
pub struct McpServer { ... }

impl McpServer {
    pub async fn new(config: McpServerConfig) -> Result<Self>
    pub async fn start(&mut self) -> Result<()>
    pub async fn stop(&mut self) -> Result<()>

    // Tool registration
    pub fn register_tool(&mut self, definition: ToolDefinition, handler: impl ToolHandler)
    pub fn unregister_tool(&mut self, name: &str)
    pub fn send_tool_list_changed(&self)

    // Resource registration
    pub fn register_resource(&mut self, definition: ResourceDefinition, handler: impl ResourceHandler)
    pub fn send_resource_list_changed(&self)

    // Prompt registration
    pub fn register_prompt(&mut self, definition: PromptDefinition, handler: impl PromptHandler)
}

pub trait ToolHandler: Send + Sync {
    fn execute(&self, args: Value) -> BoxFuture<Result<ToolResult>>;
}
```

### Internals

- **Client transports** -- stdio (spawn child process) and HTTP/SSE (connect to URL), both implementing a shared `Transport` trait
- **Server transport** -- stdio (listen on stdin/stdout)
- **Connection deduplication** -- in-flight future tracking per server
- **Circuit breaker + health monitor** -- per-server, same pattern as ACP
- **Retry** -- exponential backoff on tool calls and resource reads
- **Notification dispatch** -- handler registries for tools-changed, resources-changed, logging
- **JSON-RPC 2.0 protocol** -- full implementation (requests, responses, notifications, error codes)

### Dependencies

- `tokio` -- async runtime, process, timers, net
- `serde` / `serde_json` -- JSON
- `hyper` or `reqwest` -- HTTP client transport
- `futures` -- Stream trait, BoxFuture
- `tracing` -- logging

## Transitional JSON-RPC Protocol

### simse-acp Engine Methods

| Method | Params | Returns |
|--------|--------|---------|
| `acp/initialize` | `{ servers, defaultServer?, defaultAgent?, mcpServers? }` | `{ serverNames }` |
| `acp/generate` | `{ prompt, agentId?, serverName?, systemPrompt?, sampling? }` | `{ content, agentId, serverName, sessionId, usage?, stopReason? }` |
| `acp/chat` | `{ messages, agentId?, serverName?, sampling? }` | Same as generate |
| `acp/streamStart` | `{ prompt, agentId?, serverName?, systemPrompt?, sampling? }` | `{ streamId }` |
| `acp/embed` | `{ input, model?, serverName? }` | `{ embeddings, agentId, serverName, usage? }` |
| `acp/listAgents` | `{ serverName? }` | `{ agents }` |
| `acp/listSessions` | `{ serverName? }` | `{ sessions }` |
| `acp/loadSession` | `{ sessionId, serverName? }` | `{ session }` |
| `acp/deleteSession` | `{ sessionId, serverName? }` | `{}` |
| `acp/setSessionMode` | `{ sessionId, modeId, serverName? }` | `{}` |
| `acp/setSessionModel` | `{ sessionId, modelId, serverName? }` | `{}` |
| `acp/setPermissionPolicy` | `{ policy }` | `{}` |
| `acp/serverHealth` | `{ serverName? }` | `{ available, health? }` |
| `acp/dispose` | `{}` | `{}` |

#### Streaming

`acp/streamStart` returns a `streamId`. The Rust engine emits NDJSON notifications:
- `{ method: "stream/delta", params: { streamId, text } }`
- `{ method: "stream/toolCall", params: { streamId, toolCall } }`
- `{ method: "stream/toolCallUpdate", params: { streamId, update } }`
- `{ method: "stream/complete", params: { streamId, usage? } }`

#### Permission Callbacks

When the agent subprocess requests permission, the Rust engine emits:
`{ method: "permission/request", params: { requestId, description, options } }`

TS responds with: `acp/permissionResponse { requestId, optionId }`

### simse-mcp Engine Methods

| Method | Params | Returns |
|--------|--------|---------|
| `mcp/initialize` | `{ clientConfig?, serverConfig? }` | `{}` |
| `mcp/connect` | `{ serverName }` | `{}` |
| `mcp/connectAll` | `{}` | `{ connected }` |
| `mcp/disconnect` | `{ serverName }` | `{}` |
| `mcp/listTools` | `{ serverName? }` | `{ tools }` |
| `mcp/callTool` | `{ serverName, toolName, args }` | `{ content, isError }` |
| `mcp/listResources` | `{ serverName? }` | `{ resources }` |
| `mcp/readResource` | `{ serverName, uri }` | `{ content }` |
| `mcp/listPrompts` | `{ serverName? }` | `{ prompts }` |
| `mcp/getPrompt` | `{ serverName, promptName, args }` | `{ content }` |
| `mcp/setLoggingLevel` | `{ serverName, level }` | `{}` |
| `mcp/complete` | `{ serverName, ref, argument }` | `{ completions }` |
| `mcp/setRoots` | `{ roots }` | `{}` |
| `server/start` | `{}` | `{}` |
| `server/stop` | `{}` | `{}` |
| `server/registerTool` | `{ name, description, inputSchema }` | `{}` |
| `server/unregisterTool` | `{ name }` | `{}` |
| `mcp/dispose` | `{}` | `{}` |

#### Server Tool Callbacks

When an external MCP client invokes a registered tool, the Rust engine emits:
`{ method: "tool/execute", params: { requestId, toolName, args } }`

TS responds with: `server/toolResult { requestId, content, isError }`

#### Notification Forwarding

MCP notifications (tools-changed, resources-changed, logging) forwarded as NDJSON notifications to TS.

## Testing

### simse-acp Rust Tests

**Unit tests (in each module):**
- `connection.rs` -- NDJSON buffer parsing, message routing, partial line handling
- `stream.rs` -- chunk buffering, timeout reset on activity, stream completion
- `permission.rs` -- auto-approve/deny/prompt policy selection, timeout suspension
- `resilience.rs` -- circuit breaker state transitions, health window tracking, retry backoff calculation
- `protocol.rs` -- JSON-RPC request/response/notification serialization round-trips

**Integration tests (`tests/integration.rs`):**
- Spawn mock agent process, send `acp/initialize`, verify handshake
- Generate request -> response lifecycle
- Stream start -> delta notifications -> complete
- Permission request -> response -> stream resumes
- Circuit breaker opens after N failures, rejects subsequent calls
- Session CRUD: create, list, load, delete
- Process exit -> pending requests rejected
- Timeout fires when agent process hangs

### simse-mcp Rust Tests

**Unit tests:**
- `stdio_transport.rs` -- frame parsing, child process spawning
- `http_transport.rs` -- URL validation, HTTP request/response handling
- `tool.rs` -- tool registration, schema validation, handler dispatch
- `resource.rs` -- resource listing, URI matching
- `protocol.rs` -- MCP protocol message serialization

**Integration tests:**
- Spawn mock MCP server, connect client, list tools, call tool
- HTTP transport: connect to mock HTTP server, call tool
- Multi-server: connect to 2 servers, list tools from each, disconnect one
- Server mode: start server, register tool, receive tool call from mock client, return result
- Notification forwarding: server sends tools-changed, client handler fires
- Circuit breaker: server returns errors, breaker opens
- Connection deduplication: concurrent connect() calls resolve to single connection

### TypeScript E2E Tests

- Spawn simse-acp engine, full generate/stream/embed cycle through JSON-RPC
- Spawn simse-mcp engine, connect/listTools/callTool through JSON-RPC
- MCP server mode: register tool via JSON-RPC, verify external client can invoke it
- Permission callback round-trip: engine sends permission request, TS responds
- Verify existing consumers (loop, tools, chain) still work with the thin client

### Zero Regression

All existing tests in `tests/` continue to pass since the public API interfaces don't change.

## Migration Path

1. **Phase 1 (this design):** Build simse-acp and simse-mcp Rust crates. TS becomes thin clients.
2. **Phase 2:** Migrate loop/, tools/, chain/, agent/ to Rust. They import simse-acp/simse-mcp as Rust libraries directly.
3. **Phase 3:** Migrate conversation/, tasks/, prompts/ to Rust. Pure logic modules, straightforward port.
4. **Phase 4:** Migrate library/ and vfs/ orchestration layers to Rust (stacks/client already in Rust, high-level API moves too).
5. **Phase 5:** Delete all TypeScript. Single Rust binary links all crates. Remove JSON-RPC wrappers (`main.rs` files become optional CLI tools or get deleted).
