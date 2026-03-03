use std::collections::HashMap;

use crate::error::VnetError;
use crate::mock_store::{self, MockStore};
use crate::protocol::{HttpResponseResult, MetricsResult, MockDefinitionInfo, MockHitInfo, SessionInfo};
use crate::sandbox::{HostRule, NetSandboxConfig, PortRange};
use crate::session::{Scheme, SessionManager, SessionType};

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
    mocks: MockStore,
    pub(crate) sessions: SessionManager,
    pub(crate) total_requests: u64,
    pub(crate) total_errors: u64,
    bytes_total: u64,
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
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn initialize(&mut self, sandbox: Option<SandboxInit>) {
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
        self.initialized = true;
    }

    pub fn require_init(&self) -> Result<(), VnetError> {
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
        self.require_init()?;
        self.total_requests += 1;

        match self.mocks.find_match(url, method) {
            Some((_id, response)) => {
                let body_len = response.body.len() as u64;
                self.bytes_total += body_len;

                Ok(HttpResponseResult {
                    status: response.status,
                    headers: response.headers,
                    body: response.body,
                    body_type: response.body_type,
                    duration_ms: response.delay_ms.unwrap_or(0),
                    bytes_received: body_len,
                })
            }
            None => {
                self.total_errors += 1;
                Err(VnetError::NoMockMatch(url.to_string()))
            }
        }
    }

    // ── Sandbox validation ──

    pub fn validate_net_request(
        &self,
        host: &str,
        port: u16,
        protocol: &str,
    ) -> Result<(), VnetError> {
        self.require_init()?;
        self.sandbox
            .validate(host, port, protocol)
            .map_err(VnetError::SandboxViolation)
    }

    pub fn check_connection_limit(&self) -> Result<(), VnetError> {
        self.require_init()?;
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
        self.mocks.register(method, url_pattern, response, times)
    }

    pub fn unregister_mock(&mut self, id: &str) -> Result<(), VnetError> {
        if self.mocks.unregister(id) {
            Ok(())
        } else {
            Err(VnetError::MockNotFound(id.to_string()))
        }
    }

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

    pub fn clear_mocks(&mut self) {
        self.mocks.clear();
    }

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

    pub fn mock_store_find_match(
        &mut self,
        url: &str,
        method: Option<&str>,
    ) -> Option<mock_store::MockResponse> {
        self.mocks.find_match(url, method).map(|(_, resp)| resp)
    }

    // ── Session management ──

    pub fn create_session(
        &mut self,
        session_type: SessionType,
        target: &str,
        scheme: Scheme,
    ) -> String {
        self.sessions.create(session_type, target, scheme)
    }

    pub fn get_session(&self, id: &str) -> Result<SessionInfo, VnetError> {
        let session = self
            .sessions
            .get(id)
            .ok_or_else(|| VnetError::SessionNotFound(id.to_string()))?;

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
        let mut net = VirtualNetwork::new();
        let err = net
            .mock_http_request("mock://x", Some("GET"), None, None)
            .unwrap_err();
        assert!(matches!(err, VnetError::NotInitialized));
    }

    #[test]
    fn initialize_sets_state() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);
        assert!(net.is_initialized());
    }

    #[test]
    fn mock_http_request_no_match() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);
        let err = net
            .mock_http_request("mock://api/test", Some("GET"), None, None)
            .unwrap_err();
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
        let resp = net
            .mock_http_request("mock://api/users", Some("GET"), None, None)
            .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "{\"users\":[]}");
    }

    #[test]
    fn metrics_track_requests() {
        let mut net = VirtualNetwork::new();
        net.initialize(None);
        net.register_mock(
            None,
            "mock://api/*",
            mock_store::MockResponse {
                status: 200,
                headers: HashMap::new(),
                body: "ok".into(),
                body_type: "text".into(),
                delay_ms: None,
            },
            None,
        );
        net.mock_http_request("mock://api/a", None, None, None)
            .unwrap();
        net.mock_http_request("mock://api/b", None, None, None)
            .unwrap();
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
        let err = net
            .validate_net_request("evil.com", 80, "http")
            .unwrap_err();
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
        assert!(net
            .validate_net_request("api.example.com", 443, "https")
            .is_ok());
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

        let id = net.register_mock(
            None,
            "mock://a",
            mock_store::MockResponse {
                status: 200,
                headers: HashMap::new(),
                body: "".into(),
                body_type: "text".into(),
                delay_ms: None,
            },
            None,
        );

        assert_eq!(net.list_mocks().len(), 1);
        net.unregister_mock(&id).unwrap();
        assert!(net.list_mocks().is_empty());

        net.register_mock(
            None,
            "mock://b",
            mock_store::MockResponse {
                status: 200,
                headers: HashMap::new(),
                body: "".into(),
                body_type: "text".into(),
                delay_ms: None,
            },
            None,
        );
        net.clear_mocks();
        assert!(net.list_mocks().is_empty());
    }
}
