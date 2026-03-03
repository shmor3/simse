# simse-vnet Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build `simse-vnet`, a sandboxed virtual networking crate exposing HTTP, WebSocket, TCP, UDP, and DNS over JSON-RPC 2.0 / NDJSON stdio with dual-mode `mock://` and `net://` backends.

**Architecture:** Backend-Per-Scheme routing. `mock://` URLs resolve against an in-memory MockStore. `net://` URLs are validated against a NetSandboxConfig allowlist, then executed with real networking (reqwest for HTTP, tokio-tungstenite for WS, tokio TCP/UDP). VirtualNetwork owns all state; VnetServer dispatches JSON-RPC methods.

**Tech Stack:** Rust (edition 2024), tokio, reqwest, tokio-tungstenite, serde/serde_json, thiserror, tracing, uuid, regex

**Design doc:** `docs/plans/2026-03-03-simse-vnet-design.md`

---

### Task 1: Scaffold the crate

**Files:**
- Create: `simse-vnet/Cargo.toml`
- Create: `simse-vnet/src/lib.rs`
- Create: `simse-vnet/src/main.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "simse-vnet-engine"
version = "0.1.0"
edition = "2024"
license = "MIT"
rust-version = "1.85"
description = "Virtual network server over JSON-RPC 2.0 / NDJSON stdio"

[lib]
name = "simse_vnet_engine"
path = "src/lib.rs"

[[bin]]
name = "simse-vnet-engine"
path = "src/main.rs"

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

**Step 2: Create lib.rs**

```rust
pub mod error;
pub mod protocol;
pub mod transport;
pub mod server;
pub mod network;
pub mod sandbox;
pub mod mock_store;
pub mod session;
```

**Step 3: Create main.rs**

```rust
use simse_vnet_engine::server::VnetServer;
use simse_vnet_engine::transport::NdjsonTransport;

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
    let mut server = VnetServer::new(transport);

    tracing::info!("simse-vnet-engine ready");

    if let Err(e) = server.run().await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

**Step 4: Create stub modules**

Create empty stub files so it compiles: `error.rs`, `protocol.rs`, `transport.rs`, `server.rs`, `network.rs`, `sandbox.rs`, `mock_store.rs`, `session.rs`. Each contains just enough to satisfy the imports from main.rs (empty structs for `VnetServer`, `NdjsonTransport`, and a `VnetServer::new()` + `VnetServer::run()` method).

Minimal stubs:

`error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VnetError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
```

`protocol.rs`:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}
```

`transport.rs` — copy exact pattern from `simse-vsh/src/transport.rs` (see vsh reference).

`sandbox.rs`:
```rust
pub struct NetSandboxConfig;
```

`mock_store.rs`:
```rust
pub struct MockStore;
```

`session.rs`:
```rust
// Empty for now
```

`network.rs`:
```rust
pub struct VirtualNetwork;
```

`server.rs`:
```rust
use crate::transport::NdjsonTransport;

pub struct VnetServer {
    transport: NdjsonTransport,
}

impl VnetServer {
    pub fn new(transport: NdjsonTransport) -> Self {
        Self { transport }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
```

**Step 5: Verify it builds**

Run: `cd simse-vnet && cargo build`
Expected: Compiles with no errors (warnings OK for now).

**Step 6: Commit**

```bash
git add simse-vnet/
git commit -m "feat(simse-vnet): scaffold crate with stub modules"
```

---

### Task 2: Error types

**Files:**
- Modify: `simse-vnet/src/error.rs`

**Step 1: Write unit tests for VnetError**

Add to bottom of `error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_vnet_prefix() {
        let cases = vec![
            (VnetError::NotInitialized, "VNET_NOT_INITIALIZED"),
            (
                VnetError::SandboxViolation("test".into()),
                "VNET_SANDBOX_VIOLATION",
            ),
            (
                VnetError::ConnectionFailed("test".into()),
                "VNET_CONNECTION_FAILED",
            ),
            (VnetError::Timeout("test".into()), "VNET_TIMEOUT"),
            (
                VnetError::SessionNotFound("test".into()),
                "VNET_SESSION_NOT_FOUND",
            ),
            (
                VnetError::MockNotFound("test".into()),
                "VNET_MOCK_NOT_FOUND",
            ),
            (
                VnetError::NoMockMatch("test".into()),
                "VNET_NO_MOCK_MATCH",
            ),
            (
                VnetError::LimitExceeded("test".into()),
                "VNET_LIMIT_EXCEEDED",
            ),
            (
                VnetError::InvalidParams("test".into()),
                "VNET_INVALID_PARAMS",
            ),
            (
                VnetError::ResponseTooLarge("test".into()),
                "VNET_RESPONSE_TOO_LARGE",
            ),
            (
                VnetError::DnsResolutionFailed("test".into()),
                "VNET_DNS_FAILED",
            ),
        ];
        for (err, expected_code) in cases {
            assert_eq!(err.code(), expected_code, "wrong code for {err}");
        }
    }

    #[test]
    fn to_json_rpc_error_includes_vnet_code() {
        let err = VnetError::SandboxViolation("blocked".into());
        let val = err.to_json_rpc_error();
        assert_eq!(val["vnetCode"], "VNET_SANDBOX_VIOLATION");
        assert!(val["message"].as_str().unwrap().contains("blocked"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vnet && cargo test -- error`
Expected: FAIL — missing variants/methods.

**Step 3: Implement VnetError**

Replace `error.rs` with:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VnetError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Mock not found: {0}")]
    MockNotFound(String),
    #[error("No mock match: {0}")]
    NoMockMatch(String),
    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("Response too large: {0}")]
    ResponseTooLarge(String),
    #[error("DNS resolution failed: {0}")]
    DnsResolutionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl VnetError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "VNET_NOT_INITIALIZED",
            Self::SandboxViolation(_) => "VNET_SANDBOX_VIOLATION",
            Self::ConnectionFailed(_) => "VNET_CONNECTION_FAILED",
            Self::Timeout(_) => "VNET_TIMEOUT",
            Self::SessionNotFound(_) => "VNET_SESSION_NOT_FOUND",
            Self::MockNotFound(_) => "VNET_MOCK_NOT_FOUND",
            Self::NoMockMatch(_) => "VNET_NO_MOCK_MATCH",
            Self::LimitExceeded(_) => "VNET_LIMIT_EXCEEDED",
            Self::InvalidParams(_) => "VNET_INVALID_PARAMS",
            Self::ResponseTooLarge(_) => "VNET_RESPONSE_TOO_LARGE",
            Self::DnsResolutionFailed(_) => "VNET_DNS_FAILED",
            Self::Io(_) => "VNET_IO_ERROR",
            Self::Json(_) => "VNET_JSON_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "vnetCode": self.code(),
            "message": self.to_string(),
        })
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vnet && cargo test -- error`
Expected: 2 tests PASS.

**Step 5: Commit**

```bash
git add simse-vnet/src/error.rs
git commit -m "feat(simse-vnet): add VnetError enum with VNET_ code prefix"
```

---

### Task 3: Protocol types

**Files:**
- Modify: `simse-vnet/src/protocol.rs`

**Step 1: Write serde round-trip tests**

Add to bottom of `protocol.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_params_deserializes() {
        let json = serde_json::json!({
            "sandbox": {
                "allowedHosts": ["*.example.com", "10.0.0.0/8"],
                "allowedPorts": [{"start": 80, "end": 80}, {"start": 443, "end": 443}],
                "allowedProtocols": ["http", "https"],
                "defaultTimeoutMs": 5000,
                "maxResponseBytes": 1048576,
                "maxConnections": 10
            }
        });
        let params: InitializeParams = serde_json::from_value(json).unwrap();
        let sandbox = params.sandbox.unwrap();
        assert_eq!(sandbox.allowed_hosts.unwrap().len(), 2);
        assert_eq!(sandbox.allowed_ports.unwrap()[0].start, 80);
    }

    #[test]
    fn http_request_params_deserializes() {
        let json = serde_json::json!({
            "url": "mock://api.example.com/users",
            "method": "GET",
            "headers": {"Authorization": "Bearer token"}
        });
        let params: HttpRequestParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.url, "mock://api.example.com/users");
        assert_eq!(params.method.unwrap(), "GET");
    }

    #[test]
    fn mock_register_params_deserializes() {
        let json = serde_json::json!({
            "urlPattern": "mock://api.example.com/*",
            "method": "GET",
            "response": {
                "status": 200,
                "headers": {"Content-Type": "application/json"},
                "body": "{\"ok\":true}",
                "bodyType": "text"
            },
            "times": 3
        });
        let params: MockRegisterParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.url_pattern, "mock://api.example.com/*");
        assert_eq!(params.times, Some(3));
        assert_eq!(params.response.status, 200);
    }

    #[test]
    fn http_response_result_serializes_camel_case() {
        let result = HttpResponseResult {
            status: 200,
            headers: std::collections::HashMap::new(),
            body: "hello".into(),
            body_type: "text".into(),
            duration_ms: 42,
            bytes_received: 5,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["durationMs"], 42);
        assert_eq!(json["bytesReceived"], 5);
        assert_eq!(json["bodyType"], "text");
    }

    #[test]
    fn session_info_serializes_camel_case() {
        let info = SessionInfo {
            id: "abc".into(),
            session_type: "ws".into(),
            target: "example.com:443".into(),
            scheme: "mock".into(),
            created_at: 1000,
            last_active_at: 2000,
            bytes_sent: 100,
            bytes_received: 200,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["sessionType"], "ws");
        assert_eq!(json["createdAt"], 1000);
        assert_eq!(json["lastActiveAt"], 2000);
        assert_eq!(json["bytesSent"], 100);
        assert_eq!(json["bytesReceived"], 200);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vnet && cargo test -- protocol`
Expected: FAIL — structs don't exist yet.

**Step 3: Implement protocol types**

Replace `protocol.rs` with all JSON-RPC request/response types. All structs use `#[serde(rename_all = "camelCase")]`:

```rust
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── JSON-RPC framing ──

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const VNET_ERROR: i32 = -32000;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

// ── Initialize ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub sandbox: Option<SandboxParams>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxParams {
    pub allowed_hosts: Option<Vec<String>>,
    pub allowed_ports: Option<Vec<PortRangeParam>>,
    pub allowed_protocols: Option<Vec<String>>,
    pub default_timeout_ms: Option<u64>,
    pub max_response_bytes: Option<u64>,
    pub max_connections: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortRangeParam {
    pub start: u16,
    pub end: u16,
}

// ── Network methods ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestParams {
    pub url: String,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseResult {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_type: String,
    pub duration_ms: u64,
    pub bytes_received: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsConnectParams {
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsMessageParams {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdParam {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpConnectParams {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpSendParams {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpSendParams {
    pub host: String,
    pub port: u16,
    pub data: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveParams {
    pub hostname: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveResult {
    pub addresses: Vec<String>,
    pub ttl: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsResult {
    pub total_requests: u64,
    pub total_errors: u64,
    pub active_sessions: usize,
    pub bytes_total: u64,
}

// ── Mock methods ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockRegisterParams {
    pub method: Option<String>,
    pub url_pattern: String,
    pub response: MockResponseParam,
    pub times: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockResponseParam {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
    #[serde(default = "default_body_type")]
    pub body_type: String,
    pub delay_ms: Option<u64>,
}

fn default_body_type() -> String {
    "text".into()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockIdParam {
    pub id: String,
}

// ── Mock response types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockDefinitionInfo {
    pub id: String,
    pub method: Option<String>,
    pub url_pattern: String,
    pub status: u16,
    pub times: Option<usize>,
    pub remaining: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockHitInfo {
    pub mock_id: String,
    pub url: String,
    pub method: Option<String>,
    pub timestamp: u64,
}

// ── Session response types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub session_type: String,
    pub target: String,
    pub scheme: String,
    pub created_at: u64,
    pub last_active_at: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vnet && cargo test -- protocol`
Expected: 5 tests PASS.

**Step 5: Commit**

```bash
git add simse-vnet/src/protocol.rs
git commit -m "feat(simse-vnet): add JSON-RPC protocol types with serde"
```

---

### Task 4: Transport

**Files:**
- Modify: `simse-vnet/src/transport.rs`

**Step 1: Implement NdjsonTransport**

Copy the exact pattern from `simse-vsh/src/transport.rs`. The transport is identical across all crates:

```rust
use std::io::{self, Write};

use serde::Serialize;

#[derive(Serialize)]
struct JsonRpcResponse<'a> {
    jsonrpc: &'a str,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcErrorBody>,
}

#[derive(Serialize)]
struct JsonRpcErrorBody {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct JsonRpcNotification<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

pub struct NdjsonTransport;

impl Default for NdjsonTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl NdjsonTransport {
    pub fn new() -> Self {
        Self
    }

    pub fn write_response(&self, id: u64, result: serde_json::Value) {
        self.write_line(&JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        });
    }

    pub fn write_error(
        &self,
        id: u64,
        code: i32,
        message: impl Into<String>,
        data: Option<serde_json::Value>,
    ) {
        self.write_line(&JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcErrorBody {
                code,
                message: message.into(),
                data,
            }),
        });
    }

    pub fn write_notification(&self, method: &str, params: serde_json::Value) {
        self.write_line(&JsonRpcNotification {
            jsonrpc: "2.0",
            method,
            params: Some(params),
        });
    }

    fn write_line(&self, value: &impl Serialize) {
        let mut stdout = io::stdout().lock();
        if let Err(e) = serde_json::to_writer(&mut stdout, value) {
            tracing::error!("Failed to serialize: {}", e);
            return;
        }
        let _ = writeln!(stdout);
        let _ = stdout.flush();
    }
}
```

**Step 2: Verify it builds**

Run: `cd simse-vnet && cargo build`
Expected: Compiles.

**Step 3: Commit**

```bash
git add simse-vnet/src/transport.rs
git commit -m "feat(simse-vnet): add NdjsonTransport for JSON-RPC stdio"
```

---

### Task 5: Sandbox validation

**Files:**
- Modify: `simse-vnet/src/sandbox.rs`

**Step 1: Write unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> NetSandboxConfig {
        NetSandboxConfig::default()
    }

    #[test]
    fn empty_allowlist_blocks_everything() {
        let cfg = default_config();
        let err = cfg.validate("example.com", 80, "http").unwrap_err();
        assert!(err.contains("not in allowed hosts"));
    }

    #[test]
    fn exact_host_match() {
        let mut cfg = default_config();
        cfg.allowed_hosts.push(HostRule::Exact("api.example.com".into()));
        assert!(cfg.validate("api.example.com", 80, "http").is_ok());
        assert!(cfg.validate("other.example.com", 80, "http").is_err());
    }

    #[test]
    fn wildcard_host_match() {
        let mut cfg = default_config();
        cfg.allowed_hosts.push(HostRule::Wildcard("*.github.com".into()));
        assert!(cfg.validate("api.github.com", 443, "https").is_ok());
        assert!(cfg.validate("raw.github.com", 443, "https").is_ok());
        assert!(cfg.validate("github.com", 443, "https").is_err());
        assert!(cfg.validate("evil.com", 443, "https").is_err());
    }

    #[test]
    fn cidr_host_match() {
        let mut cfg = default_config();
        cfg.allowed_hosts.push(HostRule::Cidr {
            addr: [10, 0, 0, 0],
            prefix: 8,
        });
        assert!(cfg.validate("10.0.0.1", 80, "http").is_ok());
        assert!(cfg.validate("10.255.255.255", 80, "http").is_ok());
        assert!(cfg.validate("11.0.0.1", 80, "http").is_err());
    }

    #[test]
    fn port_range_validation() {
        let mut cfg = default_config();
        cfg.allowed_hosts.push(HostRule::Exact("example.com".into()));
        cfg.allowed_ports.push(PortRange { start: 80, end: 80 });
        cfg.allowed_ports.push(PortRange { start: 443, end: 443 });
        assert!(cfg.validate("example.com", 80, "http").is_ok());
        assert!(cfg.validate("example.com", 443, "https").is_ok());
        assert!(cfg.validate("example.com", 8080, "http").is_err());
    }

    #[test]
    fn empty_ports_allows_all() {
        let mut cfg = default_config();
        cfg.allowed_hosts.push(HostRule::Exact("example.com".into()));
        // No port restrictions
        assert!(cfg.validate("example.com", 12345, "http").is_ok());
    }

    #[test]
    fn protocol_restriction() {
        let mut cfg = default_config();
        cfg.allowed_hosts.push(HostRule::Exact("example.com".into()));
        cfg.allowed_protocols.push("https".into());
        assert!(cfg.validate("example.com", 443, "https").is_ok());
        assert!(cfg.validate("example.com", 80, "http").is_err());
    }

    #[test]
    fn empty_protocols_allows_all() {
        let mut cfg = default_config();
        cfg.allowed_hosts.push(HostRule::Exact("example.com".into()));
        assert!(cfg.validate("example.com", 80, "tcp").is_ok());
    }

    #[test]
    fn parse_host_rule_exact() {
        let rule = HostRule::parse("api.example.com");
        assert!(matches!(rule, HostRule::Exact(h) if h == "api.example.com"));
    }

    #[test]
    fn parse_host_rule_wildcard() {
        let rule = HostRule::parse("*.github.com");
        assert!(matches!(rule, HostRule::Wildcard(p) if p == "*.github.com"));
    }

    #[test]
    fn parse_host_rule_cidr() {
        let rule = HostRule::parse("192.168.1.0/24");
        assert!(matches!(rule, HostRule::Cidr { prefix: 24, .. }));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vnet && cargo test -- sandbox`
Expected: FAIL.

**Step 3: Implement sandbox.rs**

```rust
use crate::error::VnetError;

#[derive(Debug, Clone)]
pub enum HostRule {
    Exact(String),
    Wildcard(String),
    Cidr { addr: [u8; 4], prefix: u8 },
}

impl HostRule {
    pub fn parse(s: &str) -> Self {
        if s.starts_with("*.") {
            return HostRule::Wildcard(s.to_string());
        }
        if let Some(cidr) = Self::try_parse_cidr(s) {
            return cidr;
        }
        HostRule::Exact(s.to_string())
    }

    fn try_parse_cidr(s: &str) -> Option<Self> {
        let (ip_str, prefix_str) = s.split_once('/')?;
        let prefix: u8 = prefix_str.parse().ok()?;
        if prefix > 32 {
            return None;
        }
        let parts: Vec<&str> = ip_str.split('.').collect();
        if parts.len() != 4 {
            return None;
        }
        let mut addr = [0u8; 4];
        for (i, part) in parts.iter().enumerate() {
            addr[i] = part.parse().ok()?;
        }
        Some(HostRule::Cidr { addr, prefix })
    }

    fn matches(&self, host: &str) -> bool {
        match self {
            HostRule::Exact(h) => h.eq_ignore_ascii_case(host),
            HostRule::Wildcard(pattern) => {
                // "*.github.com" matches "api.github.com" but not "github.com"
                let suffix = &pattern[1..]; // ".github.com"
                host.len() > suffix.len()
                    && host[host.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
                    && !host[..host.len() - suffix.len()].contains('.')
            }
            HostRule::Cidr { addr, prefix } => {
                let Some(ip) = Self::parse_ipv4(host) else {
                    return false;
                };
                let mask = if *prefix == 0 {
                    0u32
                } else {
                    !0u32 << (32 - prefix)
                };
                let net = u32::from_be_bytes(*addr);
                let target = u32::from_be_bytes(ip);
                (net & mask) == (target & mask)
            }
        }
    }

    fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 4 {
            return None;
        }
        let mut addr = [0u8; 4];
        for (i, part) in parts.iter().enumerate() {
            addr[i] = part.parse().ok()?;
        }
        Some(addr)
    }
}

#[derive(Debug, Clone)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

pub struct NetSandboxConfig {
    pub allowed_hosts: Vec<HostRule>,
    pub allowed_ports: Vec<PortRange>,
    pub allowed_protocols: Vec<String>,
    pub default_timeout_ms: u64,
    pub max_response_bytes: u64,
    pub max_connections: usize,
}

impl Default for NetSandboxConfig {
    fn default() -> Self {
        Self {
            allowed_hosts: Vec::new(),
            allowed_ports: Vec::new(),
            allowed_protocols: Vec::new(),
            default_timeout_ms: 30_000,
            max_response_bytes: 10 * 1024 * 1024,
            max_connections: 50,
        }
    }
}

impl NetSandboxConfig {
    pub fn validate(&self, host: &str, port: u16, protocol: &str) -> Result<(), String> {
        // Host check: if empty, block all (safe default)
        if !self.allowed_hosts.iter().any(|rule| rule.matches(host)) {
            return Err(format!("host '{host}' not in allowed hosts"));
        }

        // Port check: if empty, allow all
        if !self.allowed_ports.is_empty()
            && !self.allowed_ports.iter().any(|r| port >= r.start && port <= r.end)
        {
            return Err(format!("port {port} not in allowed ports"));
        }

        // Protocol check: if empty, allow all
        if !self.allowed_protocols.is_empty()
            && !self.allowed_protocols.iter().any(|p| p.eq_ignore_ascii_case(protocol))
        {
            return Err(format!("protocol '{protocol}' not allowed"));
        }

        Ok(())
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vnet && cargo test -- sandbox`
Expected: 11 tests PASS.

**Step 5: Commit**

```bash
git add simse-vnet/src/sandbox.rs
git commit -m "feat(simse-vnet): add NetSandboxConfig with host/port/protocol validation"
```

---

### Task 6: Mock store

**Files:**
- Modify: `simse-vnet/src/mock_store.rs`

**Step 1: Write unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_response() -> MockResponse {
        MockResponse {
            status: 200,
            headers: HashMap::new(),
            body: "{\"ok\":true}".into(),
            body_type: "text".into(),
            delay_ms: None,
        }
    }

    #[test]
    fn register_and_match() {
        let mut store = MockStore::new();
        let id = store.register(Some("GET".into()), "mock://api.example.com/users", make_response(), None);
        let hit = store.find_match("mock://api.example.com/users", Some("GET"));
        assert!(hit.is_some());
        let (matched_id, resp) = hit.unwrap();
        assert_eq!(matched_id, id);
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn glob_pattern_matching() {
        let mut store = MockStore::new();
        store.register(None, "mock://api.example.com/*", make_response(), None);
        assert!(store.find_match("mock://api.example.com/users", None).is_some());
        assert!(store.find_match("mock://api.example.com/posts/123", None).is_some());
        assert!(store.find_match("mock://other.com/users", None).is_none());
    }

    #[test]
    fn method_filtering() {
        let mut store = MockStore::new();
        store.register(Some("POST".into()), "mock://api.example.com/users", make_response(), None);
        assert!(store.find_match("mock://api.example.com/users", Some("POST")).is_some());
        assert!(store.find_match("mock://api.example.com/users", Some("GET")).is_none());
    }

    #[test]
    fn none_method_matches_any() {
        let mut store = MockStore::new();
        store.register(None, "mock://api.example.com/users", make_response(), None);
        assert!(store.find_match("mock://api.example.com/users", Some("GET")).is_some());
        assert!(store.find_match("mock://api.example.com/users", Some("POST")).is_some());
    }

    #[test]
    fn times_limit_consumes_mock() {
        let mut store = MockStore::new();
        store.register(None, "mock://api.example.com/once", make_response(), Some(1));
        assert!(store.find_match("mock://api.example.com/once", None).is_some());
        assert!(store.find_match("mock://api.example.com/once", None).is_none());
    }

    #[test]
    fn unregister_removes_mock() {
        let mut store = MockStore::new();
        let id = store.register(None, "mock://x", make_response(), None);
        assert!(store.unregister(&id));
        assert!(store.find_match("mock://x", None).is_none());
    }

    #[test]
    fn clear_removes_all() {
        let mut store = MockStore::new();
        store.register(None, "mock://a", make_response(), None);
        store.register(None, "mock://b", make_response(), None);
        store.clear();
        assert!(store.list().is_empty());
    }

    #[test]
    fn history_tracks_hits() {
        let mut store = MockStore::new();
        store.register(None, "mock://api/*", make_response(), None);
        store.find_match("mock://api/one", Some("GET"));
        store.find_match("mock://api/two", Some("POST"));
        let hist = store.history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].url, "mock://api/one");
        assert_eq!(hist[1].url, "mock://api/two");
    }

    #[test]
    fn first_match_wins() {
        let mut store = MockStore::new();
        let mut resp1 = make_response();
        resp1.status = 200;
        let mut resp2 = make_response();
        resp2.status = 404;
        store.register(None, "mock://api/*", resp1, None);
        store.register(None, "mock://api/*", resp2, None);
        let (_, resp) = store.find_match("mock://api/test", None).unwrap();
        assert_eq!(resp.status, 200);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vnet && cargo test -- mock_store`
Expected: FAIL.

**Step 3: Implement mock_store.rs**

```rust
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_type: String,
    pub delay_ms: Option<u64>,
}

#[derive(Debug)]
struct MockDefinition {
    id: String,
    method: Option<String>,
    url_pattern: String,
    compiled: Regex,
    response: MockResponse,
    times: Option<usize>,
    remaining: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct MockHit {
    pub mock_id: String,
    pub url: String,
    pub method: Option<String>,
    pub timestamp: u64,
}

pub struct MockListItem {
    pub id: String,
    pub method: Option<String>,
    pub url_pattern: String,
    pub status: u16,
    pub times: Option<usize>,
    pub remaining: Option<usize>,
}

pub struct MockStore {
    mocks: Vec<MockDefinition>,
    hits: Vec<MockHit>,
}

impl MockStore {
    pub fn new() -> Self {
        Self {
            mocks: Vec::new(),
            hits: Vec::new(),
        }
    }

    pub fn register(
        &mut self,
        method: Option<String>,
        url_pattern: &str,
        response: MockResponse,
        times: Option<usize>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let compiled = glob_to_regex(url_pattern);
        self.mocks.push(MockDefinition {
            id: id.clone(),
            method,
            url_pattern: url_pattern.to_string(),
            compiled,
            response,
            times,
            remaining: times,
        });
        id
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        let len_before = self.mocks.len();
        self.mocks.retain(|m| m.id != id);
        self.mocks.len() < len_before
    }

    pub fn find_match(&mut self, url: &str, method: Option<&str>) -> Option<(String, MockResponse)> {
        let idx = self.mocks.iter().position(|m| {
            // Check remaining count
            if let Some(0) = m.remaining {
                return false;
            }
            // Check method (None mock method = match any)
            if let Some(ref mock_method) = m.method {
                if let Some(req_method) = method {
                    if !mock_method.eq_ignore_ascii_case(req_method) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            // Check URL pattern
            m.compiled.is_match(url)
        })?;

        let mock = &mut self.mocks[idx];
        let id = mock.id.clone();
        let response = mock.response.clone();

        // Decrement remaining
        if let Some(ref mut remaining) = mock.remaining {
            *remaining -= 1;
        }

        // Record hit
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.hits.push(MockHit {
            mock_id: id.clone(),
            url: url.to_string(),
            method: method.map(String::from),
            timestamp: now,
        });

        Some((id, response))
    }

    pub fn list(&self) -> Vec<MockListItem> {
        self.mocks
            .iter()
            .filter(|m| m.remaining != Some(0))
            .map(|m| MockListItem {
                id: m.id.clone(),
                method: m.method.clone(),
                url_pattern: m.url_pattern.clone(),
                status: m.response.status,
                times: m.times,
                remaining: m.remaining,
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.mocks.clear();
        self.hits.clear();
    }

    pub fn history(&self) -> &[MockHit] {
        &self.hits
    }
}

/// Convert a glob pattern (with `*`) to a regex.
/// `*` matches any character sequence (non-greedy).
fn glob_to_regex(pattern: &str) -> Regex {
    let mut re = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '.' | '(' | ')' | '[' | ']' | '{' | '}' | '+' | '?' | '^' | '$' | '|' | '\\' => {
                re.push('\\');
                re.push(ch);
            }
            _ => re.push(ch),
        }
    }
    re.push('$');
    Regex::new(&re).expect("invalid glob pattern")
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vnet && cargo test -- mock_store`
Expected: 9 tests PASS.

**Step 5: Commit**

```bash
git add simse-vnet/src/mock_store.rs
git commit -m "feat(simse-vnet): add MockStore with pattern matching and hit history"
```

---

### Task 7: Session management

**Files:**
- Modify: `simse-vnet/src/session.rs`

**Step 1: Write unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get_session() {
        let mut mgr = SessionManager::new();
        let id = mgr.create(SessionType::Ws, "example.com:443", Scheme::Mock);
        let session = mgr.get(&id).unwrap();
        assert_eq!(session.target, "example.com:443");
        assert_eq!(session.session_type, SessionType::Ws);
        assert_eq!(session.scheme, Scheme::Mock);
    }

    #[test]
    fn list_sessions() {
        let mut mgr = SessionManager::new();
        mgr.create(SessionType::Ws, "a.com:443", Scheme::Mock);
        mgr.create(SessionType::Tcp, "b.com:80", Scheme::Net);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn close_session() {
        let mut mgr = SessionManager::new();
        let id = mgr.create(SessionType::Tcp, "c.com:22", Scheme::Net);
        assert!(mgr.close(&id));
        assert!(mgr.get(&id).is_none());
        assert!(!mgr.close(&id)); // already closed
    }

    #[test]
    fn update_activity() {
        let mut mgr = SessionManager::new();
        let id = mgr.create(SessionType::Ws, "d.com:443", Scheme::Mock);
        let before = mgr.get(&id).unwrap().last_active_at;
        mgr.record_activity(&id, 100, 200);
        let after = mgr.get(&id).unwrap();
        assert!(after.last_active_at >= before);
        assert_eq!(after.bytes_sent, 100);
        assert_eq!(after.bytes_received, 200);
    }

    #[test]
    fn active_count() {
        let mut mgr = SessionManager::new();
        assert_eq!(mgr.active_count(), 0);
        let id1 = mgr.create(SessionType::Ws, "a.com:443", Scheme::Mock);
        mgr.create(SessionType::Tcp, "b.com:80", Scheme::Net);
        assert_eq!(mgr.active_count(), 2);
        mgr.close(&id1);
        assert_eq!(mgr.active_count(), 1);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vnet && cargo test -- session`
Expected: FAIL.

**Step 3: Implement session.rs**

```rust
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub enum SessionType {
    Ws,
    Tcp,
}

impl SessionType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ws => "ws",
            Self::Tcp => "tcp",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scheme {
    Mock,
    Net,
}

impl Scheme {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Mock => "mock",
            Self::Net => "net",
        }
    }
}

#[derive(Debug)]
pub struct NetSession {
    pub id: String,
    pub session_type: SessionType,
    pub target: String,
    pub scheme: Scheme,
    pub created_at: u64,
    pub last_active_at: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

pub struct SessionManager {
    sessions: HashMap<String, NetSession>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn create(&mut self, session_type: SessionType, target: &str, scheme: Scheme) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        self.sessions.insert(
            id.clone(),
            NetSession {
                id: id.clone(),
                session_type,
                target: target.to_string(),
                scheme,
                created_at: now,
                last_active_at: now,
                bytes_sent: 0,
                bytes_received: 0,
            },
        );
        id
    }

    pub fn get(&self, id: &str) -> Option<&NetSession> {
        self.sessions.get(id)
    }

    pub fn list(&self) -> Vec<&NetSession> {
        self.sessions.values().collect()
    }

    pub fn close(&mut self, id: &str) -> bool {
        self.sessions.remove(id).is_some()
    }

    pub fn record_activity(&mut self, id: &str, bytes_sent: u64, bytes_received: u64) {
        if let Some(session) = self.sessions.get_mut(id) {
            session.last_active_at = now_ms();
            session.bytes_sent += bytes_sent;
            session.bytes_received += bytes_received;
        }
    }

    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vnet && cargo test -- session`
Expected: 5 tests PASS.

**Step 5: Commit**

```bash
git add simse-vnet/src/session.rs
git commit -m "feat(simse-vnet): add SessionManager for persistent connection tracking"
```

---

### Task 8: VirtualNetwork core

**Files:**
- Modify: `simse-vnet/src/network.rs`

This is the central struct that ties sandbox, mock store, and sessions together. This task covers `initialize`, mock-backed HTTP requests, and metrics. Real networking (`net://`) handlers are stubs returning `ConnectionFailed` for now — they'll be wired in Task 9.

**Step 1: Write unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn not_initialized_error() {
        let net = VirtualNetwork::new();
        let err = net.mock_http_request("mock://x", Some("GET"), None, None).unwrap_err();
        assert!(matches!(err, VnetError::NotInitialized));
    }

    #[test]
    fn initialize_sets_sandbox() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);
        assert!(net.is_initialized());
    }

    #[test]
    fn mock_http_request_no_match() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);
        let err = net.mock_http_request("mock://api/test", Some("GET"), None, None).unwrap_err();
        assert!(matches!(err, VnetError::NoMockMatch(_)));
    }

    #[test]
    fn mock_http_request_success() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);
        net.register_mock(
            Some("GET".into()),
            "mock://api/users",
            mock_store::MockResponse {
                status: 200,
                headers: HashMap::new(),
                body: "{\"users\":[]}".into(),
                body_type: "text".into(),
                delay_ms: None,
            },
            None,
        );
        let resp = net.mock_http_request("mock://api/users", Some("GET"), None, None).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "{\"users\":[]}");
    }

    #[test]
    fn metrics_track_requests() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);
        net.register_mock(None, "mock://api/*", mock_store::MockResponse {
            status: 200,
            headers: HashMap::new(),
            body: "ok".into(),
            body_type: "text".into(),
            delay_ms: None,
        }, None);
        net.mock_http_request("mock://api/a", None, None, None).unwrap();
        net.mock_http_request("mock://api/b", None, None, None).unwrap();
        let m = net.metrics();
        assert_eq!(m.total_requests, 2);
        assert_eq!(m.total_errors, 0);
    }

    #[test]
    fn metrics_track_errors() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);
        let _ = net.mock_http_request("mock://no-match", None, None, None);
        let m = net.metrics();
        assert_eq!(m.total_requests, 1);
        assert_eq!(m.total_errors, 1);
    }

    #[test]
    fn sandbox_blocks_net_request() {
        let mut net = VirtualNetwork::new();
        net.initialize(None); // empty allowlist = block all
        let err = net.validate_net_request("evil.com", 80, "http").unwrap_err();
        assert!(matches!(err, VnetError::SandboxViolation(_)));
    }

    #[test]
    fn sandbox_allows_configured_host() {
        let mut net = VirtualNetwork::new();
        let sandbox = SandboxInit {
            allowed_hosts: vec!["api.example.com".into()],
            allowed_ports: vec![],
            allowed_protocols: vec![],
            default_timeout_ms: 30_000,
            max_response_bytes: 10 * 1024 * 1024,
            max_connections: 50,
        };
        net.initialize(Some(sandbox));
        assert!(net.validate_net_request("api.example.com", 443, "https").is_ok());
    }

    #[test]
    fn connection_limit_enforced() {
        let mut net = VirtualNetwork::new();
        let sandbox = SandboxInit {
            allowed_hosts: vec!["*".into()],
            allowed_ports: vec![],
            allowed_protocols: vec![],
            default_timeout_ms: 30_000,
            max_response_bytes: 10 * 1024 * 1024,
            max_connections: 1,
        };
        net.initialize(Some(sandbox));
        // Directly add a session to simulate active connection
        net.sessions.create(
            crate::session::SessionType::Tcp,
            "a.com:80",
            crate::session::Scheme::Net,
        );
        let err = net.check_connection_limit().unwrap_err();
        assert!(matches!(err, VnetError::LimitExceeded(_)));
    }

    #[test]
    fn mock_register_list_unregister_clear() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);

        let id = net.register_mock(None, "mock://a", mock_store::MockResponse {
            status: 200,
            headers: HashMap::new(),
            body: "".into(),
            body_type: "text".into(),
            delay_ms: None,
        }, None);

        assert_eq!(net.list_mocks().len(), 1);

        net.unregister_mock(&id).unwrap();
        assert!(net.list_mocks().is_empty());

        net.register_mock(None, "mock://b", mock_store::MockResponse {
            status: 200,
            headers: HashMap::new(),
            body: "".into(),
            body_type: "text".into(),
            delay_ms: None,
        }, None);
        net.clear_mocks();
        assert!(net.list_mocks().is_empty());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vnet && cargo test -- network`
Expected: FAIL.

**Step 3: Implement network.rs**

```rust
use std::collections::HashMap;

use crate::error::VnetError;
use crate::mock_store::{self, MockStore};
use crate::protocol::{HttpResponseResult, MetricsResult, MockDefinitionInfo, MockHitInfo, SessionInfo};
use crate::sandbox::{HostRule, NetSandboxConfig, PortRange};
use crate::session::{SessionManager, SessionType, Scheme};

pub struct SandboxInit {
    pub allowed_hosts: Vec<String>,
    pub allowed_ports: Vec<(u16, u16)>,
    pub allowed_protocols: Vec<String>,
    pub default_timeout_ms: u64,
    pub max_response_bytes: u64,
    pub max_connections: usize,
}

pub struct VirtualNetwork {
    initialized: bool,
    sandbox: NetSandboxConfig,
    mock_store: MockStore,
    pub(crate) sessions: SessionManager,
    total_requests: u64,
    total_errors: u64,
}

impl VirtualNetwork {
    pub fn new() -> Self {
        Self {
            initialized: false,
            sandbox: NetSandboxConfig::default(),
            mock_store: MockStore::new(),
            sessions: SessionManager::new(),
            total_requests: 0,
            total_errors: 0,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn initialize(&mut self, sandbox: Option<SandboxInit>) {
        if let Some(s) = sandbox {
            self.sandbox = NetSandboxConfig {
                allowed_hosts: s.allowed_hosts.iter().map(|h| HostRule::parse(h)).collect(),
                allowed_ports: s.allowed_ports.iter().map(|(start, end)| PortRange { start: *start, end: *end }).collect(),
                allowed_protocols: s.allowed_protocols,
                default_timeout_ms: s.default_timeout_ms,
                max_response_bytes: s.max_response_bytes,
                max_connections: s.max_connections,
            };
        }
        self.initialized = true;
    }

    fn require_initialized(&self) -> Result<(), VnetError> {
        if !self.initialized {
            return Err(VnetError::NotInitialized);
        }
        Ok(())
    }

    // ── Mock HTTP ──

    pub fn mock_http_request(
        &mut self,
        url: &str,
        method: Option<&str>,
        _headers: Option<&HashMap<String, String>>,
        _body: Option<&str>,
    ) -> Result<HttpResponseResult, VnetError> {
        self.require_initialized()?;
        self.total_requests += 1;

        let result = self.mock_store.find_match(url, method);
        match result {
            Some((_id, resp)) => {
                let bytes = resp.body.len() as u64;
                Ok(HttpResponseResult {
                    status: resp.status,
                    headers: resp.headers,
                    body: resp.body,
                    body_type: resp.body_type,
                    duration_ms: 0,
                    bytes_received: bytes,
                })
            }
            None => {
                self.total_errors += 1;
                Err(VnetError::NoMockMatch(url.to_string()))
            }
        }
    }

    // ── Sandbox validation (for net:// requests) ──

    pub fn validate_net_request(&self, host: &str, port: u16, protocol: &str) -> Result<(), VnetError> {
        self.require_initialized()?;
        self.sandbox
            .validate(host, port, protocol)
            .map_err(VnetError::SandboxViolation)
    }

    pub fn check_connection_limit(&self) -> Result<(), VnetError> {
        if self.sessions.active_count() >= self.sandbox.max_connections {
            return Err(VnetError::LimitExceeded(format!(
                "max connections ({}) reached",
                self.sandbox.max_connections
            )));
        }
        Ok(())
    }

    pub fn default_timeout(&self) -> u64 {
        self.sandbox.default_timeout_ms
    }

    pub fn max_response_bytes(&self) -> u64 {
        self.sandbox.max_response_bytes
    }

    // ── Mock management ──

    pub fn register_mock(
        &mut self,
        method: Option<String>,
        url_pattern: &str,
        response: mock_store::MockResponse,
        times: Option<usize>,
    ) -> String {
        self.mock_store.register(method, url_pattern, response, times)
    }

    pub fn unregister_mock(&mut self, id: &str) -> Result<(), VnetError> {
        if self.mock_store.unregister(id) {
            Ok(())
        } else {
            Err(VnetError::MockNotFound(id.to_string()))
        }
    }

    pub fn list_mocks(&self) -> Vec<MockDefinitionInfo> {
        self.mock_store
            .list()
            .into_iter()
            .map(|m| MockDefinitionInfo {
                id: m.id,
                method: m.method,
                url_pattern: m.url_pattern,
                status: m.status,
                times: m.times,
                remaining: m.remaining,
            })
            .collect()
    }

    pub fn clear_mocks(&mut self) {
        self.mock_store.clear();
    }

    pub fn mock_history(&self) -> Vec<MockHitInfo> {
        self.mock_store
            .history()
            .iter()
            .map(|h| MockHitInfo {
                mock_id: h.mock_id.clone(),
                url: h.url.clone(),
                method: h.method.clone(),
                timestamp: h.timestamp,
            })
            .collect()
    }

    // ── Session management ──

    pub fn create_session(&mut self, session_type: SessionType, target: &str, scheme: Scheme) -> String {
        self.sessions.create(session_type, target, scheme)
    }

    pub fn get_session(&self, id: &str) -> Result<SessionInfo, VnetError> {
        let s = self.sessions.get(id).ok_or_else(|| VnetError::SessionNotFound(id.to_string()))?;
        Ok(SessionInfo {
            id: s.id.clone(),
            session_type: s.session_type.as_str().to_string(),
            target: s.target.clone(),
            scheme: s.scheme.as_str().to_string(),
            created_at: s.created_at,
            last_active_at: s.last_active_at,
            bytes_sent: s.bytes_sent,
            bytes_received: s.bytes_received,
        })
    }

    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .list()
            .into_iter()
            .map(|s| SessionInfo {
                id: s.id.clone(),
                session_type: s.session_type.as_str().to_string(),
                target: s.target.clone(),
                scheme: s.scheme.as_str().to_string(),
                created_at: s.created_at,
                last_active_at: s.last_active_at,
                bytes_sent: s.bytes_sent,
                bytes_received: s.bytes_received,
            })
            .collect()
    }

    pub fn close_session(&mut self, id: &str) -> Result<(), VnetError> {
        if self.sessions.close(id) {
            Ok(())
        } else {
            Err(VnetError::SessionNotFound(id.to_string()))
        }
    }

    // ── Metrics ──

    pub fn metrics(&self) -> MetricsResult {
        let bytes_total: u64 = self.sessions.list().iter().map(|s| s.bytes_sent + s.bytes_received).sum();
        MetricsResult {
            total_requests: self.total_requests,
            total_errors: self.total_errors,
            active_sessions: self.sessions.active_count(),
            bytes_total,
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vnet && cargo test -- network`
Expected: 10 tests PASS.

**Step 5: Commit**

```bash
git add simse-vnet/src/network.rs
git commit -m "feat(simse-vnet): add VirtualNetwork core with mock HTTP, sandbox, and metrics"
```

---

### Task 9: Server dispatch

**Files:**
- Modify: `simse-vnet/src/server.rs`

This wires all 19 JSON-RPC methods. The server reads stdin line-by-line, parses JSON-RPC requests, dispatches to the right handler, and writes responses. Follows the exact same pattern as `simse-vsh/src/server.rs`.

**Step 1: Implement server.rs**

No TDD for the server — it's pure wiring tested by integration tests (Task 10). Implementation:

```rust
use std::collections::HashMap;
use std::io::BufRead;

use crate::error::VnetError;
use crate::mock_store::MockResponse;
use crate::network::{SandboxInit, VirtualNetwork};
use crate::protocol::*;
use crate::transport::NdjsonTransport;

pub struct VnetServer {
    transport: NdjsonTransport,
    network: Option<VirtualNetwork>,
}

impl VnetServer {
    pub fn new(transport: NdjsonTransport) -> Self {
        Self {
            transport,
            network: None,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stdin = std::io::stdin();
        let reader = stdin.lock();
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let req: JsonRpcRequest = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("Invalid JSON-RPC: {}", e);
                    continue;
                }
            };
            self.dispatch(req).await;
        }
        Ok(())
    }

    async fn dispatch(&mut self, req: JsonRpcRequest) {
        let id = req.id;
        let method = req.method.as_str();
        let params = req.params;

        let result = match method {
            // ── Core ──
            "initialize" => self.handle_initialize(params),

            // ── Network ──
            "net/httpRequest" => self.handle_http_request(params),
            "net/wsConnect" => self.handle_ws_connect(params),
            "net/wsMessage" => self.handle_ws_message(params),
            "net/wsClose" => self.handle_ws_close(params),
            "net/tcpConnect" => self.handle_tcp_connect(params),
            "net/tcpSend" => self.handle_tcp_send(params),
            "net/tcpClose" => self.handle_tcp_close(params),
            "net/udpSend" => self.handle_udp_send(params),
            "net/resolve" => self.handle_resolve(params),
            "net/metrics" => self.handle_metrics(),

            // ── Mock ──
            "mock/register" => self.handle_mock_register(params),
            "mock/unregister" => self.handle_mock_unregister(params),
            "mock/list" => self.handle_mock_list(),
            "mock/clear" => self.handle_mock_clear(),
            "mock/history" => self.handle_mock_history(),

            // ── Session ──
            "session/list" => self.handle_session_list(),
            "session/get" => self.handle_session_get(params),
            "session/close" => self.handle_session_close(params),

            _ => {
                self.transport.write_error(
                    id,
                    METHOD_NOT_FOUND,
                    format!("Method not found: {method}"),
                    None,
                );
                return;
            }
        };

        match result {
            Ok(value) => self.transport.write_response(id, value),
            Err(e) => self.transport.write_error(
                id,
                VNET_ERROR,
                e.to_string(),
                Some(e.to_json_rpc_error()),
            ),
        }
    }

    fn with_network(&self) -> Result<&VirtualNetwork, VnetError> {
        self.network.as_ref().ok_or(VnetError::NotInitialized)
    }

    fn with_network_mut(&mut self) -> Result<&mut VirtualNetwork, VnetError> {
        self.network.as_mut().ok_or(VnetError::NotInitialized)
    }

    // ── Core handlers ──

    fn handle_initialize(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: InitializeParams = serde_json::from_value(params).map_err(|e| VnetError::InvalidParams(e.to_string()))?;
        let mut net = VirtualNetwork::new();
        let sandbox = p.sandbox.map(|s| SandboxInit {
            allowed_hosts: s.allowed_hosts.unwrap_or_default(),
            allowed_ports: s.allowed_ports.unwrap_or_default().into_iter().map(|p| (p.start, p.end)).collect(),
            allowed_protocols: s.allowed_protocols.unwrap_or_default(),
            default_timeout_ms: s.default_timeout_ms.unwrap_or(30_000),
            max_response_bytes: s.max_response_bytes.unwrap_or(10 * 1024 * 1024),
            max_connections: s.max_connections.unwrap_or(50),
        });
        net.initialize(sandbox);
        self.network = Some(net);
        Ok(serde_json::json!({ "ok": true }))
    }

    // ── Network handlers ──

    fn handle_http_request(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: HttpRequestParams = parse_params(params)?;

        if p.url.starts_with("mock://") {
            let net = self.with_network_mut()?;
            let result = net.mock_http_request(
                &p.url,
                p.method.as_deref(),
                p.headers.as_ref(),
                p.body.as_deref(),
            )?;
            Ok(serde_json::to_value(result)?)
        } else if p.url.starts_with("net://") {
            // Real HTTP: extract host, validate sandbox, execute with reqwest
            // Stub for now — real implementation needs reqwest + async
            Err(VnetError::ConnectionFailed("net:// HTTP not yet implemented".into()))
        } else {
            Err(VnetError::InvalidParams(format!("URL must start with mock:// or net://: {}", p.url)))
        }
    }

    fn handle_ws_connect(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: WsConnectParams = parse_params(params)?;

        if p.url.starts_with("mock://") {
            let net = self.with_network_mut()?;
            // For mock WS, create a session and return it
            let target = p.url.strip_prefix("mock://").unwrap_or(&p.url).to_string();
            let session_id = net.create_session(
                crate::session::SessionType::Ws,
                &target,
                crate::session::Scheme::Mock,
            );
            Ok(serde_json::json!({ "sessionId": session_id, "status": "connected" }))
        } else {
            Err(VnetError::ConnectionFailed("net:// WebSocket not yet implemented".into()))
        }
    }

    fn handle_ws_message(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: WsMessageParams = parse_params(params)?;
        let net = self.with_network_mut()?;
        // Verify session exists
        net.get_session(&p.session_id)?;
        net.sessions.record_activity(&p.session_id, p.data.len() as u64, 0);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn handle_ws_close(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: SessionIdParam = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.close_session(&p.session_id)?;
        Ok(serde_json::json!({ "ok": true }))
    }

    fn handle_tcp_connect(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: TcpConnectParams = parse_params(params)?;
        let target = format!("{}:{}", p.host, p.port);

        // Determine scheme from host prefix
        if p.host.starts_with("mock://") || target.starts_with("mock://") {
            let net = self.with_network_mut()?;
            let clean_target = target.strip_prefix("mock://").unwrap_or(&target);
            let session_id = net.create_session(
                crate::session::SessionType::Tcp,
                clean_target,
                crate::session::Scheme::Mock,
            );
            Ok(serde_json::json!({ "sessionId": session_id, "status": "connected" }))
        } else {
            Err(VnetError::ConnectionFailed("net:// TCP not yet implemented".into()))
        }
    }

    fn handle_tcp_send(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: TcpSendParams = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.get_session(&p.session_id)?;
        let bytes = p.data.len() as u64;
        net.sessions.record_activity(&p.session_id, bytes, 0);
        Ok(serde_json::json!({ "bytesWritten": bytes }))
    }

    fn handle_tcp_close(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: SessionIdParam = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.close_session(&p.session_id)?;
        Ok(serde_json::json!({ "ok": true }))
    }

    fn handle_udp_send(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: UdpSendParams = parse_params(params)?;
        let url = format!("mock://{}:{}", p.host, p.port);

        let net = self.with_network_mut()?;
        // Try mock match for UDP
        let result = net.mock_store_find_match(&url, Some("udp"));
        match result {
            Some(resp) => {
                let bytes = resp.body.len() as u64;
                Ok(serde_json::json!({
                    "response": resp.body,
                    "bytesReceived": bytes
                }))
            }
            None => Ok(serde_json::json!({ "response": null, "bytesReceived": 0 })),
        }
    }

    fn handle_resolve(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: ResolveParams = parse_params(params)?;
        let mock_url = format!("mock://dns/{}", p.hostname);
        let net = self.with_network_mut()?;
        net.require_init()?;
        net.total_requests += 1;

        let result = net.mock_store_find_match(&mock_url, Some("dns"));
        match result {
            Some(resp) => {
                // Parse body as JSON array of addresses
                let addresses: Vec<String> = serde_json::from_str(&resp.body).unwrap_or_default();
                Ok(serde_json::json!({ "addresses": addresses, "ttl": null }))
            }
            None => {
                // No mock — could do real DNS lookup if net:// context, but for now error
                net.total_errors += 1;
                Err(VnetError::DnsResolutionFailed(p.hostname))
            }
        }
    }

    fn handle_metrics(&self) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network()?;
        let m = net.metrics();
        Ok(serde_json::to_value(m)?)
    }

    // ── Mock handlers ──

    fn handle_mock_register(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: MockRegisterParams = parse_params(params)?;
        let net = self.with_network_mut()?;
        let response = MockResponse {
            status: p.response.status,
            headers: p.response.headers,
            body: p.response.body,
            body_type: p.response.body_type,
            delay_ms: p.response.delay_ms,
        };
        let id = net.register_mock(p.method, &p.url_pattern, response, p.times);
        Ok(serde_json::json!({ "id": id }))
    }

    fn handle_mock_unregister(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: MockIdParam = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.unregister_mock(&p.id)?;
        Ok(serde_json::json!({ "ok": true }))
    }

    fn handle_mock_list(&self) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network()?;
        let mocks = net.list_mocks();
        Ok(serde_json::to_value(mocks)?)
    }

    fn handle_mock_clear(&mut self) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network_mut()?;
        net.clear_mocks();
        Ok(serde_json::json!({ "ok": true }))
    }

    fn handle_mock_history(&self) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network()?;
        let history = net.mock_history();
        Ok(serde_json::to_value(history)?)
    }

    // ── Session handlers ──

    fn handle_session_list(&self) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network()?;
        let sessions = net.list_sessions();
        Ok(serde_json::to_value(sessions)?)
    }

    fn handle_session_get(&self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: SessionIdParam = parse_params(params)?;
        let net = self.with_network()?;
        let info = net.get_session(&p.session_id)?;
        Ok(serde_json::to_value(info)?)
    }

    fn handle_session_close(&mut self, params: serde_json::Value) -> Result<serde_json::Value, VnetError> {
        let p: SessionIdParam = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.close_session(&p.session_id)?;
        Ok(serde_json::json!({ "ok": true }))
    }
}

fn parse_params<T: serde::de::DeserializeOwned>(params: serde_json::Value) -> Result<T, VnetError> {
    serde_json::from_value(params).map_err(|e| VnetError::InvalidParams(e.to_string()))
}
```

**Important:** The `handle_udp_send` and `handle_resolve` handlers reference methods on `VirtualNetwork` that don't exist yet. Add these two helper methods to `network.rs`:

```rust
// Add to VirtualNetwork impl:
pub fn mock_store_find_match(&mut self, url: &str, method: Option<&str>) -> Option<mock_store::MockResponse> {
    self.mock_store.find_match(url, method).map(|(_, resp)| resp)
}

pub fn require_init(&self) -> Result<(), VnetError> {
    self.require_initialized()
}
```

Also make `total_requests` and `total_errors` `pub(crate)` in `VirtualNetwork`.

**Step 2: Verify it builds**

Run: `cd simse-vnet && cargo build`
Expected: Compiles. Fix any type mismatches.

**Step 3: Commit**

```bash
git add simse-vnet/src/server.rs simse-vnet/src/network.rs
git commit -m "feat(simse-vnet): add VnetServer with 19 JSON-RPC method dispatch"
```

---

### Task 10: Integration tests

**Files:**
- Create: `simse-vnet/tests/integration.rs`

These tests spawn the `simse-vnet-engine` binary and exercise the JSON-RPC interface end-to-end. They follow the exact same pattern as `simse-vsh/tests/integration.rs`.

**Step 1: Write integration tests**

```rust
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

struct VnetProcess {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    next_id: AtomicU64,
}

#[derive(Debug)]
enum RpcResponse {
    Ok(Value),
    Error(Value),
}

impl VnetProcess {
    fn spawn() -> Self {
        let bin = env!("CARGO_BIN_EXE_simse-vnet-engine");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn simse-vnet-engine");

        let stdout = child.stdout.take().expect("no stdout");
        let reader = BufReader::new(stdout);

        Self {
            child,
            reader,
            next_id: AtomicU64::new(1),
        }
    }

    fn send(&mut self, method: &str, params: Value) -> RpcResponse {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let stdin = self.child.stdin.as_mut().expect("no stdin");
        let mut line = serde_json::to_string(&request).unwrap();
        line.push('\n');
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.flush().unwrap();

        loop {
            let mut buf = String::new();
            let bytes_read = self
                .reader
                .read_line(&mut buf)
                .expect("failed to read from stdout");
            if bytes_read == 0 {
                panic!("unexpected EOF while waiting for response to id={}", id);
            }
            let buf = buf.trim();
            if buf.is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(buf)
                .unwrap_or_else(|e| panic!("invalid JSON: {e}\nline: {buf}"));

            if parsed.get("id").is_none() {
                continue; // skip notifications
            }

            let resp_id = parsed["id"].as_u64().expect("response id is not u64");
            assert_eq!(resp_id, id, "response id mismatch");

            if let Some(error) = parsed.get("error") {
                return RpcResponse::Error(error.clone());
            }
            return RpcResponse::Ok(parsed.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        match self.send(method, params) {
            RpcResponse::Ok(v) => v,
            RpcResponse::Error(e) => panic!("expected success, got error: {e}"),
        }
    }

    fn call_err(&mut self, method: &str, params: Value) -> Value {
        match self.send(method, params) {
            RpcResponse::Error(e) => e,
            RpcResponse::Ok(v) => panic!("expected error, got success: {v}"),
        }
    }

    fn initialize(&mut self) {
        self.call("initialize", json!({}));
    }

    fn initialize_with_sandbox(&mut self, sandbox: Value) {
        self.call("initialize", json!({ "sandbox": sandbox }));
    }
}

impl Drop for VnetProcess {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

// ── Tests ──

#[test]
fn initialize_returns_ok() {
    let mut proc = VnetProcess::spawn();
    let result = proc.call("initialize", json!({}));
    assert_eq!(result["ok"], true);
}

#[test]
fn method_before_init_returns_error() {
    let mut proc = VnetProcess::spawn();
    let err = proc.call_err("net/metrics", json!({}));
    assert_eq!(err["data"]["vnetCode"], "VNET_NOT_INITIALIZED");
}

#[test]
fn unknown_method_returns_error() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();
    let err = proc.call_err("nonexistent/method", json!({}));
    assert_eq!(err["code"], -32601);
}

#[test]
fn mock_register_and_http_request() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    // Register a mock
    let reg = proc.call("mock/register", json!({
        "urlPattern": "mock://api.example.com/users",
        "method": "GET",
        "response": {
            "status": 200,
            "headers": {"Content-Type": "application/json"},
            "body": "[{\"id\":1}]",
            "bodyType": "text"
        }
    }));
    assert!(reg["id"].is_string());

    // Make request
    let resp = proc.call("net/httpRequest", json!({
        "url": "mock://api.example.com/users",
        "method": "GET"
    }));
    assert_eq!(resp["status"], 200);
    assert_eq!(resp["body"], "[{\"id\":1}]");
    assert_eq!(resp["bodyType"], "text");
}

#[test]
fn mock_glob_pattern() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    proc.call("mock/register", json!({
        "urlPattern": "mock://api/*",
        "response": { "status": 200, "body": "ok", "bodyType": "text" }
    }));

    let resp = proc.call("net/httpRequest", json!({
        "url": "mock://api/anything/here"
    }));
    assert_eq!(resp["status"], 200);
}

#[test]
fn mock_no_match_returns_error() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let err = proc.call_err("net/httpRequest", json!({
        "url": "mock://nothing"
    }));
    assert_eq!(err["data"]["vnetCode"], "VNET_NO_MOCK_MATCH");
}

#[test]
fn mock_times_limit() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    proc.call("mock/register", json!({
        "urlPattern": "mock://once",
        "response": { "status": 200, "body": "x", "bodyType": "text" },
        "times": 1
    }));

    proc.call("net/httpRequest", json!({ "url": "mock://once" }));
    let err = proc.call_err("net/httpRequest", json!({ "url": "mock://once" }));
    assert_eq!(err["data"]["vnetCode"], "VNET_NO_MOCK_MATCH");
}

#[test]
fn mock_list_and_unregister() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let reg = proc.call("mock/register", json!({
        "urlPattern": "mock://a",
        "response": { "status": 200, "body": "", "bodyType": "text" }
    }));
    let id = reg["id"].as_str().unwrap().to_string();

    let list = proc.call("mock/list", json!({}));
    assert_eq!(list.as_array().unwrap().len(), 1);

    proc.call("mock/unregister", json!({ "id": id }));
    let list = proc.call("mock/list", json!({}));
    assert!(list.as_array().unwrap().is_empty());
}

#[test]
fn mock_clear_and_history() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    proc.call("mock/register", json!({
        "urlPattern": "mock://test/*",
        "response": { "status": 200, "body": "ok", "bodyType": "text" }
    }));

    proc.call("net/httpRequest", json!({ "url": "mock://test/1" }));
    proc.call("net/httpRequest", json!({ "url": "mock://test/2" }));

    let history = proc.call("mock/history", json!({}));
    assert_eq!(history.as_array().unwrap().len(), 2);

    proc.call("mock/clear", json!({}));
    let list = proc.call("mock/list", json!({}));
    assert!(list.as_array().unwrap().is_empty());
}

#[test]
fn ws_connect_send_close() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let conn = proc.call("net/wsConnect", json!({
        "url": "mock://ws.example.com/chat"
    }));
    let session_id = conn["sessionId"].as_str().unwrap().to_string();
    assert_eq!(conn["status"], "connected");

    proc.call("net/wsMessage", json!({
        "sessionId": session_id,
        "data": "hello"
    }));

    proc.call("net/wsClose", json!({ "sessionId": session_id }));

    // Session should be gone
    let err = proc.call_err("session/get", json!({ "sessionId": session_id }));
    assert_eq!(err["data"]["vnetCode"], "VNET_SESSION_NOT_FOUND");
}

#[test]
fn session_list_and_get() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let conn = proc.call("net/wsConnect", json!({
        "url": "mock://ws.example.com/test"
    }));
    let session_id = conn["sessionId"].as_str().unwrap().to_string();

    let list = proc.call("session/list", json!({}));
    assert_eq!(list.as_array().unwrap().len(), 1);

    let info = proc.call("session/get", json!({ "sessionId": session_id }));
    assert_eq!(info["sessionType"], "ws");
    assert_eq!(info["scheme"], "mock");
}

#[test]
fn metrics_track_activity() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    proc.call("mock/register", json!({
        "urlPattern": "mock://m/*",
        "response": { "status": 200, "body": "ok", "bodyType": "text" }
    }));

    proc.call("net/httpRequest", json!({ "url": "mock://m/1" }));
    proc.call("net/httpRequest", json!({ "url": "mock://m/2" }));

    let m = proc.call("net/metrics", json!({}));
    assert_eq!(m["totalRequests"], 2);
    assert_eq!(m["totalErrors"], 0);
}

#[test]
fn invalid_url_scheme_rejected() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let err = proc.call_err("net/httpRequest", json!({
        "url": "https://example.com"
    }));
    assert_eq!(err["data"]["vnetCode"], "VNET_INVALID_PARAMS");
}
```

**Step 2: Run integration tests**

Run: `cd simse-vnet && cargo test --test integration`
Expected: All 14 tests PASS.

**Step 3: Commit**

```bash
git add simse-vnet/tests/integration.rs
git commit -m "test(simse-vnet): add 14 integration tests for mock HTTP, WS, sessions, and metrics"
```

---

### Task 11: Documentation update

**Files:**
- Modify: `CLAUDE.md`
- Modify: `Cargo.toml` (root workspace)

**Step 1: Add simse-vnet to CLAUDE.md**

In the "Repository Layout" section, add after the `simse-vsh/` entry:

```
simse-vnet/                 # Pure Rust crate — virtual network engine (JSON-RPC over stdio)
```

In the "Other Rust Crates" section, add:

```
simse-vnet/                 # Pure Rust crate — virtual network
  src/
    error.rs                # VnetError enum (thiserror, VNET_ prefix)
    protocol.rs             # JSON-RPC request/response types
    transport.rs            # NdjsonTransport (shared pattern)
    server.rs               # VnetServer: 19-method JSON-RPC dispatch
    network.rs              # VirtualNetwork: core logic, mock HTTP, sandbox
    sandbox.rs              # NetSandboxConfig: host/port/protocol allowlist
    mock_store.rs           # MockStore: mock registry + pattern matching
    session.rs              # SessionManager: persistent connection tracking
```

In the "Commands" section, add:

```bash
bun run build:vnet-engine    # cd simse-vnet && cargo build --release
cd simse-vnet && cargo test  # Rust vnet engine tests
```

**Step 2: Add simse-vnet to workspace exclude list**

In root `Cargo.toml`, add `"simse-vnet"` to the `exclude` array.

**Step 3: Commit**

```bash
git add CLAUDE.md Cargo.toml
git commit -m "docs: add simse-vnet to CLAUDE.md and workspace exclude list"
```

---

**Total: 11 tasks, ~26 unit tests + 14 integration tests.**

**Note on net:// real networking:** Tasks 1-11 implement the full `mock://` path and sandbox validation. Real `net://` HTTP/WS/TCP/UDP using reqwest and tokio-tungstenite can be added as follow-up tasks once the mock foundation is solid. The server handlers already have the `net://` branch stubs returning `ConnectionFailed` errors, making it straightforward to wire in real implementations later.
