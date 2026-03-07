use serde::Deserialize;

// -- JSON-RPC 2.0 error codes ------------------------------------------------

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const SANDBOX_ERROR: i32 = -32000;

// -- Incoming request ---------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

// -- Initialize ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub backend: Option<BackendParams>,
    pub vfs: Option<serde_json::Value>,
    pub vsh: Option<serde_json::Value>,
    pub vnet: Option<serde_json::Value>,
}

// -- Backend ------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendParams {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub ssh: Option<SshParams>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshParams {
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub auth: SshAuthParams,
    pub max_channels: Option<usize>,
    pub keepalive_interval_ms: Option<u64>,
    /// Expected server host key fingerprint (`SHA256:<base64>`).
    /// When set, connections are rejected if the server key doesn't match.
    pub host_key_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthParams {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
    pub password: Option<String>,
}

// -- Switch backend -----------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBackendParams {
    pub backend: BackendParams,
}
