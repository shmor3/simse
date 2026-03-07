use std::collections::HashMap;

use crate::sandbox::error::SandboxError;
use crate::sandbox::vnet_backend::NetImpl;
use crate::sandbox::vnet_mock_store::{self, MockStore};
use crate::sandbox::vnet_sandbox::{HostRule, NetSandboxConfig, PortRange};
use crate::sandbox::vnet_session::{Scheme, SessionManager, SessionType};
use crate::sandbox::vnet_types::{HttpResponseResult, MetricsResult, MockDefinitionInfo, MockHitInfo, SessionInfo};

// ── VirtualNetwork ──────────────────────────────────────────────────────────

pub struct SandboxInit {
    pub allowed_hosts: Vec<String>,
    pub allowed_ports: Vec<(u16, u16)>,
    pub allowed_protocols: Vec<String>,
    pub default_timeout_ms: u64,
    pub max_response_bytes: u64,
    pub max_connections: usize,
}

/// Pure state for the virtual network — uses `im`-backed collections
/// (via `MockStore` and `SessionManager`) for cheap cloning and
/// functional-style owned-return transitions.
///
/// The `backend` field is the I/O boundary: methods that touch it
/// are marked with `// PERF: hot-path I/O` and keep `&mut self`.
pub struct VirtualNetwork {
    initialized: bool,
    sandbox: NetSandboxConfig,
    mocks: MockStore,
    pub(crate) sessions: SessionManager,
    pub(crate) total_requests: u64,
    pub(crate) total_errors: u64,
    bytes_total: u64,
    backend: Option<NetImpl>,
}

impl Default for VirtualNetwork {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualNetwork {
    pub fn new() -> Self {
        Self {
            initialized: false,
            sandbox: NetSandboxConfig::default(),
            mocks: MockStore::new(),
            sessions: SessionManager::new(),
            total_requests: 0,
            total_errors: 0,
            bytes_total: 0,
            backend: None,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Initialize the network with sandbox config and optional backend.
    /// Owned-return: consumes self, returns `Self`.
    pub fn initialize(
        mut self,
        sandbox: Option<SandboxInit>,
        backend: Option<NetImpl>,
    ) -> Self {
        if let Some(init) = sandbox {
            self.sandbox = NetSandboxConfig {
                allowed_hosts: init.allowed_hosts.iter().map(|h| HostRule::parse(h)).collect(),
                allowed_ports: init
                    .allowed_ports
                    .iter()
                    .map(|(start, end)| PortRange {
                        start: *start,
                        end: *end,
                    })
                    .collect(),
                allowed_protocols: init.allowed_protocols,
                default_timeout_ms: init.default_timeout_ms,
                max_response_bytes: init.max_response_bytes,
                max_connections: init.max_connections,
            };
        } else {
            self.sandbox = NetSandboxConfig::default();
        }
        self.backend = backend;
        self.initialized = true;
        self
    }

    /// Returns a reference to the backend, if one has been set.
    pub fn backend(&self) -> Option<&NetImpl> {
        self.backend.as_ref()
    }

    pub fn require_init(&self) -> Result<(), SandboxError> {
        if !self.initialized {
            return Err(SandboxError::VnetNotInitialized);
        }
        Ok(())
    }

    // ── Mock HTTP ──

    /// Perform a mock HTTP request. Owned-return: consumes self, returns
    /// `(Self, Result<HttpResponseResult, SandboxError>)`.
    pub fn mock_http_request(
        mut self,
        url: &str,
        method: Option<&str>,
        _headers: Option<&HashMap<String, String>>,
        _body: Option<&str>,
    ) -> (Self, Result<HttpResponseResult, SandboxError>) {
        if let Err(e) = self.require_init() {
            return (self, Err(e));
        }
        self.total_requests += 1;

        let (mocks, found) = self.mocks.find_match(url, method);
        self.mocks = mocks;

        match found {
            Some((_id, response)) => {
                let body_len = response.body.len() as u64;
                self.bytes_total += body_len;

                (self, Ok(HttpResponseResult {
                    status: response.status,
                    headers: response.headers,
                    body: response.body,
                    body_type: response.body_type,
                    duration_ms: response.delay_ms.unwrap_or(0),
                    bytes_received: body_len,
                }))
            }
            None => {
                self.total_errors += 1;
                (self, Err(SandboxError::VnetNoMockMatch(url.to_string())))
            }
        }
    }

    // ── Net HTTP (backend-delegated) ──

    /// Send an HTTP request through the backend (for `net://` scheme).
    ///
    /// The caller strips `net://` from the URL and provides the real URL.
    /// This method increments metrics and delegates to the backend.
    // PERF: hot-path I/O — requires &mut self for backend access + async
    pub async fn net_http_request(
        &mut self,
        real_url: &str,
        method: &str,
        headers: &HashMap<String, String>,
        body: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<HttpResponseResult, SandboxError> {
        self.require_init()?;
        self.total_requests += 1;

        let backend = self.backend.as_ref().ok_or_else(|| {
            SandboxError::VnetConnectionFailed("no network backend configured".to_string())
        })?;

        let timeout = timeout_ms.unwrap_or(self.sandbox.default_timeout_ms);
        let max_bytes = self.sandbox.max_response_bytes;

        match backend.http_request(real_url, method, headers, body, timeout, max_bytes).await {
            Ok(result) => {
                self.bytes_total += result.bytes_received;
                Ok(result)
            }
            Err(e) => {
                self.total_errors += 1;
                Err(e)
            }
        }
    }

    // ── Sandbox validation ──

    pub fn validate_net_request(
        &self,
        host: &str,
        port: u16,
        protocol: &str,
    ) -> Result<(), SandboxError> {
        self.require_init()?;
        self.sandbox
            .validate(host, port, protocol)
            .map_err(SandboxError::VnetSandboxViolation)
    }

    pub fn check_connection_limit(&self) -> Result<(), SandboxError> {
        self.require_init()?;
        if self.sessions.active_count() >= self.sandbox.max_connections {
            return Err(SandboxError::VnetLimitExceeded(format!(
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

    /// Register a new mock. Owned-return: consumes self, returns `(Self, String)`.
    pub fn register_mock(
        mut self,
        method: Option<String>,
        url_pattern: &str,
        response: vnet_mock_store::MockResponse,
        times: Option<usize>,
    ) -> (Self, String) {
        let (mocks, id) = self.mocks.register(method, url_pattern, response, times);
        self.mocks = mocks;
        (self, id)
    }

    /// Unregister a mock by ID. Owned-return: consumes self, returns
    /// `(Self, Result<(), SandboxError>)`.
    pub fn unregister_mock(mut self, id: &str) -> (Self, Result<(), SandboxError>) {
        let (mocks, removed) = self.mocks.unregister(id);
        self.mocks = mocks;
        if removed {
            (self, Ok(()))
        } else {
            (self, Err(SandboxError::VnetMockNotFound(id.to_string())))
        }
    }

    /// List active mocks. Read-only.
    pub fn list_mocks(&self) -> Vec<MockDefinitionInfo> {
        self.mocks
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

    /// Clear all mocks. Owned-return: consumes self, returns `Self`.
    pub fn clear_mocks(mut self) -> Self {
        self.mocks = self.mocks.clear();
        self
    }

    /// Get mock hit history. Read-only.
    pub fn mock_history(&self) -> Vec<MockHitInfo> {
        self.mocks
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

    /// Find a matching mock (consuming call). Owned-return: consumes self,
    /// returns `(Self, Option<MockResponse>)`.
    pub fn mock_store_find_match(
        mut self,
        url: &str,
        method: Option<&str>,
    ) -> (Self, Option<vnet_mock_store::MockResponse>) {
        let (mocks, found) = self.mocks.find_match(url, method);
        self.mocks = mocks;
        (self, found.map(|(_, resp)| resp))
    }

    // ── Session management ──

    /// Create a new session. Owned-return: consumes self, returns `(Self, String)`.
    pub fn create_session(
        mut self,
        session_type: SessionType,
        target: &str,
        scheme: Scheme,
    ) -> (Self, String) {
        let (sessions, id) = self.sessions.create(session_type, target, scheme);
        self.sessions = sessions;
        (self, id)
    }

    /// Get a session by ID. Read-only.
    pub fn get_session(&self, id: &str) -> Result<SessionInfo, SandboxError> {
        let session = self
            .sessions
            .get(id)
            .ok_or_else(|| SandboxError::VnetSessionNotFound(id.to_string()))?;

        Ok(SessionInfo {
            id: session.id.clone(),
            session_type: session.session_type.as_str().to_string(),
            target: session.target.clone(),
            scheme: session.scheme.as_str().to_string(),
            created_at: session.created_at,
            last_active_at: session.last_active_at,
            bytes_sent: session.bytes_sent,
            bytes_received: session.bytes_received,
        })
    }

    /// List all sessions. Read-only.
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

    /// Close a session by ID. Owned-return: consumes self, returns
    /// `(Self, Result<(), SandboxError>)`.
    pub fn close_session(mut self, id: &str) -> (Self, Result<(), SandboxError>) {
        let (sessions, removed) = self.sessions.close(id);
        self.sessions = sessions;
        if removed {
            (self, Ok(()))
        } else {
            (self, Err(SandboxError::VnetSessionNotFound(id.to_string())))
        }
    }

    // ── Session activity (for external callers) ──

    /// Record bytes sent/received on an existing session.
    /// Owned-return: consumes self, returns `Self`.
    pub fn record_session_activity(
        mut self,
        session_id: &str,
        bytes_sent: u64,
        bytes_received: u64,
    ) -> Self {
        self.sessions = self.sessions.record_activity(session_id, bytes_sent, bytes_received);
        self
    }

    /// Increment the total request counter (for protocols handled externally).
    /// Owned-return: consumes self, returns `Self`.
    pub fn increment_requests(mut self) -> Self {
        self.total_requests += 1;
        self
    }

    // ── Metrics ──

    pub fn metrics(&self) -> MetricsResult {
        MetricsResult {
            total_requests: self.total_requests,
            total_errors: self.total_errors,
            active_sessions: self.sessions.active_count(),
            bytes_total: self.bytes_total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn not_initialized_error() {
        let net = VirtualNetwork::new();
        let (_, err) = net.mock_http_request("mock://x", Some("GET"), None, None);
        assert!(matches!(err.unwrap_err(), SandboxError::VnetNotInitialized));
    }

    #[test]
    fn initialize_sets_state() {
        let net = VirtualNetwork::new();
        let net = net.initialize(None, None);
        assert!(net.is_initialized());
    }

    #[test]
    fn mock_http_request_no_match() {
        let net = VirtualNetwork::new();
        let net = net.initialize(None, None);
        let (_, err) = net.mock_http_request("mock://api/test", Some("GET"), None, None);
        assert!(matches!(err.unwrap_err(), SandboxError::VnetNoMockMatch(_)));
    }

    #[test]
    fn mock_http_request_success() {
        let net = VirtualNetwork::new();
        let net = net.initialize(None, None);
        let (net, _id) = net.register_mock(
            Some("GET".into()),
            "mock://api/users",
            vnet_mock_store::MockResponse {
                status: 200,
                headers: HashMap::new(),
                body: "{\"users\":[]}".into(),
                body_type: "text".into(),
                delay_ms: None,
            },
            None,
        );
        let (_, resp) = net.mock_http_request("mock://api/users", Some("GET"), None, None);
        let resp = resp.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "{\"users\":[]}");
    }

    #[test]
    fn metrics_track_requests() {
        let net = VirtualNetwork::new();
        let net = net.initialize(None, None);
        let (net, _) = net.register_mock(
            None,
            "mock://api/*",
            vnet_mock_store::MockResponse {
                status: 200,
                headers: HashMap::new(),
                body: "ok".into(),
                body_type: "text".into(),
                delay_ms: None,
            },
            None,
        );
        let (net, _) = net.mock_http_request("mock://api/a", None, None, None);
        let (net, _) = net.mock_http_request("mock://api/b", None, None, None);
        let m = net.metrics();
        assert_eq!(m.total_requests, 2);
        assert_eq!(m.total_errors, 0);
    }

    #[test]
    fn metrics_track_errors() {
        let net = VirtualNetwork::new();
        let net = net.initialize(None, None);
        let (net, _) = net.mock_http_request("mock://no-match", None, None, None);
        let m = net.metrics();
        assert_eq!(m.total_requests, 1);
        assert_eq!(m.total_errors, 1);
    }

    #[test]
    fn sandbox_blocks_net_request() {
        let net = VirtualNetwork::new();
        let net = net.initialize(None, None); // empty allowlist = block all
        let err = net
            .validate_net_request("evil.com", 80, "http")
            .unwrap_err();
        assert!(matches!(err, SandboxError::VnetSandboxViolation(_)));
    }

    #[test]
    fn sandbox_allows_configured_host() {
        let net = VirtualNetwork::new();
        let sandbox = SandboxInit {
            allowed_hosts: vec!["api.example.com".into()],
            allowed_ports: vec![],
            allowed_protocols: vec![],
            default_timeout_ms: 30_000,
            max_response_bytes: 10 * 1024 * 1024,
            max_connections: 50,
        };
        let net = net.initialize(Some(sandbox), None);
        assert!(net
            .validate_net_request("api.example.com", 443, "https")
            .is_ok());
    }

    #[test]
    fn connection_limit_enforced() {
        let net = VirtualNetwork::new();
        let sandbox = SandboxInit {
            allowed_hosts: vec!["*".into()],
            allowed_ports: vec![],
            allowed_protocols: vec![],
            default_timeout_ms: 30_000,
            max_response_bytes: 10 * 1024 * 1024,
            max_connections: 1,
        };
        let net = net.initialize(Some(sandbox), None);
        let (net, _) = net.create_session(
            crate::sandbox::vnet_session::SessionType::Tcp,
            "a.com:80",
            crate::sandbox::vnet_session::Scheme::Net,
        );
        let err = net.check_connection_limit().unwrap_err();
        assert!(matches!(err, SandboxError::VnetLimitExceeded(_)));
    }

    #[test]
    fn mock_register_list_unregister_clear() {
        let net = VirtualNetwork::new();
        let net = net.initialize(None, None);

        let (net, id) = net.register_mock(
            None,
            "mock://a",
            vnet_mock_store::MockResponse {
                status: 200,
                headers: HashMap::new(),
                body: "".into(),
                body_type: "text".into(),
                delay_ms: None,
            },
            None,
        );

        assert_eq!(net.list_mocks().len(), 1);
        let (net, result) = net.unregister_mock(&id);
        result.unwrap();
        assert!(net.list_mocks().is_empty());

        let (net, _) = net.register_mock(
            None,
            "mock://b",
            vnet_mock_store::MockResponse {
                status: 200,
                headers: HashMap::new(),
                body: "".into(),
                body_type: "text".into(),
                delay_ms: None,
            },
            None,
        );
        let net = net.clear_mocks();
        assert!(net.list_mocks().is_empty());
    }
}
