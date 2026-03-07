// ---------------------------------------------------------------------------
// MCP HTTP Transport — connects to an MCP server over HTTP
// ---------------------------------------------------------------------------
//
// Responsibilities:
//   - Sending JSON-RPC requests to an MCP server via HTTP POST
//   - URL validation (only http:// and https:// schemes allowed)
//   - MCP initialize/initialized handshake on connect
//   - Basic connection lifecycle
//
// Limitations:
//   - HTTP is request-response only — server-initiated notifications are not
//     supported. on_notification() returns a no-op handle.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;

use crate::engine::mcp::error::McpError;
use crate::engine::mcp::protocol::{
	ClientCapabilities, ImplementationInfo, McpInitializeParams, McpInitializeResult,
	RootCapabilities,
};
use crate::engine::mcp::stdio_transport::{
	NotificationHandler, SubscriptionHandle, Transport, MCP_PROTOCOL_VERSION,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for connecting to an MCP server over HTTP.
#[derive(Debug, Clone)]
pub struct HttpTransportConfig {
	/// The URL of the MCP server (must be http:// or https://).
	pub url: String,
	/// Default timeout for HTTP requests (milliseconds). Default: 60 000.
	pub timeout_ms: u64,
	/// Additional HTTP headers to include in every request.
	pub headers: HashMap<String, String>,
	/// Client name sent during initialization. Default: `"simse"`.
	pub client_name: String,
	/// Client version sent during initialization. Default: `"1.0.0"`.
	pub client_version: String,
}

impl Default for HttpTransportConfig {
	fn default() -> Self {
		Self {
			url: String::new(),
			timeout_ms: 60_000,
			headers: HashMap::new(),
			client_name: "simse".into(),
			client_version: "1.0.0".into(),
		}
	}
}

// ---------------------------------------------------------------------------
// HttpTransport
// ---------------------------------------------------------------------------

/// Connects to an MCP server over HTTP using JSON-RPC 2.0.
///
/// Each `request()` call sends an HTTP POST with a JSON-RPC request body and
/// parses the JSON-RPC response from the response body.
///
/// Server-initiated notifications are **not supported** — the
/// `on_notification()` method returns a no-op handle and logs a warning.
pub struct HttpTransport {
	/// Configuration for the HTTP connection.
	config: HttpTransportConfig,
	/// The reqwest HTTP client (created on connect).
	client: Option<reqwest::Client>,
	/// Whether the transport is currently connected.
	connected: AtomicBool,
	/// Monotonically increasing request ID counter.
	next_id: AtomicU64,
	/// Default timeout for requests (ms).
	default_timeout_ms: u64,
}

impl HttpTransport {
	/// Create a new `HttpTransport` with the given configuration.
	///
	/// The transport is not connected until [`connect`](Transport::connect)
	/// is called.
	pub fn new(config: HttpTransportConfig) -> Self {
		let default_timeout_ms = config.timeout_ms;
		Self {
			config,
			client: None,
			connected: AtomicBool::new(false),
			next_id: AtomicU64::new(1),
			default_timeout_ms,
		}
	}

	// -------------------------------------------------------------------
	// Internal: validate URL
	// -------------------------------------------------------------------

	fn validate_url(url: &str) -> Result<(), McpError> {
		if url.is_empty() {
			return Err(McpError::TransportConfigError(
				"URL must not be empty".into(),
			));
		}

		if !url.starts_with("http://") && !url.starts_with("https://") {
			return Err(McpError::TransportConfigError(format!(
				"URL must use http:// or https:// scheme, got: {}",
				url
			)));
		}

		Ok(())
	}

	// -------------------------------------------------------------------
	// Internal: build the reqwest Client
	// -------------------------------------------------------------------

	fn build_client(&self) -> Result<reqwest::Client, McpError> {
		let mut builder = reqwest::Client::builder()
			.timeout(std::time::Duration::from_millis(self.default_timeout_ms));

		// Build default headers from config.
		let mut header_map = reqwest::header::HeaderMap::new();
		header_map.insert(
			reqwest::header::CONTENT_TYPE,
			reqwest::header::HeaderValue::from_static("application/json"),
		);
		for (key, value) in &self.config.headers {
			if let (Ok(name), Ok(val)) = (
				reqwest::header::HeaderName::from_bytes(key.as_bytes()),
				reqwest::header::HeaderValue::from_str(value),
			) {
				header_map.insert(name, val);
			}
		}
		builder = builder.default_headers(header_map);

		builder.build().map_err(|e| {
			McpError::ConnectionFailed(format!("Failed to build HTTP client: {}", e))
		})
	}

	// -------------------------------------------------------------------
	// Internal: send a JSON-RPC request over HTTP
	// -------------------------------------------------------------------

	async fn send_request(
		&self,
		method: &str,
		params: serde_json::Value,
	) -> Result<serde_json::Value, McpError> {
		let client = self
			.client
			.as_ref()
			.ok_or_else(|| McpError::ConnectionFailed("HTTP client not initialized".into()))?;

		let id = self.next_id.fetch_add(1, Ordering::SeqCst);

		let request_body = serde_json::json!({
			"jsonrpc": "2.0",
			"id": id,
			"method": method,
			"params": params,
		});

		let response = client
			.post(&self.config.url)
			.json(&request_body)
			.send()
			.await
			.map_err(|e| {
				if e.is_timeout() {
					McpError::Timeout {
						method: method.to_string(),
						timeout_ms: self.default_timeout_ms,
					}
				} else {
					McpError::ConnectionFailed(format!("HTTP request failed: {}", e))
				}
			})?;

		if !response.status().is_success() {
			return Err(McpError::ProtocolError(format!(
				"HTTP {} for method '{}'",
				response.status(),
				method
			)));
		}

		let body: serde_json::Value = response.json().await.map_err(|e| {
			McpError::ProtocolError(format!("Failed to parse response body: {}", e))
		})?;

		// Check for JSON-RPC error.
		if let Some(error) = body.get("error") {
			let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
			let message = error
				.get("message")
				.and_then(|m| m.as_str())
				.unwrap_or("Unknown error");
			return Err(McpError::ProtocolError(format!(
				"MCP error {}: {}",
				code, message
			)));
		}

		Ok(body.get("result").cloned().unwrap_or(serde_json::Value::Null))
	}

	// -------------------------------------------------------------------
	// Internal: send the MCP initialize handshake
	// -------------------------------------------------------------------

	async fn perform_handshake(&self) -> Result<McpInitializeResult, McpError> {
		let params = McpInitializeParams {
			protocol_version: MCP_PROTOCOL_VERSION.into(),
			capabilities: ClientCapabilities {
				roots: Some(RootCapabilities {
					list_changed: Some(true),
				}),
				sampling: None,
			},
			client_info: ImplementationInfo {
				name: self.config.client_name.clone(),
				version: self.config.client_version.clone(),
			},
		};

		// Step 1: Send `initialize` request and wait for server capabilities.
		let result_value = self
			.send_request(
				"initialize",
				serde_json::to_value(&params)
					.map_err(|e| McpError::Serialization(e.to_string()))?,
			)
			.await?;

		let result: McpInitializeResult =
			serde_json::from_value(result_value).map_err(|e| {
				McpError::ProtocolError(format!(
					"Failed to deserialize initialize result: {}",
					e
				))
			})?;

		// Step 2: Send `notifications/initialized` notification.
		// For HTTP, notifications are just regular POST requests (fire-and-forget).
		let _ = self
			.send_request("notifications/initialized", serde_json::json!({}))
			.await;

		Ok(result)
	}
}

#[async_trait]
impl Transport for HttpTransport {
	async fn connect(&mut self) -> Result<McpInitializeResult, McpError> {
		if self.connected.load(Ordering::SeqCst) {
			return Err(McpError::ConnectionFailed(
				"Already connected".into(),
			));
		}

		Self::validate_url(&self.config.url)?;

		let client = self.build_client()?;
		self.client = Some(client);
		self.connected.store(true, Ordering::SeqCst);

		match self.perform_handshake().await {
			Ok(result) => Ok(result),
			Err(e) => {
				// If handshake fails, mark as disconnected.
				self.connected.store(false, Ordering::SeqCst);
				self.client = None;
				Err(e)
			}
		}
	}

	async fn request<T: DeserializeOwned + Send>(
		&self,
		method: &str,
		params: serde_json::Value,
	) -> Result<T, McpError> {
		if !self.connected.load(Ordering::SeqCst) {
			return Err(McpError::ConnectionFailed(
				"HTTP transport is not connected".into(),
			));
		}

		let result = self.send_request(method, params).await?;
		serde_json::from_value(result).map_err(|e| {
			McpError::ProtocolError(format!(
				"Failed to deserialize response for '{}': {}",
				method, e
			))
		})
	}

	fn on_notification(&self, method: &str, _handler: NotificationHandler) -> SubscriptionHandle {
		tracing::warn!(
			"MCP HTTP transport does not support server notifications (method: {})",
			method
		);
		// Return a no-op handle that is already inactive.
		let active = Arc::new(AtomicBool::new(false));
		SubscriptionHandle { active }
	}

	async fn close(&mut self) -> Result<(), McpError> {
		self.connected.store(false, Ordering::SeqCst);
		self.client = None;
		Ok(())
	}

	fn is_connected(&self) -> bool {
		self.connected.load(Ordering::SeqCst)
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -------------------------------------------------------------------
	// HttpTransportConfig defaults
	// -------------------------------------------------------------------

	#[test]
	fn test_http_config_defaults() {
		let config = HttpTransportConfig::default();
		assert!(config.url.is_empty());
		assert_eq!(config.timeout_ms, 60_000);
		assert!(config.headers.is_empty());
		assert_eq!(config.client_name, "simse");
		assert_eq!(config.client_version, "1.0.0");
	}

	#[test]
	fn test_http_config_custom() {
		let config = HttpTransportConfig {
			url: "https://mcp.example.com/rpc".into(),
			timeout_ms: 30_000,
			headers: {
				let mut m = HashMap::new();
				m.insert("Authorization".into(), "Bearer token123".into());
				m
			},
			client_name: "test-client".into(),
			client_version: "2.0.0".into(),
		};
		assert_eq!(config.url, "https://mcp.example.com/rpc");
		assert_eq!(config.timeout_ms, 30_000);
		assert_eq!(
			config.headers.get("Authorization").unwrap(),
			"Bearer token123"
		);
	}

	// -------------------------------------------------------------------
	// URL validation
	// -------------------------------------------------------------------

	#[test]
	fn test_url_validation_http_valid() {
		assert!(HttpTransport::validate_url("http://localhost:3000").is_ok());
	}

	#[test]
	fn test_url_validation_https_valid() {
		assert!(HttpTransport::validate_url("https://mcp.example.com/rpc").is_ok());
	}

	#[test]
	fn test_url_validation_ftp_rejected() {
		let result = HttpTransport::validate_url("ftp://files.example.com");
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, McpError::TransportConfigError(_)));
		assert!(err.to_string().contains("http:// or https://"));
	}

	#[test]
	fn test_url_validation_empty_rejected() {
		let result = HttpTransport::validate_url("");
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, McpError::TransportConfigError(_)));
		assert!(err.to_string().contains("must not be empty"));
	}

	#[test]
	fn test_url_validation_no_scheme_rejected() {
		let result = HttpTransport::validate_url("localhost:3000");
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), McpError::TransportConfigError(_)));
	}

	#[test]
	fn test_url_validation_ws_rejected() {
		let result = HttpTransport::validate_url("ws://localhost:3000");
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), McpError::TransportConfigError(_)));
	}

	// -------------------------------------------------------------------
	// on_notification is no-op
	// -------------------------------------------------------------------

	#[test]
	fn test_on_notification_returns_inactive_handle() {
		let transport = HttpTransport::new(HttpTransportConfig {
			url: "http://localhost:3000".into(),
			..Default::default()
		});

		let handle = transport.on_notification(
			"notifications/tools/list_changed",
			Box::new(|_| {}),
		);

		// The handle should be born inactive since HTTP doesn't support
		// server notifications.
		assert!(!handle.is_active());
	}

	// -------------------------------------------------------------------
	// HttpTransport construction
	// -------------------------------------------------------------------

	#[test]
	fn test_http_transport_new_not_connected() {
		let transport = HttpTransport::new(HttpTransportConfig::default());
		assert!(!transport.is_connected());
	}

	#[test]
	fn test_http_transport_preserves_timeout() {
		let config = HttpTransportConfig {
			timeout_ms: 15_000,
			..Default::default()
		};
		let transport = HttpTransport::new(config);
		assert_eq!(transport.default_timeout_ms, 15_000);
	}

	// -------------------------------------------------------------------
	// Close marks as disconnected
	// -------------------------------------------------------------------

	#[tokio::test]
	async fn test_close_marks_disconnected() {
		let mut transport = HttpTransport::new(HttpTransportConfig {
			url: "http://localhost:3000".into(),
			..Default::default()
		});

		// Manually mark as connected for this test.
		transport.connected.store(true, Ordering::SeqCst);
		assert!(transport.is_connected());

		transport.close().await.unwrap();
		assert!(!transport.is_connected());
		assert!(transport.client.is_none());
	}

	// -------------------------------------------------------------------
	// Request when not connected
	// -------------------------------------------------------------------

	#[tokio::test]
	async fn test_request_when_not_connected() {
		let transport = HttpTransport::new(HttpTransportConfig {
			url: "http://localhost:3000".into(),
			..Default::default()
		});

		let result: Result<serde_json::Value, _> = transport
			.request("tools/list", serde_json::json!({}))
			.await;

		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, McpError::ConnectionFailed(_)));
		assert!(err.to_string().contains("not connected"));
	}
}
