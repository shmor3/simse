use std::io::{self, BufRead};

use crate::error::VnetError;
use crate::local_backend::LocalNetBackend;
use crate::mock_store::MockResponse;
use crate::network::{SandboxInit, VirtualNetwork};
use crate::protocol::*;
use crate::session::{Scheme, SessionType};
use crate::transport::NdjsonTransport;

/// VNET JSON-RPC server -- dispatches incoming requests to virtual network operations.
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

    // -- Dispatch -------------------------------------------------------------

    async fn dispatch(&mut self, req: JsonRpcRequest) {
        // Methods that may need async (net:// backend calls) are dispatched
        // separately so we can .await them.
        let result = match req.method.as_str() {
            "initialize" => self.handle_initialize(req.params),

            // Network -- async path for methods that may hit a real backend
            "net/httpRequest" => {
                let r = self.handle_http_request(req.params).await;
                self.write_result(req.id, r);
                return;
            }

            // Network -- sync paths (backend delegation for these is future work)
            "net/wsConnect" => self.handle_ws_connect(req.params),
            "net/wsMessage" => self.handle_ws_message(req.params),
            "net/wsClose" => self.handle_ws_close(req.params),
            "net/tcpConnect" => self.handle_tcp_connect(req.params),
            "net/tcpSend" => self.handle_tcp_send(req.params),
            "net/tcpClose" => self.handle_tcp_close(req.params),
            "net/udpSend" => self.handle_udp_send(req.params),
            "net/resolve" => self.handle_resolve(req.params),
            "net/metrics" => self.handle_metrics(req.params),

            // Mock
            "mock/register" => self.handle_mock_register(req.params),
            "mock/unregister" => self.handle_mock_unregister(req.params),
            "mock/list" => self.handle_mock_list(req.params),
            "mock/clear" => self.handle_mock_clear(req.params),
            "mock/history" => self.handle_mock_history(req.params),

            // Session
            "session/list" => self.handle_session_list(req.params),
            "session/get" => self.handle_session_get(req.params),
            "session/close" => self.handle_session_close(req.params),

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

    fn write_result(&mut self, id: u64, result: Result<serde_json::Value, VnetError>) {
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

    // -- Network accessors ----------------------------------------------------

    fn with_network(&self) -> Result<&VirtualNetwork, VnetError> {
        self.network.as_ref().ok_or(VnetError::NotInitialized)
    }

    fn with_network_mut(&mut self) -> Result<&mut VirtualNetwork, VnetError> {
        self.network.as_mut().ok_or(VnetError::NotInitialized)
    }

    // -- Initialize -----------------------------------------------------------

    fn handle_initialize(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: InitializeParams = parse_params(params)?;

        let sandbox_init = p.sandbox.map(|s| SandboxInit {
            allowed_hosts: s.allowed_hosts.unwrap_or_default(),
            allowed_ports: s
                .allowed_ports
                .unwrap_or_default()
                .into_iter()
                .map(|pr| (pr.start, pr.end))
                .collect(),
            allowed_protocols: s.allowed_protocols.unwrap_or_default(),
            default_timeout_ms: s.default_timeout_ms.unwrap_or(30_000),
            max_response_bytes: s.max_response_bytes.unwrap_or(10 * 1024 * 1024),
            max_connections: s.max_connections.unwrap_or(50),
        });

        let backend = LocalNetBackend::new();

        let mut network = VirtualNetwork::new();
        network.initialize(sandbox_init, Some(Box::new(backend)));
        self.network = Some(network);

        Ok(serde_json::json!({ "ok": true }))
    }

    // -- net/httpRequest ------------------------------------------------------

    async fn handle_http_request(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: HttpRequestParams = parse_params(params)?;
        let url = &p.url;

        if url.starts_with("mock://") {
            let net = self.with_network_mut()?;
            let result = net.mock_http_request(
                url,
                p.method.as_deref(),
                p.headers.as_ref(),
                p.body.as_deref(),
            )?;
            Ok(serde_json::to_value(result)?)
        } else if url.starts_with("net://") {
            // Strip the net:// prefix and reconstruct as a real URL.
            // net://https://example.com/path -> https://example.com/path
            // net://example.com/path         -> https://example.com/path (default)
            let remainder = &url[6..]; // strip "net://"
            let real_url = if remainder.starts_with("http://") || remainder.starts_with("https://") {
                remainder.to_string()
            } else {
                format!("https://{remainder}")
            };

            let method = p.method.as_deref().unwrap_or("GET");
            let headers = p.headers.unwrap_or_default();
            let net = self.with_network_mut()?;
            let result = net
                .net_http_request(&real_url, method, &headers, p.body.as_deref(), p.timeout_ms)
                .await?;
            Ok(serde_json::to_value(result)?)
        } else {
            Err(VnetError::InvalidParams(format!(
                "unsupported URL scheme: {url} (use mock:// or net://)"
            )))
        }
    }

    // -- net/wsConnect --------------------------------------------------------

    fn handle_ws_connect(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: WsConnectParams = parse_params(params)?;
        let url = &p.url;

        if url.starts_with("mock://") {
            let target = url.strip_prefix("mock://").unwrap_or(url);
            let net = self.with_network_mut()?;
            net.check_connection_limit()?;
            let session_id = net.create_session(SessionType::Ws, target, Scheme::Mock);
            Ok(serde_json::json!({
                "sessionId": session_id,
                "status": "connected"
            }))
        } else if url.starts_with("net://") {
            Err(VnetError::ConnectionFailed(
                "net:// WebSocket not yet implemented".to_string(),
            ))
        } else {
            Err(VnetError::InvalidParams(format!(
                "unsupported URL scheme: {url} (use mock:// or net://)"
            )))
        }
    }

    // -- net/wsMessage --------------------------------------------------------

    fn handle_ws_message(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: WsMessageParams = parse_params(params)?;
        let net = self.with_network_mut()?;

        // Verify session exists
        net.get_session(&p.session_id)?;

        let data_len = p.data.len() as u64;
        net.sessions.record_activity(&p.session_id, data_len, 0);

        Ok(serde_json::json!({ "ok": true }))
    }

    // -- net/wsClose ----------------------------------------------------------

    fn handle_ws_close(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: SessionIdParam = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.close_session(&p.session_id)?;
        Ok(serde_json::json!({ "ok": true }))
    }

    // -- net/tcpConnect -------------------------------------------------------

    fn handle_tcp_connect(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: TcpConnectParams = parse_params(params)?;
        let target = format!("{}:{}", p.host, p.port);
        let net = self.with_network_mut()?;
        net.check_connection_limit()?;
        // Treat all TCP connections as mock for now (net:// TCP not implemented)
        let session_id = net.create_session(SessionType::Tcp, &target, Scheme::Mock);
        Ok(serde_json::json!({
            "sessionId": session_id,
            "status": "connected"
        }))
    }

    // -- net/tcpSend ----------------------------------------------------------

    fn handle_tcp_send(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: TcpSendParams = parse_params(params)?;
        let net = self.with_network_mut()?;

        // Verify session exists
        net.get_session(&p.session_id)?;

        let bytes_written = p.data.len() as u64;
        net.sessions
            .record_activity(&p.session_id, bytes_written, 0);

        Ok(serde_json::json!({ "bytesWritten": bytes_written }))
    }

    // -- net/tcpClose ---------------------------------------------------------

    fn handle_tcp_close(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: SessionIdParam = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.close_session(&p.session_id)?;
        Ok(serde_json::json!({ "ok": true }))
    }

    // -- net/udpSend ----------------------------------------------------------

    fn handle_udp_send(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: UdpSendParams = parse_params(params)?;
        let mock_url = format!("mock://{}:{}", p.host, p.port);
        let net = self.with_network_mut()?;
        net.total_requests += 1;

        match net.mock_store_find_match(&mock_url, Some("udp")) {
            Some(response) => {
                let bytes_received = response.body.len() as u64;
                Ok(serde_json::json!({
                    "response": response.body,
                    "bytesReceived": bytes_received
                }))
            }
            // UDP is connectionless fire-and-forget — no mock match returns null
            // response (unlike HTTP which errors). Design spec: response is optional.
            None => Ok(serde_json::json!({
                "response": null,
                "bytesReceived": 0
            })),
        }
    }

    // -- net/resolve ----------------------------------------------------------

    fn handle_resolve(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: ResolveParams = parse_params(params)?;
        let mock_url = format!("mock://dns/{}", p.hostname);
        let net = self.with_network_mut()?;

        match net.mock_store_find_match(&mock_url, Some("dns")) {
            Some(response) => {
                let addresses: Vec<String> = serde_json::from_str(&response.body)
                    .unwrap_or_default();
                Ok(serde_json::to_value(ResolveResult {
                    addresses,
                    ttl: response.delay_ms,
                })?)
            }
            None => Err(VnetError::DnsResolutionFailed(p.hostname)),
        }
    }

    // -- net/metrics ----------------------------------------------------------

    fn handle_metrics(
        &self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network()?;
        Ok(serde_json::to_value(net.metrics())?)
    }

    // -- mock/register --------------------------------------------------------

    fn handle_mock_register(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
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

    // -- mock/unregister ------------------------------------------------------

    fn handle_mock_unregister(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: MockIdParam = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.unregister_mock(&p.id)?;
        Ok(serde_json::json!({ "ok": true }))
    }

    // -- mock/list ------------------------------------------------------------

    fn handle_mock_list(
        &self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network()?;
        let mocks = net.list_mocks();
        Ok(serde_json::to_value(mocks)?)
    }

    // -- mock/clear -----------------------------------------------------------

    fn handle_mock_clear(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network_mut()?;
        net.clear_mocks();
        Ok(serde_json::json!({ "ok": true }))
    }

    // -- mock/history ---------------------------------------------------------

    fn handle_mock_history(
        &self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network()?;
        let history = net.mock_history();
        Ok(serde_json::to_value(history)?)
    }

    // -- session/list ---------------------------------------------------------

    fn handle_session_list(
        &self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let net = self.with_network()?;
        let sessions = net.list_sessions();
        Ok(serde_json::to_value(sessions)?)
    }

    // -- session/get ----------------------------------------------------------

    fn handle_session_get(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: SessionIdParam = parse_params(params)?;
        let net = self.with_network()?;
        let info = net.get_session(&p.session_id)?;
        Ok(serde_json::to_value(info)?)
    }

    // -- session/close --------------------------------------------------------

    fn handle_session_close(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, VnetError> {
        let p: SessionIdParam = parse_params(params)?;
        let net = self.with_network_mut()?;
        net.close_session(&p.session_id)?;
        Ok(serde_json::json!({ "ok": true }))
    }
}

// -- Free-standing helpers ----------------------------------------------------

fn parse_params<T: serde::de::DeserializeOwned>(
    params: serde_json::Value,
) -> Result<T, VnetError> {
    serde_json::from_value(params).map_err(|e| VnetError::InvalidParams(e.to_string()))
}
