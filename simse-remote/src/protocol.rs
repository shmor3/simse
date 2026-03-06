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
