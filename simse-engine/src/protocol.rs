use serde::{Deserialize, Serialize};

// ── JSON-RPC 2.0 error codes ──────────────────────────────────────────────

/// Invalid JSON was received by the server.
pub const PARSE_ERROR: i32 = -32700;
/// The JSON sent is not a valid Request object.
pub const INVALID_REQUEST: i32 = -32600;
/// The method does not exist or is not available.
pub const METHOD_NOT_FOUND: i32 = -32601;
/// Invalid method parameter(s).
pub const INVALID_PARAMS: i32 = -32602;
/// Internal JSON-RPC error.
pub const INTERNAL_ERROR: i32 = -32603;

// ── JSON-RPC 2.0 framing ──────────────────────────────────────────────────

/// Incoming JSON-RPC message — may be a request (has id+method) or a response.
/// We only receive requests from the client, but the serde structure handles both.
#[derive(Debug, Deserialize)]
pub struct JsonRpcIncoming {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub method: Option<String>,
    pub params: Option<serde_json::Value>,
}

/// Outgoing JSON-RPC 2.0 response (success or error).
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Outgoing JSON-RPC 2.0 notification (no response expected).
#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// ── ACP protocol types (camelCase on the wire) ────────────────────────────

/// ACP `initialize` response payload.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpInitializeResult {
    pub protocol_version: u32,
    pub agent_info: AcpAgentInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_capabilities: Option<serde_json::Value>,
}

/// Agent identity (name and version) in ACP responses.
#[derive(Debug, Serialize)]
pub struct AcpAgentInfo {
    pub name: String,
    pub version: String,
}

/// ACP `session/new` response payload.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionNewResult {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<AcpModelsInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes: Option<AcpModesInfo>,
}

/// Available models and current selection.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpModelsInfo {
    pub available_models: Vec<AcpModelInfo>,
    pub current_model_id: String,
}

/// Descriptor for a single available model.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpModelInfo {
    pub model_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Available modes and current selection.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpModesInfo {
    pub current_mode_id: String,
    pub available_modes: Vec<AcpModeInfo>,
}

/// Descriptor for a single available mode.
#[derive(Debug, Serialize)]
pub struct AcpModeInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ── Content blocks ────────────────────────────────────────────────────────

/// Tagged union of ACP content block types (text, data, resource_link, resource).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AcpContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "data")]
    Data {
        data: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none", rename = "mimeType")]
        mime_type: Option<String>,
    },

    #[serde(rename = "resource_link")]
    ResourceLink {
        uri: String,
        name: String,
    },

    #[serde(rename = "resource")]
    Resource {
        resource: serde_json::Value,
    },
}

// ── session/prompt request params ─────────────────────────────────────────

/// Parameters for the `session/prompt` request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionPromptParams {
    pub session_id: String,
    pub prompt: Vec<AcpContentBlock>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

// ── session/prompt response ───────────────────────────────────────────────

/// Response payload for `session/prompt`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionPromptResult {
    pub content: Vec<AcpContentBlock>,
    pub stop_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ── session/update notifications (streaming) ──────────────────────────────

/// Notification params for streaming `session/update` messages.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionUpdateParams {
    pub session_id: String,
    pub update: AcpSessionUpdate,
}

/// Body of a streaming session update notification.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionUpdate {
    pub session_update: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<AcpContentBlock>>,
}

// ── session/set_config_option params ──────────────────────────────────────

/// Parameters for the `session/set_config_option` request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSetConfigParams {
    #[allow(dead_code)]
    pub session_id: String,
    pub config_option_id: String,
    pub group_id: String,
}

// ── Token usage ───────────────────────────────────────────────────────────

/// Token usage statistics for a generation or embedding request.
#[derive(Debug, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl TokenUsage {
    /// Create a new `TokenUsage` (total is computed automatically).
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}
