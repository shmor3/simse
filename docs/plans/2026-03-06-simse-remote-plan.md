# simse-remote Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build simse-remote (Rust crate) for API authentication and WebSocket tunnel, plus simse-relay (Cloudflare Worker) for cloud-side WebSocket relay with Durable Objects.

**Architecture:** simse-remote is a standalone Rust binary (JSON-RPC 2.0 / NDJSON stdio) that authenticates with simse-api and opens a WebSocket tunnel to simse-relay. simse-relay is a Cloudflare Worker using Durable Objects to pair tunnel ↔ client WebSocket connections for bidirectional JSON-RPC proxying.

**Tech Stack:** Rust (tokio, tokio-tungstenite, reqwest, serde, thiserror, tracing), TypeScript (Hono, Cloudflare Workers, Durable Objects, Vitest)

---

## Task 1: Scaffold simse-remote Rust crate

**Files:**
- Create: `simse-remote/Cargo.toml`
- Create: `simse-remote/src/lib.rs`
- Create: `simse-remote/src/main.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "simse-remote-engine"
version = "0.1.0"
edition = "2024"
license = "MIT"
rust-version = "1.85"
description = "Remote access engine over JSON-RPC 2.0 / NDJSON stdio"

[lib]
name = "simse_remote_engine"
path = "src/lib.rs"

[[bin]]
name = "simse-remote-engine"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = { version = "0.26", features = ["native-tls"] }
reqwest = { version = "0.12", features = ["json"] }
uuid = { version = "1", features = ["v4"] }
futures = "0.3"

[dev-dependencies]
tempfile = "3"
```

**Step 2: Create lib.rs**

```rust
pub mod error;
pub mod protocol;
pub mod transport;
pub mod server;
pub mod auth;
pub mod tunnel;
pub mod router;
pub mod heartbeat;
```

**Step 3: Create main.rs**

```rust
use simse_remote_engine::server::RemoteServer;
use simse_remote_engine::transport::NdjsonTransport;

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
    let mut server = RemoteServer::new(transport);

    tracing::info!("simse-remote-engine ready");

    if let Err(e) = server.run().await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

**Step 4: Verify it compiles (will fail — modules are empty)**

Run: `cd simse-remote && cargo check 2>&1 | head -5`
Expected: Errors about missing module files (this is expected, we'll fill them in)

**Step 5: Commit**

```bash
git add simse-remote/
git commit -m "feat(simse-remote): scaffold crate with Cargo.toml, lib.rs, main.rs"
```

---

## Task 2: Implement error.rs and protocol.rs

**Files:**
- Create: `simse-remote/src/error.rs`
- Create: `simse-remote/src/protocol.rs`

**Step 1: Write error.rs tests and implementation**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RemoteError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Auth failed: {0}")]
    AuthFailed(String),
    #[error("Token expired")]
    TokenExpired,
    #[error("Tunnel not connected")]
    TunnelNotConnected,
    #[error("Tunnel already connected")]
    TunnelAlreadyConnected,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Relay error: {0}")]
    RelayError(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}

impl RemoteError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "REMOTE_NOT_INITIALIZED",
            Self::NotAuthenticated => "REMOTE_NOT_AUTHENTICATED",
            Self::AuthFailed(_) => "REMOTE_AUTH_FAILED",
            Self::TokenExpired => "REMOTE_TOKEN_EXPIRED",
            Self::TunnelNotConnected => "REMOTE_TUNNEL_NOT_CONNECTED",
            Self::TunnelAlreadyConnected => "REMOTE_TUNNEL_ALREADY_CONNECTED",
            Self::ConnectionFailed(_) => "REMOTE_CONNECTION_FAILED",
            Self::Timeout(_) => "REMOTE_TIMEOUT",
            Self::RelayError(_) => "REMOTE_RELAY_ERROR",
            Self::InvalidParams(_) => "REMOTE_INVALID_PARAMS",
            Self::Io(_) => "REMOTE_IO_ERROR",
            Self::Json(_) => "REMOTE_JSON_ERROR",
            Self::Http(_) => "REMOTE_HTTP_ERROR",
            Self::WebSocket(_) => "REMOTE_WEBSOCKET_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "remoteCode": self.code(),
            "message": self.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_remote_prefix() {
        let cases = vec![
            (RemoteError::NotInitialized, "REMOTE_NOT_INITIALIZED"),
            (RemoteError::NotAuthenticated, "REMOTE_NOT_AUTHENTICATED"),
            (RemoteError::AuthFailed("test".into()), "REMOTE_AUTH_FAILED"),
            (RemoteError::TokenExpired, "REMOTE_TOKEN_EXPIRED"),
            (RemoteError::TunnelNotConnected, "REMOTE_TUNNEL_NOT_CONNECTED"),
            (RemoteError::TunnelAlreadyConnected, "REMOTE_TUNNEL_ALREADY_CONNECTED"),
            (RemoteError::ConnectionFailed("test".into()), "REMOTE_CONNECTION_FAILED"),
            (RemoteError::Timeout("test".into()), "REMOTE_TIMEOUT"),
            (RemoteError::RelayError("test".into()), "REMOTE_RELAY_ERROR"),
            (RemoteError::InvalidParams("test".into()), "REMOTE_INVALID_PARAMS"),
            (RemoteError::WebSocket("test".into()), "REMOTE_WEBSOCKET_ERROR"),
        ];
        for (err, expected_code) in cases {
            assert_eq!(err.code(), expected_code, "wrong code for {err}");
        }
    }

    #[test]
    fn to_json_rpc_error_has_remote_code() {
        let err = RemoteError::AuthFailed("bad credentials".into());
        let json = err.to_json_rpc_error();
        assert_eq!(json["remoteCode"], "REMOTE_AUTH_FAILED");
        assert_eq!(json["message"], "Auth failed: bad credentials");
    }
}
```

**Step 2: Write protocol.rs**

```rust
use serde::{Deserialize, Serialize};

// ── JSON-RPC framing ──

pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const REMOTE_ERROR: i32 = -32000;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

// ── Auth methods ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginParams {
    pub api_url: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    pub user_id: String,
    pub session_token: String,
    pub team_id: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResult {
    pub authenticated: bool,
    pub user_id: Option<String>,
    pub team_id: Option<String>,
    pub role: Option<String>,
    pub api_url: Option<String>,
}

// ── Tunnel methods ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelConnectParams {
    pub relay_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelConnectResult {
    pub tunnel_id: String,
    pub relay_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatusResult {
    pub connected: bool,
    pub tunnel_id: Option<String>,
    pub relay_url: Option<String>,
    pub uptime_ms: Option<u64>,
    pub reconnect_count: u32,
}

// ── Health ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResult {
    pub ok: bool,
    pub authenticated: bool,
    pub tunnel_connected: bool,
}

// ── Helpers ──

pub fn parse_params<T: serde::de::DeserializeOwned>(
    params: serde_json::Value,
) -> Result<T, crate::error::RemoteError> {
    serde_json::from_value(params).map_err(|e| {
        crate::error::RemoteError::InvalidParams(e.to_string())
    })
}
```

**Step 3: Run tests**

Run: `cd simse-remote && cargo test error`
Expected: 2 tests pass

**Step 4: Commit**

```bash
git add simse-remote/src/error.rs simse-remote/src/protocol.rs
git commit -m "feat(simse-remote): add error types and JSON-RPC protocol types"
```

---

## Task 3: Implement transport.rs

**Files:**
- Create: `simse-remote/src/transport.rs`

**Step 1: Write transport.rs**

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

**Step 2: Verify it compiles**

Run: `cd simse-remote && cargo check 2>&1 | head -5`
Expected: Errors only about missing auth/tunnel/router/heartbeat/server modules

**Step 3: Commit**

```bash
git add simse-remote/src/transport.rs
git commit -m "feat(simse-remote): add NDJSON transport layer"
```

---

## Task 4: Implement auth.rs

**Files:**
- Create: `simse-remote/src/auth.rs`

**Step 1: Write auth.rs**

```rust
use serde::Deserialize;

use crate::error::RemoteError;

// ── Auth state ──

#[derive(Debug, Clone)]
pub struct AuthState {
    pub user_id: String,
    pub session_token: String,
    pub team_id: Option<String>,
    pub role: Option<String>,
    pub api_url: String,
}

// ── API response types ──

#[derive(Debug, Deserialize)]
struct LoginResponse {
    data: LoginData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginData {
    session_token: String,
    user: LoginUser,
}

#[derive(Debug, Deserialize)]
struct LoginUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ValidateResponse {
    data: ValidateData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateData {
    user_id: String,
    team_id: Option<String>,
    role: Option<String>,
}

// ── Auth client ──

pub struct AuthClient {
    http: reqwest::Client,
    state: Option<AuthState>,
}

impl Default for AuthClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            state: None,
        }
    }

    pub fn state(&self) -> Option<&AuthState> {
        self.state.as_ref()
    }

    pub fn is_authenticated(&self) -> bool {
        self.state.is_some()
    }

    /// Login with email/password. Returns auth state on success.
    pub async fn login_password(
        &mut self,
        api_url: &str,
        email: &str,
        password: &str,
    ) -> Result<AuthState, RemoteError> {
        let url = format!("{api_url}/auth/login");
        let res = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "email": email,
                "password": password,
            }))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(RemoteError::AuthFailed(format!(
                "HTTP {status}: {body}"
            )));
        }

        let login_resp: LoginResponse = res.json().await?;

        // Validate the token to get team/role info
        let validate = self
            .validate_token(api_url, &login_resp.data.session_token)
            .await?;

        let state = AuthState {
            user_id: login_resp.data.user.id,
            session_token: login_resp.data.session_token,
            team_id: validate.team_id,
            role: validate.role,
            api_url: api_url.to_string(),
        };

        self.state = Some(state.clone());
        Ok(state)
    }

    /// Login with API key. Returns auth state on success.
    pub async fn login_api_key(
        &mut self,
        api_url: &str,
        api_key: &str,
    ) -> Result<AuthState, RemoteError> {
        let validate = self.validate_token(api_url, api_key).await?;

        let state = AuthState {
            user_id: validate.user_id,
            session_token: api_key.to_string(),
            team_id: validate.team_id,
            role: validate.role,
            api_url: api_url.to_string(),
        };

        self.state = Some(state.clone());
        Ok(state)
    }

    /// Logout: clear local state.
    pub fn logout(&mut self) {
        self.state = None;
    }

    /// Validate a token against the auth service.
    async fn validate_token(
        &self,
        api_url: &str,
        token: &str,
    ) -> Result<ValidateData, RemoteError> {
        let url = format!("{api_url}/auth/validate");
        let res = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "token": token }))
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(RemoteError::AuthFailed(
                "Token validation failed".to_string(),
            ));
        }

        let validate: ValidateResponse = res.json().await?;
        Ok(validate.data)
    }

    /// Get current token, or error if not authenticated.
    pub fn require_auth(&self) -> Result<&AuthState, RemoteError> {
        self.state.as_ref().ok_or(RemoteError::NotAuthenticated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_client_is_not_authenticated() {
        let client = AuthClient::new();
        assert!(!client.is_authenticated());
        assert!(client.state().is_none());
    }

    #[test]
    fn require_auth_fails_when_not_authenticated() {
        let client = AuthClient::new();
        let result = client.require_auth();
        assert!(result.is_err());
        match result.unwrap_err() {
            RemoteError::NotAuthenticated => {}
            other => panic!("expected NotAuthenticated, got {other}"),
        }
    }

    #[test]
    fn logout_clears_state() {
        let mut client = AuthClient::new();
        client.state = Some(AuthState {
            user_id: "u1".into(),
            session_token: "session_abc".into(),
            team_id: None,
            role: None,
            api_url: "https://api.example.com".into(),
        });
        assert!(client.is_authenticated());
        client.logout();
        assert!(!client.is_authenticated());
    }
}
```

**Step 2: Run tests**

Run: `cd simse-remote && cargo test auth`
Expected: 3 tests pass

**Step 3: Commit**

```bash
git add simse-remote/src/auth.rs
git commit -m "feat(simse-remote): add auth client with login/logout/validate"
```

---

## Task 5: Implement heartbeat.rs

**Files:**
- Create: `simse-remote/src/heartbeat.rs`

**Step 1: Write heartbeat.rs**

```rust
use std::time::Duration;

/// Exponential backoff configuration for reconnection.
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    pub initial_ms: u64,
    pub max_ms: u64,
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_ms: 1_000,
            max_ms: 30_000,
            multiplier: 2.0,
        }
    }
}

/// Tracks reconnection attempts with exponential backoff.
pub struct Backoff {
    config: BackoffConfig,
    attempt: u32,
}

impl Backoff {
    pub fn new(config: BackoffConfig) -> Self {
        Self { config, attempt: 0 }
    }

    /// Get the next backoff duration and increment the attempt counter.
    pub fn next_delay(&mut self) -> Duration {
        let delay_ms = (self.config.initial_ms as f64
            * self.config.multiplier.powi(self.attempt as i32))
            as u64;
        let clamped = delay_ms.min(self.config.max_ms);
        self.attempt += 1;
        Duration::from_millis(clamped)
    }

    /// Reset on successful connection.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Current attempt count.
    pub fn attempts(&self) -> u32 {
        self.attempt
    }
}

/// Keepalive ping interval.
pub const PING_INTERVAL_MS: u64 = 30_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_exponential() {
        let mut b = Backoff::new(BackoffConfig {
            initial_ms: 1_000,
            max_ms: 30_000,
            multiplier: 2.0,
        });
        assert_eq!(b.next_delay(), Duration::from_millis(1_000));
        assert_eq!(b.next_delay(), Duration::from_millis(2_000));
        assert_eq!(b.next_delay(), Duration::from_millis(4_000));
        assert_eq!(b.next_delay(), Duration::from_millis(8_000));
        assert_eq!(b.next_delay(), Duration::from_millis(16_000));
        assert_eq!(b.next_delay(), Duration::from_millis(30_000)); // clamped
        assert_eq!(b.next_delay(), Duration::from_millis(30_000)); // still clamped
    }

    #[test]
    fn backoff_reset() {
        let mut b = Backoff::new(BackoffConfig::default());
        b.next_delay();
        b.next_delay();
        assert_eq!(b.attempts(), 2);
        b.reset();
        assert_eq!(b.attempts(), 0);
        assert_eq!(b.next_delay(), Duration::from_millis(1_000));
    }
}
```

**Step 2: Run tests**

Run: `cd simse-remote && cargo test heartbeat`
Expected: 2 tests pass

**Step 3: Commit**

```bash
git add simse-remote/src/heartbeat.rs
git commit -m "feat(simse-remote): add heartbeat with exponential backoff"
```

---

## Task 6: Implement tunnel.rs

**Files:**
- Create: `simse-remote/src/tunnel.rs`

**Step 1: Write tunnel.rs**

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::error::RemoteError;
use crate::heartbeat::{Backoff, BackoffConfig, PING_INTERVAL_MS};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsSource = SplitStream<WsStream>;

/// Tunnel state visible to the server.
#[derive(Debug, Clone)]
pub struct TunnelState {
    pub connected: bool,
    pub tunnel_id: Option<String>,
    pub relay_url: Option<String>,
    pub connected_at: Option<Instant>,
    pub reconnect_count: u32,
}

impl Default for TunnelState {
    fn default() -> Self {
        Self {
            connected: false,
            tunnel_id: None,
            relay_url: None,
            connected_at: None,
            reconnect_count: 0,
        }
    }
}

/// Manages the WebSocket tunnel to the relay.
pub struct TunnelClient {
    state: Arc<Mutex<TunnelState>>,
    sink: Arc<Mutex<Option<WsSink>>>,
    connected: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl Default for TunnelClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TunnelClient {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TunnelState::default())),
            sink: Arc::new(Mutex::new(None)),
            connected: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub async fn get_state(&self) -> TunnelState {
        self.state.lock().await.clone()
    }

    /// Connect to the relay WebSocket endpoint.
    pub async fn connect(
        &self,
        relay_url: &str,
        token: &str,
    ) -> Result<String, RemoteError> {
        if self.connected.load(Ordering::SeqCst) {
            return Err(RemoteError::TunnelAlreadyConnected);
        }

        let url = format!("{relay_url}/ws/tunnel?token={token}");
        let (ws_stream, _response) = connect_async(&url)
            .await
            .map_err(|e| RemoteError::ConnectionFailed(e.to_string()))?;

        let tunnel_id = uuid::Uuid::new_v4().to_string();
        let (sink, source) = ws_stream.split();

        *self.sink.lock().await = Some(sink);
        self.connected.store(true, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);

        {
            let mut state = self.state.lock().await;
            state.connected = true;
            state.tunnel_id = Some(tunnel_id.clone());
            state.relay_url = Some(relay_url.to_string());
            state.connected_at = Some(Instant::now());
        }

        // Spawn reader task
        let connected = self.connected.clone();
        let state = self.state.clone();
        let cancel = self.cancel.clone();
        let sink_ref = self.sink.clone();
        let relay_url_owned = relay_url.to_string();
        let token_owned = token.to_string();

        tokio::spawn(async move {
            Self::reader_loop(
                source,
                sink_ref,
                connected,
                state,
                cancel,
                relay_url_owned,
                token_owned,
            )
            .await;
        });

        tracing::info!("Tunnel connected: {tunnel_id}");
        Ok(tunnel_id)
    }

    /// Disconnect the tunnel.
    pub async fn disconnect(&self) -> Result<(), RemoteError> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(RemoteError::TunnelNotConnected);
        }

        self.cancel.store(true, Ordering::SeqCst);
        self.connected.store(false, Ordering::SeqCst);

        // Close the WebSocket
        if let Some(mut sink) = self.sink.lock().await.take() {
            let _ = sink.close().await;
        }

        {
            let mut state = self.state.lock().await;
            state.connected = false;
            state.tunnel_id = None;
            state.connected_at = None;
        }

        tracing::info!("Tunnel disconnected");
        Ok(())
    }

    /// Send a message through the tunnel.
    pub async fn send_message(&self, msg: &str) -> Result<(), RemoteError> {
        let mut sink_guard = self.sink.lock().await;
        let sink = sink_guard
            .as_mut()
            .ok_or(RemoteError::TunnelNotConnected)?;
        sink.send(Message::Text(msg.to_string()))
            .await
            .map_err(|e| RemoteError::WebSocket(e.to_string()))?;
        Ok(())
    }

    /// Reader loop: receives messages from relay, handles pings, reconnection.
    async fn reader_loop(
        mut source: WsSource,
        sink: Arc<Mutex<Option<WsSink>>>,
        connected: Arc<AtomicBool>,
        state: Arc<Mutex<TunnelState>>,
        cancel: Arc<AtomicBool>,
        relay_url: String,
        token: String,
    ) {
        let mut backoff = Backoff::new(BackoffConfig::default());
        let mut ping_interval =
            tokio::time::interval(std::time::Duration::from_millis(PING_INTERVAL_MS));

        loop {
            if cancel.load(Ordering::SeqCst) {
                break;
            }

            tokio::select! {
                msg = source.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            // Forward to local core via transport notification
                            // The server will handle routing
                            tracing::debug!("Received from relay: {}", &text[..text.len().min(200)]);
                            // TODO: Route to local simse-core
                        }
                        Some(Ok(Message::Ping(data))) => {
                            if let Some(ref mut s) = *sink.lock().await {
                                let _ = s.send(Message::Pong(data)).await;
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            tracing::warn!("WebSocket closed, attempting reconnect");
                            connected.store(false, Ordering::SeqCst);
                            state.lock().await.connected = false;

                            // Reconnect loop
                            loop {
                                if cancel.load(Ordering::SeqCst) {
                                    return;
                                }
                                let delay = backoff.next_delay();
                                tracing::info!("Reconnecting in {:?} (attempt {})", delay, backoff.attempts());
                                tokio::time::sleep(delay).await;

                                let url = format!("{relay_url}/ws/tunnel?token={token}");
                                match connect_async(&url).await {
                                    Ok((ws_stream, _)) => {
                                        let (new_sink, new_source) = ws_stream.split();
                                        *sink.lock().await = Some(new_sink);
                                        source = new_source;
                                        connected.store(true, Ordering::SeqCst);
                                        let mut s = state.lock().await;
                                        s.connected = true;
                                        s.reconnect_count += 1;
                                        s.connected_at = Some(Instant::now());
                                        backoff.reset();
                                        tracing::info!("Reconnected successfully");
                                        break;
                                    }
                                    Err(e) => {
                                        tracing::warn!("Reconnect failed: {e}");
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            tracing::error!("WebSocket error: {e}");
                        }
                        _ => {}
                    }
                }
                _ = ping_interval.tick() => {
                    if connected.load(Ordering::SeqCst) {
                        if let Some(ref mut s) = *sink.lock().await {
                            let _ = s.send(Message::Ping(vec![])).await;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_state_default() {
        let state = TunnelState::default();
        assert!(!state.connected);
        assert!(state.tunnel_id.is_none());
        assert_eq!(state.reconnect_count, 0);
    }

    #[test]
    fn tunnel_client_starts_disconnected() {
        let client = TunnelClient::new();
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn disconnect_fails_when_not_connected() {
        let client = TunnelClient::new();
        let result = client.disconnect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_fails_when_not_connected() {
        let client = TunnelClient::new();
        let result = client.send_message("test").await;
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cd simse-remote && cargo test tunnel`
Expected: 4 tests pass

**Step 3: Commit**

```bash
git add simse-remote/src/tunnel.rs
git commit -m "feat(simse-remote): add WebSocket tunnel client with reconnection"
```

---

## Task 7: Implement router.rs

**Files:**
- Create: `simse-remote/src/router.rs`

**Step 1: Write router.rs**

The router forwards JSON-RPC requests received from the relay to a local simse-core process.

```rust
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::RemoteError;

/// Routes JSON-RPC requests to a local simse-core process.
pub struct LocalRouter {
    child: Option<Child>,
    reader: Option<BufReader<std::process::ChildStdout>>,
    next_id: AtomicU64,
}

impl Default for LocalRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalRouter {
    pub fn new() -> Self {
        Self {
            child: None,
            reader: None,
            next_id: AtomicU64::new(1),
        }
    }

    /// Spawn a local simse-core-engine process.
    pub fn spawn(&mut self, binary_path: &str) -> Result<(), RemoteError> {
        let mut child = Command::new(binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                RemoteError::ConnectionFailed(format!(
                    "Failed to spawn {binary_path}: {e}"
                ))
            })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            RemoteError::ConnectionFailed("No stdout from child process".into())
        })?;
        self.reader = Some(BufReader::new(stdout));
        self.child = Some(child);
        tracing::info!("Local simse-core spawned: {binary_path}");
        Ok(())
    }

    /// Forward a raw JSON-RPC request string to the local process and return the response.
    pub fn forward(&mut self, request: &str) -> Result<String, RemoteError> {
        let child = self
            .child
            .as_mut()
            .ok_or(RemoteError::NotInitialized)?;
        let reader = self
            .reader
            .as_mut()
            .ok_or(RemoteError::NotInitialized)?;

        let stdin = child.stdin.as_mut().ok_or(RemoteError::NotInitialized)?;

        // Write request to child stdin
        stdin
            .write_all(request.as_bytes())
            .map_err(|e| RemoteError::Io(e))?;
        if !request.ends_with('\n') {
            stdin.write_all(b"\n").map_err(|e| RemoteError::Io(e))?;
        }
        stdin.flush().map_err(|e| RemoteError::Io(e))?;

        // Read response line
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).map_err(|e| RemoteError::Io(e))?;
            if n == 0 {
                return Err(RemoteError::ConnectionFailed(
                    "Child process closed stdout".into(),
                ));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip notifications (no id field)
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if parsed.get("id").is_some() {
                    return Ok(trimmed.to_string());
                }
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    /// Stop the local process.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take());
            let _ = child.wait();
        }
        self.reader = None;
        tracing::info!("Local simse-core stopped");
    }
}

impl Drop for LocalRouter {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_router_is_not_running() {
        let router = LocalRouter::new();
        assert!(!router.is_running());
    }

    #[test]
    fn forward_fails_when_not_spawned() {
        let mut router = LocalRouter::new();
        let result = router.forward(r#"{"id":1,"method":"health"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn spawn_fails_with_bad_binary() {
        let mut router = LocalRouter::new();
        let result = router.spawn("/nonexistent/binary");
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cd simse-remote && cargo test router`
Expected: 3 tests pass

**Step 3: Commit**

```bash
git add simse-remote/src/router.rs
git commit -m "feat(simse-remote): add local router for forwarding to simse-core"
```

---

## Task 8: Implement server.rs (JSON-RPC dispatcher)

**Files:**
- Create: `simse-remote/src/server.rs`

**Step 1: Write server.rs**

```rust
use std::io::{self, BufRead};

use crate::auth::AuthClient;
use crate::error::RemoteError;
use crate::protocol::*;
use crate::tunnel::TunnelClient;
use crate::transport::NdjsonTransport;

/// Remote JSON-RPC server — dispatches incoming requests.
pub struct RemoteServer {
    transport: NdjsonTransport,
    auth: AuthClient,
    tunnel: TunnelClient,
}

impl RemoteServer {
    pub fn new(transport: NdjsonTransport) -> Self {
        Self {
            transport,
            auth: AuthClient::new(),
            tunnel: TunnelClient::new(),
        }
    }

    /// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stdin = io::stdin();
        let reader = stdin.lock();

        for line_result in reader.lines() {
            let line = line_result?;
            if line.trim().is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Failed to parse request: {}", e);
                    continue;
                }
            };

            self.dispatch(request).await;
        }

        Ok(())
    }

    // ── Dispatch ──

    async fn dispatch(&mut self, req: JsonRpcRequest) {
        let result = match req.method.as_str() {
            "auth/login" => self.handle_login(req.params).await,
            "auth/logout" => self.handle_logout(req.params),
            "auth/status" => self.handle_auth_status(req.params),
            "tunnel/connect" => self.handle_tunnel_connect(req.params).await,
            "tunnel/disconnect" => self.handle_tunnel_disconnect(req.params).await,
            "tunnel/status" => self.handle_tunnel_status(req.params).await,
            "remote/health" => self.handle_health(req.params).await,
            _ => {
                self.transport.write_error(
                    req.id,
                    METHOD_NOT_FOUND,
                    format!("Unknown method: {}", req.method),
                    None,
                );
                return;
            }
        };

        self.write_result(req.id, result);
    }

    fn write_result(&self, id: u64, result: Result<serde_json::Value, RemoteError>) {
        match result {
            Ok(value) => self.transport.write_response(id, value),
            Err(e) => self.transport.write_error(
                id,
                REMOTE_ERROR,
                e.to_string(),
                Some(e.to_json_rpc_error()),
            ),
        }
    }

    // ── Auth handlers ──

    async fn handle_login(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, RemoteError> {
        let p: LoginParams = parse_params(params)?;

        let api_url = p.api_url.as_deref().unwrap_or("https://api.simse.dev");

        let state = if let Some(api_key) = p.api_key {
            self.auth.login_api_key(api_url, &api_key).await?
        } else {
            let email = p.email.ok_or_else(|| {
                RemoteError::InvalidParams("email or apiKey required".into())
            })?;
            let password = p.password.ok_or_else(|| {
                RemoteError::InvalidParams("password required".into())
            })?;
            self.auth.login_password(api_url, &email, &password).await?
        };

        Ok(serde_json::to_value(LoginResult {
            user_id: state.user_id,
            session_token: state.session_token,
            team_id: state.team_id,
            role: state.role,
        })?)
    }

    fn handle_logout(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, RemoteError> {
        self.auth.logout();
        Ok(serde_json::json!({ "ok": true }))
    }

    fn handle_auth_status(
        &self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, RemoteError> {
        let state = self.auth.state();
        Ok(serde_json::to_value(AuthStatusResult {
            authenticated: state.is_some(),
            user_id: state.map(|s| s.user_id.clone()),
            team_id: state.and_then(|s| s.team_id.clone()),
            role: state.and_then(|s| s.role.clone()),
            api_url: state.map(|s| s.api_url.clone()),
        })?)
    }

    // ── Tunnel handlers ──

    async fn handle_tunnel_connect(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, RemoteError> {
        let auth_state = self.auth.require_auth()?;
        let p: TunnelConnectParams = parse_params(params)?;

        let relay_url = p
            .relay_url
            .as_deref()
            .unwrap_or("wss://relay.simse.dev");

        let tunnel_id = self
            .tunnel
            .connect(relay_url, &auth_state.session_token)
            .await?;

        Ok(serde_json::to_value(TunnelConnectResult {
            tunnel_id,
            relay_url: relay_url.to_string(),
        })?)
    }

    async fn handle_tunnel_disconnect(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, RemoteError> {
        self.tunnel.disconnect().await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn handle_tunnel_status(
        &self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, RemoteError> {
        let state = self.tunnel.get_state().await;
        let uptime_ms = state
            .connected_at
            .map(|t| t.elapsed().as_millis() as u64);

        Ok(serde_json::to_value(TunnelStatusResult {
            connected: state.connected,
            tunnel_id: state.tunnel_id,
            relay_url: state.relay_url,
            uptime_ms,
            reconnect_count: state.reconnect_count,
        })?)
    }

    // ── Health ──

    async fn handle_health(
        &self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, RemoteError> {
        Ok(serde_json::to_value(HealthResult {
            ok: true,
            authenticated: self.auth.is_authenticated(),
            tunnel_connected: self.tunnel.is_connected(),
        })?)
    }
}
```

**Step 2: Build the whole crate**

Run: `cd simse-remote && cargo build 2>&1 | tail -5`
Expected: Build succeeds

**Step 3: Run all tests**

Run: `cd simse-remote && cargo test`
Expected: All unit tests pass (error: 2, auth: 3, heartbeat: 2, tunnel: 4, router: 3 = 14 tests)

**Step 4: Commit**

```bash
git add simse-remote/src/server.rs
git commit -m "feat(simse-remote): add JSON-RPC server dispatcher with all 7 methods"
```

---

## Task 9: Add integration tests for simse-remote

**Files:**
- Create: `simse-remote/tests/integration.rs`

**Step 1: Write integration tests**

```rust
// ---------------------------------------------------------------------------
// Integration tests for simse-remote-engine
//
// Each test spawns the binary, communicates over JSON-RPC 2.0 / NDJSON stdio,
// and verifies responses.
// ---------------------------------------------------------------------------

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

struct RemoteProcess {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    next_id: AtomicU64,
}

#[derive(Debug)]
enum RpcResponse {
    Ok(Value),
    Error(Value),
}

impl RemoteProcess {
    fn spawn() -> Self {
        let bin = env!("CARGO_BIN_EXE_simse-remote-engine");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn simse-remote-engine");

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
            let n = self
                .reader
                .read_line(&mut buf)
                .expect("failed to read from stdout");
            if n == 0 {
                panic!("unexpected EOF while waiting for response to id={}", id);
            }
            let buf = buf.trim();
            if buf.is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(buf)
                .unwrap_or_else(|e| panic!("invalid JSON: {e}\nline: {buf}"));

            if parsed.get("id").is_none() {
                continue;
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
}

impl Drop for RemoteProcess {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn health_returns_ok() {
    let mut proc = RemoteProcess::spawn();
    let result = proc.call("remote/health", json!({}));
    assert_eq!(result["ok"], true);
    assert_eq!(result["authenticated"], false);
    assert_eq!(result["tunnelConnected"], false);
}

#[test]
fn auth_status_when_not_logged_in() {
    let mut proc = RemoteProcess::spawn();
    let result = proc.call("auth/status", json!({}));
    assert_eq!(result["authenticated"], false);
    assert!(result["userId"].is_null());
}

#[test]
fn logout_succeeds_when_not_logged_in() {
    let mut proc = RemoteProcess::spawn();
    let result = proc.call("auth/logout", json!({}));
    assert_eq!(result["ok"], true);
}

#[test]
fn tunnel_connect_fails_without_auth() {
    let mut proc = RemoteProcess::spawn();
    let err = proc.call_err("tunnel/connect", json!({}));
    assert_eq!(err["data"]["remoteCode"], "REMOTE_NOT_AUTHENTICATED");
}

#[test]
fn tunnel_disconnect_fails_when_not_connected() {
    let mut proc = RemoteProcess::spawn();
    let err = proc.call_err("tunnel/disconnect", json!({}));
    assert_eq!(err["data"]["remoteCode"], "REMOTE_TUNNEL_NOT_CONNECTED");
}

#[test]
fn tunnel_status_when_not_connected() {
    let mut proc = RemoteProcess::spawn();
    let result = proc.call("tunnel/status", json!({}));
    assert_eq!(result["connected"], false);
    assert!(result["tunnelId"].is_null());
    assert_eq!(result["reconnectCount"], 0);
}

#[test]
fn unknown_method_returns_error() {
    let mut proc = RemoteProcess::spawn();
    let err = proc.call_err("nonexistent/method", json!({}));
    assert_eq!(err["code"], -32601);
}

#[test]
fn login_with_missing_params_returns_error() {
    let mut proc = RemoteProcess::spawn();
    // No email, no apiKey — should fail
    let err = proc.call_err("auth/login", json!({}));
    assert!(err["data"]["remoteCode"].as_str().unwrap().starts_with("REMOTE_"));
}
```

**Step 2: Run integration tests**

Run: `cd simse-remote && cargo test --test integration`
Expected: 8 tests pass

**Step 3: Commit**

```bash
git add simse-remote/tests/integration.rs
git commit -m "test(simse-remote): add integration tests for JSON-RPC server"
```

---

## Task 10: Scaffold simse-relay Cloudflare Worker

**Files:**
- Create: `simse-relay/package.json`
- Create: `simse-relay/tsconfig.json`
- Create: `simse-relay/wrangler.toml`
- Create: `simse-relay/src/types.ts`
- Create: `simse-relay/biome.json`

**Step 1: Create package.json**

```json
{
	"name": "simse-relay",
	"private": true,
	"type": "module",
	"scripts": {
		"dev": "wrangler dev",
		"build": "wrangler deploy --dry-run --outdir dist",
		"deploy": "wrangler deploy",
		"lint": "biome check .",
		"lint:fix": "biome check --write .",
		"typecheck": "tsc --noEmit",
		"test": "vitest run"
	},
	"dependencies": {
		"hono": "^4.0.0"
	},
	"devDependencies": {
		"@biomejs/biome": "^2.3.12",
		"@cloudflare/vitest-pool-workers": "^0.8.0",
		"@cloudflare/workers-types": "^4.20260305.0",
		"typescript": "^5.7.0",
		"vitest": "^3.0.0",
		"wrangler": "^4.0.0"
	}
}
```

**Step 2: Create tsconfig.json**

```json
{
	"compilerOptions": {
		"target": "ESNext",
		"module": "ESNext",
		"moduleResolution": "bundler",
		"strict": true,
		"noEmit": true,
		"skipLibCheck": true,
		"esModuleInterop": true,
		"forceConsistentCasingInFileNames": true,
		"resolveJsonModule": true,
		"isolatedModules": true,
		"types": ["@cloudflare/workers-types"]
	},
	"include": ["src/**/*.ts", "vitest.config.ts"],
	"exclude": ["src/**/*.test.ts", "src/test-setup.ts"]
}
```

**Step 3: Create wrangler.toml**

```toml
name = "simse-relay"
compatibility_date = "2025-04-01"
main = "src/index.ts"

workers_dev = true

routes = [{ pattern = "relay.simse.dev", custom_domain = true }]

[durable_objects]
bindings = [{ name = "TUNNEL_SESSION", class_name = "TunnelSession" }]

[[migrations]]
tag = "v1"
new_classes = ["TunnelSession"]

[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"
```

**Step 4: Create types.ts**

```typescript
export interface Env {
	TUNNEL_SESSION: DurableObjectNamespace;
	SECRETS: SecretsStoreNamespace;
	ANALYTICS: AnalyticsEngineDataset;
}

export interface ApiSecrets {
	authApiUrl: string;
}

export interface ValidateResponse {
	data: {
		userId: string;
		sessionId?: string;
		teamId: string | null;
		role: string | null;
	};
}
```

**Step 5: Create biome.json**

```json
{
	"$schema": "https://biomejs.dev/schemas/2.0.0/schema.json",
	"formatter": {
		"indentStyle": "tab"
	},
	"javascript": {
		"formatter": {
			"quoteStyle": "single",
			"semicolons": "always"
		}
	}
}
```

**Step 6: Commit**

```bash
git add simse-relay/
git commit -m "feat(simse-relay): scaffold Cloudflare Worker with wrangler, types, biome"
```

---

## Task 11: Implement TunnelSession Durable Object

**Files:**
- Create: `simse-relay/src/tunnel.ts`

**Step 1: Write tunnel.ts**

```typescript
import type { Env } from './types';

interface SessionState {
	userId: string;
	tunnelWs: WebSocket | null;
	clientWs: WebSocket | null;
	connectedAt: number;
}

export class TunnelSession implements DurableObject {
	private state: DurableObjectState;
	private env: Env;
	private session: SessionState | null = null;

	constructor(state: DurableObjectState, env: Env) {
		this.state = state;
		this.env = env;
	}

	async fetch(request: Request): Promise<Response> {
		const url = new URL(request.url);
		const path = url.pathname;

		if (path === '/ws/tunnel') {
			return this.handleTunnelWebSocket(request, url);
		}

		if (path === '/ws/client') {
			return this.handleClientWebSocket(request, url);
		}

		if (path === '/status') {
			return Response.json({
				hasSession: this.session !== null,
				hasTunnel: this.session?.tunnelWs !== null,
				hasClient: this.session?.clientWs !== null,
			});
		}

		return new Response('not found', { status: 404 });
	}

	private handleTunnelWebSocket(request: Request, url: URL): Response {
		const userId = url.searchParams.get('userId');
		if (!userId) {
			return new Response('missing userId', { status: 400 });
		}

		const pair = new WebSocketPair();
		const [client, server] = Object.values(pair);

		this.state.acceptWebSocket(server, ['tunnel']);

		this.session = {
			userId,
			tunnelWs: server,
			clientWs: this.session?.clientWs ?? null,
			connectedAt: Date.now(),
		};

		return new Response(null, { status: 101, webSocket: client });
	}

	private handleClientWebSocket(request: Request, url: URL): Response {
		if (!this.session?.tunnelWs) {
			return new Response('no tunnel connected', { status: 503 });
		}

		const pair = new WebSocketPair();
		const [client, server] = Object.values(pair);

		this.state.acceptWebSocket(server, ['client']);
		this.session.clientWs = server;

		return new Response(null, { status: 101, webSocket: client });
	}

	async webSocketMessage(
		ws: WebSocket,
		message: string | ArrayBuffer,
	): Promise<void> {
		if (!this.session) return;

		const tags = this.state.getTags(ws);
		const msgStr =
			typeof message === 'string'
				? message
				: new TextDecoder().decode(message);

		if (tags.includes('tunnel')) {
			// Message from local simse → forward to client
			if (this.session.clientWs) {
				try {
					this.session.clientWs.send(msgStr);
				} catch {
					this.session.clientWs = null;
				}
			}
		} else if (tags.includes('client')) {
			// Message from web client → forward to tunnel
			if (this.session.tunnelWs) {
				try {
					this.session.tunnelWs.send(msgStr);
				} catch {
					this.session.tunnelWs = null;
				}
			}
		}
	}

	async webSocketClose(
		ws: WebSocket,
		code: number,
		reason: string,
		wasClean: boolean,
	): Promise<void> {
		if (!this.session) return;

		const tags = this.state.getTags(ws);

		if (tags.includes('tunnel')) {
			this.session.tunnelWs = null;
			// Also close client if tunnel drops
			if (this.session.clientWs) {
				try {
					this.session.clientWs.close(1001, 'tunnel disconnected');
				} catch {
					// ignore
				}
				this.session.clientWs = null;
			}
			this.session = null;
		} else if (tags.includes('client')) {
			this.session.clientWs = null;
		}
	}

	async webSocketError(ws: WebSocket, error: unknown): Promise<void> {
		await this.webSocketClose(ws, 1011, 'error', false);
	}
}
```

**Step 2: Commit**

```bash
git add simse-relay/src/tunnel.ts
git commit -m "feat(simse-relay): add TunnelSession Durable Object for WebSocket pairing"
```

---

## Task 12: Implement simse-relay routes and index

**Files:**
- Create: `simse-relay/src/routes/ws.ts`
- Create: `simse-relay/src/routes/tunnels.ts`
- Create: `simse-relay/src/index.ts`

**Step 1: Write ws.ts**

```typescript
import { Hono } from 'hono';
import type { ApiSecrets, Env, ValidateResponse } from '../types';

const ws = new Hono<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>();

ws.get('/ws/tunnel', async (c) => {
	const token = c.req.query('token');
	if (!token) {
		return c.json({ error: { code: 'MISSING_TOKEN', message: 'token query param required' } }, 401);
	}

	const auth = await validateToken(c.var.secrets.authApiUrl, token);
	if (!auth) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	// Route to Durable Object keyed by userId
	const id = c.env.TUNNEL_SESSION.idFromName(auth.userId);
	const stub = c.env.TUNNEL_SESSION.get(id);

	const url = new URL(c.req.url);
	url.searchParams.set('userId', auth.userId);
	const doRequest = new Request(url.toString(), {
		headers: c.req.raw.headers,
	});

	return stub.fetch(doRequest);
});

ws.get('/ws/client', async (c) => {
	const token = c.req.query('token');
	if (!token) {
		return c.json({ error: { code: 'MISSING_TOKEN', message: 'token query param required' } }, 401);
	}

	const auth = await validateToken(c.var.secrets.authApiUrl, token);
	if (!auth) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	// Route to same Durable Object as the user's tunnel
	const id = c.env.TUNNEL_SESSION.idFromName(auth.userId);
	const stub = c.env.TUNNEL_SESSION.get(id);

	const url = new URL(c.req.url);
	url.searchParams.set('userId', auth.userId);
	const doRequest = new Request(url.toString(), {
		headers: c.req.raw.headers,
	});

	return stub.fetch(doRequest);
});

async function validateToken(
	authApiUrl: string,
	token: string,
): Promise<ValidateResponse['data'] | null> {
	const res = await fetch(`${authApiUrl}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) return null;

	const json = (await res.json()) as ValidateResponse;
	return json.data;
}

export default ws;
```

**Step 2: Write tunnels.ts**

```typescript
import { Hono } from 'hono';
import type { ApiSecrets, Env, ValidateResponse } from '../types';

const tunnels = new Hono<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>();

tunnels.get('/tunnels', async (c) => {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Bearer token required' } }, 401);
	}

	const token = authHeader.slice(7);
	const res = await fetch(`${c.var.secrets.authApiUrl}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	const auth = (await res.json()) as ValidateResponse;
	const userId = auth.data.userId;

	// Check if user has an active tunnel
	const id = c.env.TUNNEL_SESSION.idFromName(userId);
	const stub = c.env.TUNNEL_SESSION.get(id);
	const statusRes = await stub.fetch(new Request('https://internal/status'));
	const status = await statusRes.json() as { hasSession: boolean; hasTunnel: boolean; hasClient: boolean };

	return c.json({
		data: {
			tunnels: status.hasTunnel
				? [{ userId, hasTunnel: true, hasClient: status.hasClient }]
				: [],
		},
	});
});

export default tunnels;
```

**Step 3: Write index.ts**

```typescript
import { Hono } from 'hono';
import { createMiddleware } from 'hono/factory';
import tunnels from './routes/tunnels';
import ws from './routes/ws';
import type { ApiSecrets, Env } from './types';

export { TunnelSession } from './tunnel';

const app = new Hono<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>();

// Analytics middleware
app.use('*', async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
	const cf = (c.req.raw as any).cf;
	const url = new URL(c.req.url);

	c.env.ANALYTICS?.writeDataPoint({
		indexes: ['simse-relay'],
		blobs: [
			c.req.method,
			url.pathname,
			String(c.res.status),
			'simse-relay',
			'',
			'',
			cf?.country ?? '',
			cf?.city ?? '',
			cf?.continent ?? '',
			(c.req.header('User-Agent') ?? '').slice(0, 256),
			c.req.header('Referer') ?? '',
			c.res.headers.get('Content-Type') ?? '',
			c.req.header('Cf-Ray') ?? '',
		],
		doubles: [
			latencyMs,
			c.res.status,
			Number(c.req.header('Content-Length') ?? 0),
			Number(c.res.headers.get('Content-Length') ?? 0),
		],
	});
});

// Health check (before secrets middleware)
app.get('/health', (c) => c.json({ ok: true }));

// Secrets middleware
const secretsMiddleware = createMiddleware<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>(async (c, next) => {
	const authApiUrl = await c.env.SECRETS.get('AUTH_API_URL');

	if (!authApiUrl) {
		return c.json(
			{ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } },
			500,
		);
	}

	c.set('secrets', { authApiUrl });
	await next();
});

app.use('*', secretsMiddleware);

// Routes
app.route('', ws);
app.route('', tunnels);

export default app;
```

**Step 4: Install dependencies**

Run: `cd simse-relay && npm install`
Expected: node_modules created

**Step 5: Type-check**

Run: `cd simse-relay && npx tsc --noEmit`
Expected: No errors (or only Durable Object API type errors if types aren't fully aligned — acceptable at this stage)

**Step 6: Commit**

```bash
git add simse-relay/src/
git commit -m "feat(simse-relay): add Hono routes, auth middleware, analytics"
```

---

## Task 13: Add simse-relay tests

**Files:**
- Create: `simse-relay/vitest.config.ts`
- Create: `simse-relay/src/test-setup.ts`
- Create: `simse-relay/src/index.test.ts`

**Step 1: Create vitest.config.ts**

```typescript
import { defineWorkersConfig } from '@cloudflare/vitest-pool-workers/config';

export default defineWorkersConfig({
	test: {
		setupFiles: ['./src/test-setup.ts'],
		poolOptions: {
			workers: {
				wrangler: { configPath: './wrangler.toml' },
			},
		},
	},
});
```

**Step 2: Create test-setup.ts**

```typescript
// No external state to seed for relay tests.
// Durable Objects are created on-demand.
```

**Step 3: Create index.test.ts**

```typescript
import { SELF } from 'cloudflare:test';
import { describe, expect, it } from 'vitest';

describe('GET /health', () => {
	it('returns 200', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/health');
		expect(res.status).toBe(200);
		const body = await res.json();
		expect(body).toEqual({ ok: true });
	});
});

describe('GET /ws/tunnel', () => {
	it('returns 401 without token', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/ws/tunnel');
		expect(res.status).toBe(401);
		const body = await res.json();
		expect(body.error.code).toBe('MISSING_TOKEN');
	});
});

describe('GET /ws/client', () => {
	it('returns 401 without token', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/ws/client');
		expect(res.status).toBe(401);
		const body = await res.json();
		expect(body.error.code).toBe('MISSING_TOKEN');
	});
});

describe('GET /tunnels', () => {
	it('returns 401 without auth header', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/tunnels');
		expect(res.status).toBe(401);
		const body = await res.json();
		expect(body.error.code).toBe('UNAUTHORIZED');
	});
});

describe('unknown routes', () => {
	it('returns 404', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/unknown');
		expect(res.status).toBe(404);
	});
});
```

**Step 4: Run tests**

Run: `cd simse-relay && npm run test`
Expected: 5 tests pass

**Step 5: Commit**

```bash
git add simse-relay/vitest.config.ts simse-relay/src/test-setup.ts simse-relay/src/index.test.ts
git commit -m "test(simse-relay): add integration tests for health, auth, and error paths"
```

---

## Task 14: Update workspace config and CLAUDE.md

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `CLAUDE.md`

**Step 1: Add simse-remote to workspace exclude list**

In the root `Cargo.toml`, add `"simse-remote"` to the `exclude` list (alongside the other standalone crates like simse-vnet, simse-sandbox, etc.).

**Step 2: Update CLAUDE.md**

Add to the Commands section:
```bash
bun run build:remote-engine  # cd simse-remote && cargo build --release
```

Add to the Rust tests section:
```bash
cd simse-remote && cargo test  # Rust remote engine tests
```

Add to the TypeScript lint section:
```bash
cd simse-relay && npm run lint   # Relay worker lint
```

Add to the TypeScript tests section:
```bash
cd simse-relay && npm run test   # Relay worker tests (Vitest + @cloudflare/vitest-pool-workers)
```

Add simse-remote and simse-relay to the Repository Layout tree.

Add simse-remote to the "Other Rust Crates" section:
```
simse-remote/              # Pure Rust crate — remote access engine (JSON-RPC over stdio)
  src/
    error.rs               # RemoteError enum with REMOTE_ code prefixes
    protocol.rs            # JSON-RPC request/response types (7 methods)
    transport.rs           # NdjsonTransport for JSON-RPC over stdio
    auth.rs                # Auth client (login/logout, token validation via simse-api)
    tunnel.rs              # WebSocket tunnel client (connect, reconnect, multiplex)
    router.rs              # Local router (forward relayed requests to simse-core)
    heartbeat.rs           # Backoff config, keepalive ping interval
    server.rs              # RemoteServer: 7-method JSON-RPC dispatch
```

Add simse-relay to the TypeScript Services section:
```
simse-relay/               # TypeScript — Relay worker (WebSocket tunnel, Cloudflare Worker + Durable Objects)
  src/
    index.ts               # Hono app entry, health + analytics + secrets middleware
    types.ts               # Env interface (DurableObjectNamespace, SecretsStore, AnalyticsEngine)
    tunnel.ts              # Durable Object: TunnelSession (WebSocket pair management)
    routes/
      ws.ts                # WebSocket upgrade handlers (/ws/tunnel, /ws/client)
      tunnels.ts           # REST endpoint for listing active tunnels
```

Add simse-remote JSON-RPC Methods table:

| Domain | Methods |
|--------|---------|
| `auth/` | `login`, `logout`, `status` |
| `tunnel/` | `connect`, `disconnect`, `status` |
| `remote/` | `health` |

**Step 3: Commit**

```bash
git add Cargo.toml CLAUDE.md
git commit -m "docs: add simse-remote and simse-relay to workspace config and CLAUDE.md"
```

---

## Task 15: Final verification

**Step 1: Build simse-remote**

Run: `cd simse-remote && cargo build --release`
Expected: Build succeeds

**Step 2: Run all simse-remote tests**

Run: `cd simse-remote && cargo test`
Expected: All tests pass (14 unit + 8 integration = 22 tests)

**Step 3: Install and test simse-relay**

Run: `cd simse-relay && npm install && npm run test`
Expected: 5 tests pass

**Step 4: Lint simse-relay**

Run: `cd simse-relay && npm run lint`
Expected: No lint errors

**Step 5: Final commit (if any fixups needed)**

```bash
git add -A && git commit -m "fix: address any build/lint issues from final verification"
```
