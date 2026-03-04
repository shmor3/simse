use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::VnetError;
use crate::protocol::HttpResponseResult;

// ── NetBackend trait ────────────────────────────────────────────────────────

/// Async network backend trait.
///
/// Mirrors the network operations so they can be routed through either a
/// local (real HTTP via reqwest) or remote (e.g. SSH-tunneled) backend.
/// Mock handling lives above this layer — the backend only sees real
/// network requests that passed mock resolution.
#[async_trait]
pub trait NetBackend: Send + Sync {
    // ── HTTP ────────────────────────────────────────────────────────────

    /// Send an HTTP request and return the response.
    async fn http_request(
        &self,
        url: &str,
        method: &str,
        headers: &HashMap<String, String>,
        body: Option<&str>,
        timeout_ms: u64,
        max_response_bytes: u64,
    ) -> Result<HttpResponseResult, VnetError>;

    // ── WebSocket ───────────────────────────────────────────────────────

    /// Open a WebSocket connection, returning a session ID.
    async fn ws_connect(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
    ) -> Result<String, VnetError>;

    /// Send data over an open WebSocket session.
    async fn ws_send(&self, session_id: &str, data: &str) -> Result<(), VnetError>;

    /// Close a WebSocket session.
    async fn ws_close(&self, session_id: &str) -> Result<(), VnetError>;

    // ── TCP ─────────────────────────────────────────────────────────────

    /// Open a TCP connection, returning a session ID.
    async fn tcp_connect(&self, host: &str, port: u16) -> Result<String, VnetError>;

    /// Send data over an open TCP session.
    async fn tcp_send(&self, session_id: &str, data: &str) -> Result<(), VnetError>;

    /// Close a TCP session.
    async fn tcp_close(&self, session_id: &str) -> Result<(), VnetError>;

    // ── UDP ─────────────────────────────────────────────────────────────

    /// Send a UDP datagram. Returns the response data if one is received
    /// within the timeout, or `None` for fire-and-forget.
    async fn udp_send(
        &self,
        host: &str,
        port: u16,
        data: &str,
        timeout_ms: u64,
    ) -> Result<Option<String>, VnetError>;

    // ── DNS ─────────────────────────────────────────────────────────────

    /// Resolve a hostname to a list of IP addresses.
    async fn resolve(&self, hostname: &str) -> Result<Vec<String>, VnetError>;
}
