// ---------------------------------------------------------------------------
// RemoteServer — JSON-RPC dispatcher (functional programming patterns)
// ---------------------------------------------------------------------------
//
// Routes incoming JSON-RPC 2.0 requests (NDJSON over stdin) to auth/tunnel
// handlers. Uses immutable state with owned-return transitions:
//
//   - `with_state`: read-only access to state (no mutation)
//   - `with_state_transition`: takes owned state via Option::take(), runs
//     handler, returns updated state
//
// State (`RemoteServerState`) holds the AuthClient and is managed via
// owned-return transitions. The TunnelClient uses Arc<Mutex<>> internally
// and is kept as an I/O handle in the server (not in the state struct).
// ---------------------------------------------------------------------------

use std::io::{self, BufRead};

use crate::auth::AuthClient;
use crate::error::RemoteError;
use crate::protocol::*;
use crate::tunnel::TunnelClient;
use crate::transport::NdjsonTransport;

// ---------------------------------------------------------------------------
// State — immutable with owned-return transitions
// ---------------------------------------------------------------------------

/// Pure server state managed via owned-return transitions.
///
/// Contains the `AuthClient` which holds authentication state.
/// The `TunnelClient` is excluded because it uses internal `Arc<Mutex<>>`
/// for async I/O and is not pure state.
#[derive(Clone)]
pub struct RemoteServerState {
    pub auth: AuthClient,
}

impl RemoteServerState {
    pub fn new() -> Self {
        Self {
            auth: AuthClient::new(),
        }
    }
}

impl Default for RemoteServerState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Remote JSON-RPC server — dispatches incoming requests.
///
/// State is held in an `Option<RemoteServerState>` and accessed via:
/// - `with_state`: read-only access (borrows the state)
/// - `with_state_transition`: owned access (takes state, returns new state)
///
/// The `TunnelClient` is an I/O handle with internal `Arc<Mutex<>>` and
/// stays outside of the pure state struct.
pub struct RemoteServer {
    transport: NdjsonTransport,
    state: Option<RemoteServerState>,
    tunnel: TunnelClient,
}

impl RemoteServer {
    pub fn new(transport: NdjsonTransport) -> Self {
        Self {
            transport,
            state: Some(RemoteServerState::new()),
            tunnel: TunnelClient::new(),
        }
    }

    // ── State access helpers ─────────────────────────────────────────

    /// Read-only access to state. Calls `f` with a reference to the
    /// current state.
    fn with_state<T>(&self, f: impl FnOnce(&RemoteServerState) -> T) -> T {
        f(self.state.as_ref().expect("state invariant: always Some"))
    }

    /// Mutating access via owned-return pattern. Takes the state out of
    /// the `Option`, passes ownership to `f`, and stores the returned state.
    ///
    /// `f` receives owned state and must return the (possibly modified)
    /// state.
    async fn with_state_transition<F, Fut>(&mut self, f: F)
    where
        F: FnOnce(RemoteServerState, NdjsonTransport) -> Fut,
        Fut: std::future::Future<Output = RemoteServerState>,
    {
        let state = self.state.take().expect("state invariant: always Some");
        let new_state = f(state, self.transport.clone()).await;
        self.state = Some(new_state);
    }

    // ── Main loop ────────────────────────────────────────────────────

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

    // ── Dispatch ─────────────────────────────────────────────────────

    async fn dispatch(&mut self, req: JsonRpcRequest) {
        match req.method.as_str() {
            // -- State transitions (owned-return via with_state_transition) --
            "auth/login" => {
                let tunnel = &self.tunnel;
                let _ = tunnel; // used only for type clarity
                self.with_state_transition(|state, transport| {
                    handle_login(state, transport, req)
                })
                .await;
            }
            "auth/logout" => {
                self.with_state_transition(|state, transport| async move {
                    handle_logout(state, &transport, req)
                })
                .await;
            }

            // -- Read-only handlers (with_state) --
            "auth/status" => {
                let transport = self.transport.clone();
                self.with_state(|state| handle_auth_status(state, &transport, req));
            }

            // -- Tunnel handlers (async I/O, read state for auth check) --
            "tunnel/connect" => {
                let transport = self.transport.clone();
                let auth_state = self.with_state(|s| s.auth.state().cloned());
                // PERF: async I/O — WebSocket connection
                handle_tunnel_connect(&self.tunnel, auth_state, &transport, req).await;
            }
            "tunnel/disconnect" => {
                let transport = self.transport.clone();
                // PERF: async I/O — WebSocket close
                handle_tunnel_disconnect(&self.tunnel, &transport, req).await;
            }
            "tunnel/status" => {
                let transport = self.transport.clone();
                // PERF: async I/O — reads tunnel state behind Arc<Mutex<>>
                handle_tunnel_status(&self.tunnel, &transport, req).await;
            }

            // -- Health (read-only, async for tunnel state) --
            "remote/health" => {
                let transport = self.transport.clone();
                let authenticated = self.with_state(|s| s.auth.is_authenticated());
                handle_health(&self.tunnel, authenticated, &transport, req).await;
            }

            _ => {
                self.transport.write_error(
                    req.id,
                    METHOD_NOT_FOUND,
                    format!("Unknown method: {}", req.method),
                    None,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_result(
    transport: &NdjsonTransport,
    id: u64,
    result: Result<serde_json::Value, RemoteError>,
) {
    match result {
        Ok(value) => transport.write_response(id, value),
        Err(e) => transport.write_error(
            id,
            REMOTE_ERROR,
            e.to_string(),
            Some(e.to_json_rpc_error()),
        ),
    }
}

// ---------------------------------------------------------------------------
// Auth handlers — state transitions (owned state)
// ---------------------------------------------------------------------------

async fn handle_login(
    state: RemoteServerState,
    transport: NdjsonTransport,
    req: JsonRpcRequest,
) -> RemoteServerState {
    let p: LoginParams = match parse_params(req.params) {
        Ok(p) => p,
        Err(e) => {
            write_result(&transport, req.id, Err(e));
            return state;
        }
    };

    let api_url = p.api_url.as_deref().unwrap_or("https://api.simse.dev");

    if let Some(api_key) = p.api_key {
        // PERF: async I/O — HTTP request to auth service
        match state.auth.login_api_key(api_url, &api_key).await {
            Ok((auth, auth_state)) => {
                transport.write_response(
                    req.id,
                    serde_json::to_value(LoginResult {
                        user_id: auth_state.user_id,
                        session_token: auth_state.session_token,
                        team_id: auth_state.team_id,
                        role: auth_state.role,
                    })
                    .unwrap_or_default(),
                );
                RemoteServerState { auth }
            }
            Err((auth, e)) => {
                write_result(&transport, req.id, Err(e));
                RemoteServerState { auth }
            }
        }
    } else {
        let email = match p.email {
            Some(e) => e,
            None => {
                write_result(
                    &transport,
                    req.id,
                    Err(RemoteError::InvalidParams(
                        "email or apiKey required".into(),
                    )),
                );
                return state;
            }
        };
        let password = match p.password {
            Some(p) => p,
            None => {
                write_result(
                    &transport,
                    req.id,
                    Err(RemoteError::InvalidParams("password required".into())),
                );
                return state;
            }
        };

        // PERF: async I/O — HTTP request to auth service
        match state.auth.login_password(api_url, &email, &password).await {
            Ok((auth, auth_state)) => {
                transport.write_response(
                    req.id,
                    serde_json::to_value(LoginResult {
                        user_id: auth_state.user_id,
                        session_token: auth_state.session_token,
                        team_id: auth_state.team_id,
                        role: auth_state.role,
                    })
                    .unwrap_or_default(),
                );
                RemoteServerState { auth }
            }
            Err((auth, e)) => {
                write_result(&transport, req.id, Err(e));
                RemoteServerState { auth }
            }
        }
    }
}

fn handle_logout(
    state: RemoteServerState,
    transport: &NdjsonTransport,
    req: JsonRpcRequest,
) -> RemoteServerState {
    let auth = state.auth.logout();
    transport.write_response(req.id, serde_json::json!({ "ok": true }));
    RemoteServerState { auth }
}

// ---------------------------------------------------------------------------
// Auth handlers — read-only (with_state)
// ---------------------------------------------------------------------------

fn handle_auth_status(
    state: &RemoteServerState,
    transport: &NdjsonTransport,
    req: JsonRpcRequest,
) {
    let auth_state = state.auth.state();
    let result = serde_json::to_value(AuthStatusResult {
        authenticated: auth_state.is_some(),
        user_id: auth_state.map(|s| s.user_id.clone()),
        team_id: auth_state.and_then(|s| s.team_id.clone()),
        role: auth_state.and_then(|s| s.role.clone()),
        api_url: auth_state.map(|s| s.api_url.clone()),
    });
    write_result(transport, req.id, result.map_err(RemoteError::Json));
}

// ---------------------------------------------------------------------------
// Tunnel handlers — async I/O (operate on TunnelClient references)
// ---------------------------------------------------------------------------

async fn handle_tunnel_connect(
    tunnel: &TunnelClient,
    auth_state: Option<crate::auth::AuthState>,
    transport: &NdjsonTransport,
    req: JsonRpcRequest,
) {
    let auth_state = match auth_state {
        Some(s) => s,
        None => {
            write_result(transport, req.id, Err(RemoteError::NotAuthenticated));
            return;
        }
    };

    let p: TunnelConnectParams = match parse_params(req.params) {
        Ok(p) => p,
        Err(e) => {
            write_result(transport, req.id, Err(e));
            return;
        }
    };

    let relay_url = p
        .relay_url
        .as_deref()
        .unwrap_or("wss://relay.simse.dev");

    // PERF: async I/O — WebSocket connection to relay
    match tunnel.connect(relay_url, &auth_state.session_token).await {
        Ok(tunnel_id) => {
            write_result(
                transport,
                req.id,
                serde_json::to_value(TunnelConnectResult {
                    tunnel_id,
                    relay_url: relay_url.to_string(),
                })
                .map_err(RemoteError::Json),
            );
        }
        Err(e) => {
            write_result(transport, req.id, Err(e));
        }
    }
}

async fn handle_tunnel_disconnect(
    tunnel: &TunnelClient,
    transport: &NdjsonTransport,
    req: JsonRpcRequest,
) {
    // PERF: async I/O — WebSocket close
    match tunnel.disconnect().await {
        Ok(()) => {
            transport.write_response(req.id, serde_json::json!({ "ok": true }));
        }
        Err(e) => {
            write_result(transport, req.id, Err(e));
        }
    }
}

async fn handle_tunnel_status(
    tunnel: &TunnelClient,
    transport: &NdjsonTransport,
    req: JsonRpcRequest,
) {
    // PERF: async I/O — reads tunnel state behind Arc<Mutex<>>
    let state = tunnel.get_state().await;
    let uptime_ms = state
        .connected_at
        .map(|t| t.elapsed().as_millis() as u64);

    write_result(
        transport,
        req.id,
        serde_json::to_value(TunnelStatusResult {
            connected: state.connected,
            tunnel_id: state.tunnel_id,
            relay_url: state.relay_url,
            uptime_ms,
            reconnect_count: state.reconnect_count,
        })
        .map_err(RemoteError::Json),
    );
}

// ---------------------------------------------------------------------------
// Health handler
// ---------------------------------------------------------------------------

async fn handle_health(
    tunnel: &TunnelClient,
    authenticated: bool,
    transport: &NdjsonTransport,
    req: JsonRpcRequest,
) {
    write_result(
        transport,
        req.id,
        serde_json::to_value(HealthResult {
            ok: true,
            authenticated,
            tunnel_connected: tunnel.is_connected(),
        })
        .map_err(RemoteError::Json),
    );
}
