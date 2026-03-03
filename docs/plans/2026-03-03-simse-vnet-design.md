# simse-vnet Virtual Network Design

**Date:** 2026-03-03
**Status:** Approved

## Overview

A sandboxed virtual networking crate exposing HTTP, WebSocket, TCP, UDP, and DNS operations over JSON-RPC 2.0 / NDJSON stdio. Dual-mode: `mock://` for in-memory mock responses and `net://` for real network requests gated by sandbox rules.

## Architecture

### Scheme Convention

- `mock://` — Virtual backend. Requests matched against registered mock definitions; responses returned from memory.
- `net://` — Real backend. Requests validated against sandbox allowlist, then executed against the real network.

Server routing detects the scheme from the URL/host parameter and delegates to the appropriate handler, following the same pattern as VFS (`vfs://` vs `file://`).

### Core Struct

```rust
pub struct VirtualNetwork {
    mock_store: MockStore,
    sandbox: NetSandboxConfig,
    sessions: HashMap<String, NetSession>,
    http_client: Option<reqwest::Client>,  // Lazy-init on first net:// HTTP request
    total_requests: u64,
    total_errors: u64,
}
```

`VirtualNetwork` is lazily initialized via the `initialize` method. The `http_client` is created on first real HTTP request to avoid unnecessary resource allocation.

### Module Layout

```
simse-vnet/
  Cargo.toml
  src/
    lib.rs          # Module declarations
    main.rs         # tokio::main + tracing setup
    error.rs        # VnetError enum (thiserror, VNET_ prefix)
    protocol.rs     # JSON-RPC request/response types
    transport.rs    # NdjsonTransport (same as vfs/vsh)
    server.rs       # VnetServer: JSON-RPC dispatch
    network.rs      # VirtualNetwork: core logic
    sandbox.rs      # NetSandboxConfig: allowlist validation
    mock_store.rs   # MockStore: mock registry + matching
    session.rs      # NetSession: connection session tracking
```

## Sandbox

### NetSandboxConfig

```rust
pub struct NetSandboxConfig {
    pub allowed_hosts: Vec<HostRule>,
    pub allowed_ports: Vec<PortRange>,
    pub allowed_protocols: Vec<Protocol>,
    pub default_timeout_ms: u64,        // default 30000
    pub max_response_bytes: u64,        // default 10MB
    pub max_connections: usize,         // default 50
}
```

### HostRule

Supports three formats:

| Format | Example | Matches |
|--------|---------|---------|
| Exact hostname | `api.example.com` | Only that host |
| Wildcard | `*.github.com` | Any subdomain of github.com |
| CIDR | `10.0.0.0/8` | Any IP in that range |

### Validation

Every `net://` request is validated before any connection is opened:

1. Extract target host + port from the URL/params.
2. Check host against `allowed_hosts` (if empty, **all blocked** — safe default).
3. Check port against `allowed_ports` (if empty, all ports allowed for matched hosts).
4. Check protocol against `allowed_protocols` (if empty, all protocols allowed).
5. Check active connection count against `max_connections`.

## Mock Store

### MockStore

```rust
pub struct MockStore {
    mocks: Vec<MockDefinition>,
    history: Vec<MockHit>,
}

pub struct MockDefinition {
    pub id: String,              // UUID
    pub method: Option<String>,  // HTTP method, or "tcp"/"udp"/"ws"
    pub url_pattern: String,     // Glob pattern for URL matching
    pub response: MockResponse,
    pub times: Option<usize>,    // None = unlimited, Some(n) = n uses then consumed
}

pub struct MockResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_type: String,       // "text" or "binary" (base64)
    pub delay_ms: Option<u64>,
}

pub struct MockHit {
    pub mock_id: String,
    pub url: String,
    pub method: Option<String>,
    pub timestamp: u64,
}
```

Matching: mocks checked in registration order, first match wins. When a mock's `times` counter reaches 0, it is consumed and skipped. `status` is ignored for TCP/UDP/WS mocks.

## Sessions

Persistent connections (WebSocket, TCP) are tracked as sessions:

```rust
pub struct NetSession {
    pub id: String,                // UUID
    pub session_type: SessionType, // Ws, Tcp
    pub target: String,            // host:port
    pub scheme: Scheme,            // Mock or Net
    pub created_at: u64,
    pub last_active_at: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

pub enum SessionType {
    Ws,
    Tcp,
}
```

UDP is connectionless — no session needed. HTTP is request/response — no persistent session.

Incoming data on persistent connections (WS messages, TCP data) is emitted as JSON-RPC notifications:

```json
{"jsonrpc": "2.0", "method": "net/event", "params": {
  "type": "wsMessage",
  "sessionId": "uuid",
  "data": "..."
}}
```

## JSON-RPC Methods

### Core (1 method)

| Method | Description |
|--------|-------------|
| `initialize` | Configure sandbox rules, create VirtualNetwork |

### Network (10 methods)

| Method | Params | Result |
|--------|--------|--------|
| `net/httpRequest` | url, method, headers?, body?, timeoutMs? | status, headers, body, bodyType, durationMs, bytesReceived |
| `net/wsConnect` | url, headers? | sessionId, status |
| `net/wsMessage` | sessionId, data | ok |
| `net/wsClose` | sessionId | ok |
| `net/tcpConnect` | host, port | sessionId, status |
| `net/tcpSend` | sessionId, data | bytesWritten |
| `net/tcpClose` | sessionId | ok |
| `net/udpSend` | host, port, data, timeoutMs? | response?, bytesReceived |
| `net/resolve` | hostname | addresses, ttl |
| `net/metrics` | — | totalRequests, totalErrors, activeSessions, bytesTotal |

### Mock (5 methods)

| Method | Params | Result |
|--------|--------|--------|
| `mock/register` | method?, urlPattern, response, times? | id |
| `mock/unregister` | id | ok |
| `mock/list` | — | mocks[] |
| `mock/clear` | — | ok |
| `mock/history` | — | hits[] |

### Session (3 methods)

| Method | Params | Result |
|--------|--------|--------|
| `session/list` | — | sessions[] |
| `session/get` | id | session info |
| `session/close` | id | ok |

**Total: 19 methods across 4 domains.**

## Error Codes

```rust
pub enum VnetError {
    NotInitialized,           // VNET_NOT_INITIALIZED
    SandboxViolation(String), // VNET_SANDBOX_VIOLATION
    ConnectionFailed(String), // VNET_CONNECTION_FAILED
    Timeout(String),          // VNET_TIMEOUT
    SessionNotFound(String),  // VNET_SESSION_NOT_FOUND
    MockNotFound(String),     // VNET_MOCK_NOT_FOUND
    NoMockMatch(String),      // VNET_NO_MOCK_MATCH
    LimitExceeded(String),    // VNET_LIMIT_EXCEEDED
    InvalidParams(String),    // VNET_INVALID_PARAMS
    ResponseTooLarge(String), // VNET_RESPONSE_TOO_LARGE
    DnsResolutionFailed(String), // VNET_DNS_FAILED
    Io(std::io::Error),
    Json(serde_json::Error),
}
```

JSON-RPC error code: `-32000` (VNET_ERROR), with `vnetCode` in error data.

## Configuration (InitializeParams)

```rust
pub struct InitializeParams {
    pub sandbox: Option<SandboxParams>,
}

pub struct SandboxParams {
    pub allowed_hosts: Option<Vec<String>>,
    pub allowed_ports: Option<Vec<PortRangeParam>>,
    pub allowed_protocols: Option<Vec<String>>,
    pub default_timeout_ms: Option<u64>,
    pub max_response_bytes: Option<u64>,
    pub max_connections: Option<usize>,
}
```

## Dependencies

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
tokio-tungstenite = "0.26"
uuid = { version = "1", features = ["v4"] }
regex = "1"

[dev-dependencies]
tempfile = "3"
```

## What Does NOT Change

- All other crates (vfs, vsh, acp, mcp, core, vector) are untouched.
- No protocol changes to existing crates.
- No shared dependencies or cross-crate imports.
