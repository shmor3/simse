// ---------------------------------------------------------------------------
// Direct Rust API tests for VirtualNetwork (simse-sandbox vnet domain)
//
// Ports the JSON-RPC integration tests from simse-vnet/tests/integration.rs
// to direct Rust API calls against VirtualNetwork.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use simse_sandbox_engine::error::SandboxError;
use simse_sandbox_engine::vnet_backend::NetImpl;
use simse_sandbox_engine::vnet_local::LocalNet;
use simse_sandbox_engine::vnet_mock_store::MockResponse;
use simse_sandbox_engine::vnet_network::{SandboxInit, VirtualNetwork};
use simse_sandbox_engine::vnet_session::{Scheme, SessionType};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a VirtualNetwork initialized with default (empty) sandbox config.
fn new_initialized() -> VirtualNetwork {
    let mut net = VirtualNetwork::new();
    net.initialize(None, Some(NetImpl::Local(LocalNet::new())));
    net
}

/// Create a VirtualNetwork initialized with a sandbox config that allows
/// specific hosts via wildcard subdomain patterns.
fn new_with_allowed_hosts(hosts: Vec<String>) -> VirtualNetwork {
    let mut net = VirtualNetwork::new();
    let sandbox = SandboxInit {
        allowed_hosts: hosts,
        allowed_ports: vec![],
        allowed_protocols: vec![],
        default_timeout_ms: 30_000,
        max_response_bytes: 10 * 1024 * 1024,
        max_connections: 50,
    };
    net.initialize(Some(sandbox), Some(NetImpl::Local(LocalNet::new())));
    net
}

/// Helper to create a simple text MockResponse.
fn mock_response(status: u16, body: &str) -> MockResponse {
    MockResponse {
        status,
        headers: HashMap::new(),
        body: body.to_string(),
        body_type: "text".to_string(),
        delay_ms: None,
    }
}

/// Helper to create a MockResponse with custom headers.
fn mock_response_with_headers(
    status: u16,
    body: &str,
    headers: HashMap<String, String>,
) -> MockResponse {
    MockResponse {
        status,
        headers,
        body: body.to_string(),
        body_type: "text".to_string(),
        delay_ms: None,
    }
}

// ---------------------------------------------------------------------------
// Test 1: initialize returns ok
// ---------------------------------------------------------------------------

#[test]
fn initialize_returns_ok() {
    let mut net = VirtualNetwork::new();
    assert!(!net.is_initialized());
    net.initialize(None, Some(NetImpl::Local(LocalNet::new())));
    assert!(net.is_initialized());
}

// ---------------------------------------------------------------------------
// Test 2: method before init returns error
// ---------------------------------------------------------------------------

#[test]
fn method_before_init_returns_error() {
    let net = VirtualNetwork::new();
    let err = net.require_init().unwrap_err();
    assert!(matches!(err, SandboxError::VnetNotInitialized));
    assert_eq!(err.code(), "SANDBOX_VNET_NOT_INITIALIZED");
}

#[test]
fn mock_http_request_before_init_returns_error() {
    let mut net = VirtualNetwork::new();
    let err = net
        .mock_http_request("mock://x", Some("GET"), None, None)
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetNotInitialized));
}

#[test]
fn metrics_before_init_succeeds() {
    // metrics() doesn't call require_init -- it returns zero values
    let net = VirtualNetwork::new();
    let m = net.metrics();
    assert_eq!(m.total_requests, 0);
    assert_eq!(m.total_errors, 0);
}

// ---------------------------------------------------------------------------
// Test 3: mock register and HTTP request
// ---------------------------------------------------------------------------

#[test]
fn mock_register_and_http_request() {
    let mut net = new_initialized();

    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let id = net.register_mock(
        Some("GET".into()),
        "mock://api.example.com/users",
        mock_response_with_headers(200, "[{\"id\":1}]", headers),
        None,
    );
    assert!(!id.is_empty());

    let resp = net
        .mock_http_request(
            "mock://api.example.com/users",
            Some("GET"),
            None,
            None,
        )
        .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, "[{\"id\":1}]");
}

// ---------------------------------------------------------------------------
// Test 4: mock glob pattern
// ---------------------------------------------------------------------------

#[test]
fn mock_glob_pattern() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://api/*",
        mock_response(200, "ok"),
        None,
    );

    let resp = net
        .mock_http_request("mock://api/anything/here", None, None, None)
        .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, "ok");
}

// ---------------------------------------------------------------------------
// Test 5: mock no match returns error
// ---------------------------------------------------------------------------

#[test]
fn mock_no_match_returns_error() {
    let mut net = new_initialized();
    let err = net
        .mock_http_request("mock://nothing", None, None, None)
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetNoMockMatch(_)));
    assert_eq!(err.code(), "SANDBOX_VNET_NO_MOCK_MATCH");
}

// ---------------------------------------------------------------------------
// Test 6: mock times limit
// ---------------------------------------------------------------------------

#[test]
fn mock_times_limit() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://once",
        mock_response(200, "x"),
        Some(1),
    );

    // First request succeeds
    let resp = net
        .mock_http_request("mock://once", None, None, None)
        .unwrap();
    assert_eq!(resp.status, 200);

    // Second request fails (mock consumed)
    let err = net
        .mock_http_request("mock://once", None, None, None)
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetNoMockMatch(_)));
}

// ---------------------------------------------------------------------------
// Test 7: mock list and unregister
// ---------------------------------------------------------------------------

#[test]
fn mock_list_and_unregister() {
    let mut net = new_initialized();

    let id = net.register_mock(
        None,
        "mock://a",
        mock_response(200, ""),
        None,
    );

    let list = net.list_mocks();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);

    net.unregister_mock(&id).unwrap();

    let list = net.list_mocks();
    assert!(list.is_empty());
}

// ---------------------------------------------------------------------------
// Test 8: mock clear and history
// ---------------------------------------------------------------------------

#[test]
fn mock_clear_and_history() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://test/*",
        mock_response(200, "ok"),
        None,
    );

    net.mock_http_request("mock://test/1", None, None, None)
        .unwrap();
    net.mock_http_request("mock://test/2", None, None, None)
        .unwrap();

    let history = net.mock_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].url, "mock://test/1");
    assert_eq!(history[1].url, "mock://test/2");

    net.clear_mocks();

    let list = net.list_mocks();
    assert!(list.is_empty());
}

// ---------------------------------------------------------------------------
// Test 9: WebSocket connect, send (record activity), close
// ---------------------------------------------------------------------------

#[test]
fn ws_connect_send_close() {
    let mut net = new_initialized();

    // Create a mock WS session (same as the JSON-RPC server does for mock:// WS)
    let target = "ws.example.com/chat";
    net.check_connection_limit().unwrap();
    let session_id = net.create_session(SessionType::Ws, target, Scheme::Mock);

    // Verify session exists
    let info = net.get_session(&session_id).unwrap();
    assert_eq!(info.session_type, "ws");
    assert_eq!(info.scheme, "mock");
    assert_eq!(info.target, target);

    // Simulate sending a message (record activity)
    let data = "hello";
    let data_len = data.len() as u64;
    net.record_session_activity(&session_id, data_len, 0);

    // Close the session
    net.close_session(&session_id).unwrap();

    // After close, session should not be found
    let err = net.get_session(&session_id).unwrap_err();
    assert!(matches!(err, SandboxError::VnetSessionNotFound(_)));
}

// ---------------------------------------------------------------------------
// Test 10: session list and get
// ---------------------------------------------------------------------------

#[test]
fn session_list_and_get() {
    let mut net = new_initialized();

    let target = "ws.example.com/test";
    let session_id = net.create_session(SessionType::Ws, target, Scheme::Mock);

    let list = net.list_sessions();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, session_id);

    let info = net.get_session(&session_id).unwrap();
    assert_eq!(info.session_type, "ws");
    assert_eq!(info.scheme, "mock");
    assert_eq!(info.target, target);
}

// ---------------------------------------------------------------------------
// Test 11: metrics track activity
// ---------------------------------------------------------------------------

#[test]
fn metrics_track_activity() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://m/*",
        mock_response(200, "ok"),
        None,
    );

    net.mock_http_request("mock://m/1", None, None, None)
        .unwrap();
    net.mock_http_request("mock://m/2", None, None, None)
        .unwrap();

    let m = net.metrics();
    assert_eq!(m.total_requests, 2);
    assert_eq!(m.total_errors, 0);
}

#[test]
fn metrics_track_errors() {
    let mut net = new_initialized();

    // Request with no matching mock
    let _ = net.mock_http_request("mock://no-match", None, None, None);

    let m = net.metrics();
    assert_eq!(m.total_requests, 1);
    assert_eq!(m.total_errors, 1);
}

// ---------------------------------------------------------------------------
// Test 12: sandbox validation (blocked host/port)
// ---------------------------------------------------------------------------

#[test]
fn sandbox_blocks_unallowed_host() {
    let mut net = VirtualNetwork::new();
    // Default sandbox has empty allowlist = block all
    net.initialize(None, None);

    let err = net
        .validate_net_request("evil.com", 80, "http")
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetSandboxViolation(_)));
    assert_eq!(err.code(), "SANDBOX_VNET_SANDBOX_VIOLATION");
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
    net.initialize(Some(sandbox), None);
    assert!(net
        .validate_net_request("api.example.com", 443, "https")
        .is_ok());
}

#[test]
fn sandbox_blocks_disallowed_port() {
    let mut net = VirtualNetwork::new();
    let sandbox = SandboxInit {
        allowed_hosts: vec!["example.com".into()],
        allowed_ports: vec![(80, 80), (443, 443)],
        allowed_protocols: vec![],
        default_timeout_ms: 30_000,
        max_response_bytes: 10 * 1024 * 1024,
        max_connections: 50,
    };
    net.initialize(Some(sandbox), None);

    assert!(net.validate_net_request("example.com", 80, "http").is_ok());
    assert!(net.validate_net_request("example.com", 443, "https").is_ok());

    let err = net
        .validate_net_request("example.com", 8080, "http")
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetSandboxViolation(_)));
}

#[test]
fn sandbox_blocks_disallowed_protocol() {
    let mut net = VirtualNetwork::new();
    let sandbox = SandboxInit {
        allowed_hosts: vec!["example.com".into()],
        allowed_ports: vec![],
        allowed_protocols: vec!["https".into()],
        default_timeout_ms: 30_000,
        max_response_bytes: 10 * 1024 * 1024,
        max_connections: 50,
    };
    net.initialize(Some(sandbox), None);

    assert!(net
        .validate_net_request("example.com", 443, "https")
        .is_ok());

    let err = net
        .validate_net_request("example.com", 80, "http")
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetSandboxViolation(_)));
}

// ---------------------------------------------------------------------------
// Test 13: TCP connect and send (via sessions API)
// ---------------------------------------------------------------------------

#[test]
fn tcp_connect_and_send() {
    let mut net = new_initialized();

    // Create a TCP session (same as JSON-RPC server does for mock:// TCP)
    let target = "db.example.com:5432";
    net.check_connection_limit().unwrap();
    let session_id = net.create_session(SessionType::Tcp, target, Scheme::Mock);

    // Verify session
    let info = net.get_session(&session_id).unwrap();
    assert_eq!(info.session_type, "tcp");
    assert_eq!(info.target, target);

    // Simulate sending data
    let data = "SELECT 1";
    let bytes_written = data.len() as u64;
    net.record_session_activity(&session_id, bytes_written, 0);
    assert!(bytes_written > 0);

    // Close
    net.close_session(&session_id).unwrap();

    // After close, session should not be found
    let err = net.get_session(&session_id).unwrap_err();
    assert!(matches!(err, SandboxError::VnetSessionNotFound(_)));
}

// ---------------------------------------------------------------------------
// Test 14: UDP send with mock
// ---------------------------------------------------------------------------

#[test]
fn udp_send_with_mock() {
    let mut net = new_initialized();

    // Register a mock for UDP
    net.register_mock(
        Some("udp".into()),
        "mock://dns-server:53",
        mock_response(200, "udp-response-data"),
        None,
    );

    // Simulate UDP send (same as the JSON-RPC server: construct mock URL and find match)
    net.increment_requests();
    let mock_url = "mock://dns-server:53";
    let response = net.mock_store_find_match(mock_url, Some("udp"));
    assert!(response.is_some());
    let response = response.unwrap();
    assert_eq!(response.body, "udp-response-data");
}

// ---------------------------------------------------------------------------
// Test 15: DNS resolve with mock
// ---------------------------------------------------------------------------

#[test]
fn dns_resolve_with_mock() {
    let mut net = new_initialized();

    // Register a mock for DNS resolution
    net.register_mock(
        Some("dns".into()),
        "mock://dns/example.com",
        mock_response(200, "[\"1.2.3.4\", \"5.6.7.8\"]"),
        None,
    );

    // Simulate resolve (same as the JSON-RPC server: construct mock URL and find match)
    let mock_url = "mock://dns/example.com";
    let response = net.mock_store_find_match(mock_url, Some("dns"));
    assert!(response.is_some());
    let response = response.unwrap();
    let addresses: Vec<String> = serde_json::from_str(&response.body).unwrap();
    assert_eq!(addresses.len(), 2);
    assert_eq!(addresses[0], "1.2.3.4");
    assert_eq!(addresses[1], "5.6.7.8");
}

// ---------------------------------------------------------------------------
// Test 16: connection limit enforced
// ---------------------------------------------------------------------------

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
    net.initialize(Some(sandbox), None);

    // Create one session to fill the limit
    net.create_session(SessionType::Tcp, "a.com:80", Scheme::Net);

    // Second connection attempt should fail
    let err = net.check_connection_limit().unwrap_err();
    assert!(matches!(err, SandboxError::VnetLimitExceeded(_)));
    assert_eq!(err.code(), "SANDBOX_VNET_LIMIT_EXCEEDED");
}

// ---------------------------------------------------------------------------
// Test 17: unregister nonexistent mock returns error
// ---------------------------------------------------------------------------

#[test]
fn unregister_nonexistent_mock_returns_error() {
    let mut net = new_initialized();
    let err = net.unregister_mock("nonexistent-id").unwrap_err();
    assert!(matches!(err, SandboxError::VnetMockNotFound(_)));
    assert_eq!(err.code(), "SANDBOX_VNET_MOCK_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// Test 18: close nonexistent session returns error
// ---------------------------------------------------------------------------

#[test]
fn close_nonexistent_session_returns_error() {
    let mut net = new_initialized();
    let err = net.close_session("nonexistent-id").unwrap_err();
    assert!(matches!(err, SandboxError::VnetSessionNotFound(_)));
    assert_eq!(err.code(), "SANDBOX_VNET_SESSION_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// Test 19: get nonexistent session returns error
// ---------------------------------------------------------------------------

#[test]
fn get_nonexistent_session_returns_error() {
    let net = new_initialized();
    let err = net.get_session("nonexistent-id").unwrap_err();
    assert!(matches!(err, SandboxError::VnetSessionNotFound(_)));
}

// ---------------------------------------------------------------------------
// Test 20: bytes_total tracked in metrics
// ---------------------------------------------------------------------------

#[test]
fn bytes_total_tracked() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://bytes/*",
        mock_response(200, "12345"),
        None,
    );

    net.mock_http_request("mock://bytes/test", None, None, None)
        .unwrap();

    let m = net.metrics();
    assert_eq!(m.bytes_total, 5); // "12345" is 5 bytes
}

// ---------------------------------------------------------------------------
// Test 21: active sessions count in metrics
// ---------------------------------------------------------------------------

#[test]
fn active_sessions_in_metrics() {
    let mut net = new_initialized();

    assert_eq!(net.metrics().active_sessions, 0);

    let s1 = net.create_session(SessionType::Ws, "a.com:443", Scheme::Mock);
    assert_eq!(net.metrics().active_sessions, 1);

    let _s2 = net.create_session(SessionType::Tcp, "b.com:80", Scheme::Net);
    assert_eq!(net.metrics().active_sessions, 2);

    net.close_session(&s1).unwrap();
    assert_eq!(net.metrics().active_sessions, 1);
}

// ---------------------------------------------------------------------------
// Test 22: mock method filtering
// ---------------------------------------------------------------------------

#[test]
fn mock_method_filtering() {
    let mut net = new_initialized();

    net.register_mock(
        Some("POST".into()),
        "mock://api/users",
        mock_response(201, "created"),
        None,
    );

    // POST should match
    let resp = net
        .mock_http_request("mock://api/users", Some("POST"), None, None)
        .unwrap();
    assert_eq!(resp.status, 201);

    // GET should not match
    let err = net
        .mock_http_request("mock://api/users", Some("GET"), None, None)
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetNoMockMatch(_)));
}

// ---------------------------------------------------------------------------
// Test 23: mock with None method matches any method
// ---------------------------------------------------------------------------

#[test]
fn mock_none_method_matches_any() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://api/anything",
        mock_response(200, "any"),
        None,
    );

    let resp_get = net
        .mock_http_request("mock://api/anything", Some("GET"), None, None)
        .unwrap();
    assert_eq!(resp_get.status, 200);

    let resp_post = net
        .mock_http_request("mock://api/anything", Some("POST"), None, None)
        .unwrap();
    assert_eq!(resp_post.status, 200);
}

// ---------------------------------------------------------------------------
// Test 24: default timeout and max response bytes
// ---------------------------------------------------------------------------

#[test]
fn default_timeout_and_max_response_bytes() {
    let mut net = VirtualNetwork::new();
    let sandbox = SandboxInit {
        allowed_hosts: vec!["*".into()],
        allowed_ports: vec![],
        allowed_protocols: vec![],
        default_timeout_ms: 5000,
        max_response_bytes: 1024,
        max_connections: 10,
    };
    net.initialize(Some(sandbox), None);

    assert_eq!(net.default_timeout(), 5000);
    assert_eq!(net.max_response_bytes(), 1024);
}

// ---------------------------------------------------------------------------
// Test 25: multiple mocks first match wins
// ---------------------------------------------------------------------------

#[test]
fn first_mock_match_wins() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://api/*",
        mock_response(200, "first"),
        None,
    );
    net.register_mock(
        None,
        "mock://api/*",
        mock_response(404, "second"),
        None,
    );

    let resp = net
        .mock_http_request("mock://api/test", None, None, None)
        .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, "first");
}

// ---------------------------------------------------------------------------
// Test 26: validate_net_request before init returns error
// ---------------------------------------------------------------------------

#[test]
fn validate_net_request_before_init_returns_error() {
    let net = VirtualNetwork::new();
    let err = net
        .validate_net_request("example.com", 80, "http")
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetNotInitialized));
}

// ---------------------------------------------------------------------------
// Test 27: check_connection_limit before init returns error
// ---------------------------------------------------------------------------

#[test]
fn check_connection_limit_before_init_returns_error() {
    let net = VirtualNetwork::new();
    let err = net.check_connection_limit().unwrap_err();
    assert!(matches!(err, SandboxError::VnetNotInitialized));
}

// ---------------------------------------------------------------------------
// Test 28: mock history records method
// ---------------------------------------------------------------------------

#[test]
fn mock_history_records_method() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://api/*",
        mock_response(200, "ok"),
        None,
    );

    net.mock_http_request("mock://api/one", Some("GET"), None, None)
        .unwrap();
    net.mock_http_request("mock://api/two", Some("POST"), None, None)
        .unwrap();

    let history = net.mock_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].url, "mock://api/one");
    assert_eq!(history[0].method.as_deref(), Some("GET"));
    assert_eq!(history[1].url, "mock://api/two");
    assert_eq!(history[1].method.as_deref(), Some("POST"));
}

// ---------------------------------------------------------------------------
// Test 29: session bytes tracking
// ---------------------------------------------------------------------------

#[test]
fn session_bytes_tracking() {
    let mut net = new_initialized();

    let session_id = net.create_session(SessionType::Tcp, "db:5432", Scheme::Mock);

    net.record_session_activity(&session_id, 100, 200);
    net.record_session_activity(&session_id, 50, 75);

    let info = net.get_session(&session_id).unwrap();
    assert_eq!(info.bytes_sent, 150);
    assert_eq!(info.bytes_received, 275);
}

// ---------------------------------------------------------------------------
// Test 30: sandbox with wildcard subdomain allows matching hosts
// ---------------------------------------------------------------------------

#[test]
fn sandbox_wildcard_subdomain_allows_matching() {
    let net = new_with_allowed_hosts(vec!["*.example.com".into()]);

    assert!(net
        .validate_net_request("api.example.com", 8080, "http")
        .is_ok());
    assert!(net
        .validate_net_request("ws.example.com", 443, "https")
        .is_ok());
    // The bare domain should not match *.example.com
    assert!(net
        .validate_net_request("example.com", 80, "http")
        .is_err());
    // Unrelated host should not match
    assert!(net
        .validate_net_request("evil.com", 80, "http")
        .is_err());
}

// ---------------------------------------------------------------------------
// Test 31: VirtualNetwork::default() creates uninitialized instance
// ---------------------------------------------------------------------------

#[test]
fn default_creates_uninitialized() {
    let net = VirtualNetwork::default();
    assert!(!net.is_initialized());
}

// ---------------------------------------------------------------------------
// Test 32: mock clear also clears history
// ---------------------------------------------------------------------------

#[test]
fn mock_clear_also_clears_history() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://h/*",
        mock_response(200, "ok"),
        None,
    );

    net.mock_http_request("mock://h/1", None, None, None)
        .unwrap();
    assert_eq!(net.mock_history().len(), 1);

    net.clear_mocks();
    assert!(net.mock_history().is_empty());
}

// ---------------------------------------------------------------------------
// Test 33: net_http_request without backend returns error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn net_http_request_without_backend_returns_error() {
    let mut net = VirtualNetwork::new();
    net.initialize(None, None); // No backend

    let err = net
        .net_http_request("http://example.com", "GET", &HashMap::new(), None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, SandboxError::VnetConnectionFailed(_)));
}

// ---------------------------------------------------------------------------
// Test 34: multiple sessions of different types
// ---------------------------------------------------------------------------

#[test]
fn multiple_session_types() {
    let mut net = new_initialized();

    let ws_id = net.create_session(SessionType::Ws, "ws.com:443", Scheme::Mock);
    let tcp_id = net.create_session(SessionType::Tcp, "db.com:5432", Scheme::Net);

    let list = net.list_sessions();
    assert_eq!(list.len(), 2);

    let ws_info = net.get_session(&ws_id).unwrap();
    assert_eq!(ws_info.session_type, "ws");
    assert_eq!(ws_info.scheme, "mock");

    let tcp_info = net.get_session(&tcp_id).unwrap();
    assert_eq!(tcp_info.session_type, "tcp");
    assert_eq!(tcp_info.scheme, "net");
}

// ---------------------------------------------------------------------------
// Test 35: consumed mock not listed
// ---------------------------------------------------------------------------

#[test]
fn consumed_mock_not_listed() {
    let mut net = new_initialized();

    net.register_mock(
        None,
        "mock://consumed",
        mock_response(200, "ok"),
        Some(1),
    );

    assert_eq!(net.list_mocks().len(), 1);

    // Consume the mock
    net.mock_http_request("mock://consumed", None, None, None)
        .unwrap();

    // Consumed mock should not appear in list
    assert!(net.list_mocks().is_empty());
}

// ---------------------------------------------------------------------------
// Test 36: mock list returns metadata
// ---------------------------------------------------------------------------

#[test]
fn mock_list_returns_metadata() {
    let mut net = new_initialized();

    net.register_mock(
        Some("GET".into()),
        "mock://api/items",
        mock_response(200, "items"),
        Some(5),
    );

    let list = net.list_mocks();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].method.as_deref(), Some("GET"));
    assert_eq!(list[0].url_pattern, "mock://api/items");
    assert_eq!(list[0].status, 200);
    assert_eq!(list[0].times, Some(5));
    assert_eq!(list[0].remaining, Some(5));
}
