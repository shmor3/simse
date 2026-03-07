use std::collections::HashMap;
use std::time::Instant;

use reqwest::Client;
use tokio::net::lookup_host;

use crate::sandbox::error::SandboxError;
use crate::sandbox::vnet_types::HttpResponseResult;

// ── LocalNet ──────────────────────────────────────────────────────────────

/// Local network backend using `reqwest` for HTTP and `tokio` for DNS.
///
/// WS, TCP, and UDP operations are not yet implemented — the existing
/// mock:// scheme covers those use cases for now.
pub struct LocalNet {
    client: Client,
}

impl Default for LocalNet {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalNet {
    /// Create a new `LocalNet` with a default `reqwest::Client`.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Create a `LocalNet` with a custom `reqwest::Client`.
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    pub async fn http_request(
        &self,
        url: &str,
        method: &str,
        headers: &HashMap<String, String>,
        body: Option<&str>,
        timeout_ms: u64,
        max_response_bytes: u64,
    ) -> Result<HttpResponseResult, SandboxError> {
        let start = Instant::now();

        // Parse method
        let reqwest_method: reqwest::Method = method
            .parse()
            .map_err(|_| SandboxError::InvalidParams(format!("invalid HTTP method: {method}")))?;

        // Build request
        let mut builder = self
            .client
            .request(reqwest_method, url)
            .timeout(std::time::Duration::from_millis(timeout_ms));

        for (key, value) in headers {
            builder = builder.header(key.as_str(), value.as_str());
        }

        if let Some(body_str) = body {
            builder = builder.body(body_str.to_owned());
        }

        // Send
        let response = builder.send().await.map_err(|e| {
            if e.is_timeout() {
                SandboxError::VnetTimeout(format!("HTTP request timed out after {timeout_ms}ms"))
            } else {
                SandboxError::VnetConnectionFailed(format!("HTTP request failed: {e}"))
            }
        })?;

        // Read response
        let status = response.status().as_u16();

        let resp_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let content_type = resp_headers
            .get("content-type")
            .cloned()
            .unwrap_or_default();

        let body_type = if content_type.contains("json") {
            "json".to_string()
        } else {
            "text".to_string()
        };

        let body_bytes = response.bytes().await.map_err(|e| {
            SandboxError::VnetConnectionFailed(format!("failed to read response body: {e}"))
        })?;

        let bytes_received = body_bytes.len() as u64;

        // Enforce max_response_bytes
        if bytes_received > max_response_bytes {
            return Err(SandboxError::VnetResponseTooLarge(format!(
                "response size {bytes_received} exceeds limit {max_response_bytes}"
            )));
        }

        let body_str = String::from_utf8_lossy(&body_bytes).into_owned();
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(HttpResponseResult {
            status,
            headers: resp_headers,
            body: body_str,
            body_type,
            duration_ms,
            bytes_received,
        })
    }

    pub async fn ws_connect(
        &self,
        _url: &str,
        _headers: &HashMap<String, String>,
    ) -> Result<String, SandboxError> {
        Err(SandboxError::VnetConnectionFailed(
            "WebSocket via LocalNet not yet implemented".to_string(),
        ))
    }

    pub async fn ws_send(&self, _session_id: &str, _data: &str) -> Result<(), SandboxError> {
        Err(SandboxError::VnetConnectionFailed(
            "WebSocket via LocalNet not yet implemented".to_string(),
        ))
    }

    pub async fn ws_close(&self, _session_id: &str) -> Result<(), SandboxError> {
        Err(SandboxError::VnetConnectionFailed(
            "WebSocket via LocalNet not yet implemented".to_string(),
        ))
    }

    pub async fn tcp_connect(&self, _host: &str, _port: u16) -> Result<String, SandboxError> {
        Err(SandboxError::VnetConnectionFailed(
            "TCP via LocalNet not yet implemented".to_string(),
        ))
    }

    pub async fn tcp_send(&self, _session_id: &str, _data: &str) -> Result<(), SandboxError> {
        Err(SandboxError::VnetConnectionFailed(
            "TCP via LocalNet not yet implemented".to_string(),
        ))
    }

    pub async fn tcp_close(&self, _session_id: &str) -> Result<(), SandboxError> {
        Err(SandboxError::VnetConnectionFailed(
            "TCP via LocalNet not yet implemented".to_string(),
        ))
    }

    pub async fn udp_send(
        &self,
        _host: &str,
        _port: u16,
        _data: &str,
        _timeout_ms: u64,
    ) -> Result<Option<String>, SandboxError> {
        Err(SandboxError::VnetConnectionFailed(
            "UDP via LocalNet not yet implemented".to_string(),
        ))
    }

    pub async fn resolve(&self, hostname: &str) -> Result<Vec<String>, SandboxError> {
        // Use port 0 as a dummy — lookup_host requires host:port format
        let addr_str = format!("{hostname}:0");
        let addrs = lookup_host(&addr_str)
            .await
            .map_err(|e| SandboxError::VnetDnsResolutionFailed(format!("{hostname}: {e}")))?;

        let ips: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();

        if ips.is_empty() {
            return Err(SandboxError::VnetDnsResolutionFailed(format!(
                "no addresses found for {hostname}"
            )));
        }

        Ok(ips)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_creates_backend() {
        let backend = LocalNet::default();
        // Just verify it constructs without panic
        drop(backend);
    }

    #[test]
    fn with_client_creates_backend() {
        let client = Client::builder()
            .user_agent("simse-vnet-test")
            .build()
            .unwrap();
        let backend = LocalNet::with_client(client);
        drop(backend);
    }

    #[tokio::test]
    async fn ws_connect_returns_not_implemented() {
        let backend = LocalNet::new();
        let err = backend
            .ws_connect("ws://example.com", &HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, SandboxError::VnetConnectionFailed(_)));
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[tokio::test]
    async fn ws_send_returns_not_implemented() {
        let backend = LocalNet::new();
        let err = backend.ws_send("sess-1", "hello").await.unwrap_err();
        assert!(matches!(err, SandboxError::VnetConnectionFailed(_)));
    }

    #[tokio::test]
    async fn ws_close_returns_not_implemented() {
        let backend = LocalNet::new();
        let err = backend.ws_close("sess-1").await.unwrap_err();
        assert!(matches!(err, SandboxError::VnetConnectionFailed(_)));
    }

    #[tokio::test]
    async fn tcp_connect_returns_not_implemented() {
        let backend = LocalNet::new();
        let err = backend.tcp_connect("example.com", 80).await.unwrap_err();
        assert!(matches!(err, SandboxError::VnetConnectionFailed(_)));
    }

    #[tokio::test]
    async fn tcp_send_returns_not_implemented() {
        let backend = LocalNet::new();
        let err = backend.tcp_send("sess-1", "data").await.unwrap_err();
        assert!(matches!(err, SandboxError::VnetConnectionFailed(_)));
    }

    #[tokio::test]
    async fn tcp_close_returns_not_implemented() {
        let backend = LocalNet::new();
        let err = backend.tcp_close("sess-1").await.unwrap_err();
        assert!(matches!(err, SandboxError::VnetConnectionFailed(_)));
    }

    #[tokio::test]
    async fn udp_send_returns_not_implemented() {
        let backend = LocalNet::new();
        let err = backend
            .udp_send("example.com", 53, "query", 5000)
            .await
            .unwrap_err();
        assert!(matches!(err, SandboxError::VnetConnectionFailed(_)));
    }

    #[tokio::test]
    async fn resolve_localhost() {
        let backend = LocalNet::new();
        let addrs = backend.resolve("localhost").await.unwrap();
        assert!(!addrs.is_empty());
        // localhost should resolve to 127.0.0.1 and/or ::1
        assert!(
            addrs.iter().any(|a| a == "127.0.0.1" || a == "::1"),
            "expected localhost to resolve to loopback, got: {addrs:?}"
        );
    }

    #[tokio::test]
    async fn resolve_invalid_host() {
        let backend = LocalNet::new();
        let err = backend
            .resolve("this-host-definitely-does-not-exist.invalid")
            .await
            .unwrap_err();
        assert!(matches!(err, SandboxError::VnetDnsResolutionFailed(_)));
    }

    #[tokio::test]
    async fn http_request_invalid_method() {
        let backend = LocalNet::new();
        // Method tokens cannot contain spaces per HTTP spec; reqwest rejects them
        let err = backend
            .http_request(
                "http://example.com",
                "BAD METHOD",
                &HashMap::new(),
                None,
                5000,
                10 * 1024 * 1024,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, SandboxError::InvalidParams(_)));
        assert!(err.to_string().contains("invalid HTTP method"));
    }

    #[tokio::test]
    async fn http_request_connection_refused() {
        let backend = LocalNet::new();
        // Connect to a port that almost certainly isn't listening
        let err = backend
            .http_request(
                "http://127.0.0.1:1",
                "GET",
                &HashMap::new(),
                None,
                2000,
                10 * 1024 * 1024,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, SandboxError::VnetConnectionFailed(_) | SandboxError::VnetTimeout(_)),
            "expected VnetConnectionFailed or VnetTimeout, got: {err:?}"
        );
    }
}
