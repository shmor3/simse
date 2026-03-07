# simse-mcp Refactoring to rmcp SDK

**Date:** 2026-03-06
**Status:** Approved

## Goal

Replace custom MCP protocol types, transports, and client/server internals in simse-mcp with the official `rmcp` crate (v1.1.0). Keep the resilience layer, JSON-RPC dispatcher, and callback tool pattern.

## Decisions

- **Keep resilience layer** (circuit breaker, health monitor, retry) as a wrapper around rmcp client calls
- **Keep JSON-RPC dispatcher** as the simse-core <-> simse-mcp bridge
- **Keep callback tool pattern** for dynamic tool registration from TS side
- **Upgrade to protocol version 2025-06-18** (latest)

## Architecture

```
simse-core (spawns) -> simse-mcp-engine binary
                        |-- rpc_server.rs      (JSON-RPC bridge, kept + adapted)
                        |-- rpc_transport.rs    (NDJSON stdio to simse-core, kept as-is)
                        |-- client.rs           (multi-server manager wrapping rmcp RunningService)
                        |-- resilience.rs       (circuit breaker, health monitor, retry - extracted)
                        |-- mcp_server.rs       (ServerHandler impl with callback tool pattern)
                        |-- error.rs            (adapted - maps rmcp errors to MCP_ codes)
                        |-- main.rs             (kept as-is)
                        |-- lib.rs              (updated module declarations)
```

**Deleted:** `protocol.rs`, `stdio_transport.rs`, `http_transport.rs`

## Client Layer

Each connected server is an rmcp `RunningService`:

```rust
struct ServerConnection {
    service: RunningService<RoleClient, ClientHandler>,
    config: McpServerConfig,
    circuit_breaker: CircuitBreaker,
    health_monitor: HealthMonitor,
}

struct McpClient {
    servers: HashMap<String, ServerConnection>,
    retry_config: RetryConfig,
}
```

Connection flow:
1. Build rmcp transport (TokioChildProcess for stdio, StreamableHttpClientTransport for HTTP)
2. Call `ClientHandler::new().serve(transport).await` -> RunningService
3. Store in servers map with circuit breaker + health monitor

All tool/resource/prompt calls go through the resilience wrapper before hitting `service.peer()`.

## Server Layer

Implements rmcp `ServerHandler` with dynamic registration:

```rust
struct SimseServer {
    tools: Arc<RwLock<HashMap<String, RegisteredTool>>>,
    resources: Arc<RwLock<HashMap<String, RegisteredResource>>>,
    prompts: Arc<RwLock<HashMap<String, RegisteredPrompt>>>,
    roots: Arc<RwLock<Vec<Root>>>,
    pending_tool_calls: Arc<Mutex<HashMap<String, oneshot::Sender<CallToolResult>>>>,
}

enum ToolHandlerKind {
    Callback,  // waits for toolResult via JSON-RPC notification
    Native(Box<dyn ToolHandler>),
}
```

ServerHandler methods read from the hashmaps. Callback tools create oneshot channels, send JSON-RPC notifications to simse-core, and await the response.

## Error Handling

Same McpError enum with MCP_ code prefixes. Adds `From<rmcp::ServiceError>` impl.

## Dependencies

```toml
rmcp = { version = "1.1", features = [
    "client", "server",
    "transport-io", "transport-child-process",
    "transport-streamable-http-client-reqwest"
] }
```

Remove: reqwest, uuid (rmcp handles these). Keep: serde, serde_json, thiserror, tracing, tokio, async-trait, tokio-util.

## Testing

- Integration tests stay (same JSON-RPC interface)
- Resilience unit tests stay (extracted unchanged)
- Protocol serialization tests deleted (rmcp's responsibility)
- Client/server unit tests rewritten for wrapper layer
