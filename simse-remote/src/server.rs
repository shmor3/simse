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
