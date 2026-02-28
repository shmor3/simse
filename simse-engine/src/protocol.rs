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

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── TokenUsage::new() ────────────────────────────────────────────────

    #[test]
    fn token_usage_computes_total() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn token_usage_zero_values() {
        let usage = TokenUsage::new(0, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn token_usage_large_values() {
        let usage = TokenUsage::new(1_000_000, 500_000);
        assert_eq!(usage.total_tokens, 1_500_000);
    }

    #[test]
    fn token_usage_only_prompt() {
        let usage = TokenUsage::new(42, 0);
        assert_eq!(usage.total_tokens, 42);
    }

    #[test]
    fn token_usage_only_completion() {
        let usage = TokenUsage::new(0, 77);
        assert_eq!(usage.total_tokens, 77);
    }

    // ── AcpContentBlock serialization roundtrip ──────────────────────────

    #[test]
    fn content_block_text_roundtrip() {
        let block = AcpContentBlock::Text {
            text: "Hello, world!".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: AcpContentBlock = serde_json::from_str(&json).unwrap();
        match deserialized {
            AcpContentBlock::Text { text } => assert_eq!(text, "Hello, world!"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn content_block_data_roundtrip() {
        let block = AcpContentBlock::Data {
            data: serde_json::json!({"key": "value", "num": 42}),
            mime_type: Some("application/json".to_string()),
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: AcpContentBlock = serde_json::from_str(&json).unwrap();
        match deserialized {
            AcpContentBlock::Data { data, mime_type } => {
                assert_eq!(data["key"], "value");
                assert_eq!(data["num"], 42);
                assert_eq!(mime_type, Some("application/json".to_string()));
            }
            _ => panic!("Expected Data variant"),
        }
    }

    #[test]
    fn content_block_data_without_mime_type() {
        let block = AcpContentBlock::Data {
            data: serde_json::json!({"texts": ["a", "b"]}),
            mime_type: None,
        };
        let json = serde_json::to_string(&block).unwrap();
        // mime_type should be skipped in serialization
        assert!(!json.contains("mimeType"));
        let deserialized: AcpContentBlock = serde_json::from_str(&json).unwrap();
        match deserialized {
            AcpContentBlock::Data { mime_type, .. } => assert!(mime_type.is_none()),
            _ => panic!("Expected Data variant"),
        }
    }

    #[test]
    fn content_block_resource_link_roundtrip() {
        let block = AcpContentBlock::ResourceLink {
            uri: "file:///path/to/file.txt".to_string(),
            name: "file.txt".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: AcpContentBlock = serde_json::from_str(&json).unwrap();
        match deserialized {
            AcpContentBlock::ResourceLink { uri, name } => {
                assert_eq!(uri, "file:///path/to/file.txt");
                assert_eq!(name, "file.txt");
            }
            _ => panic!("Expected ResourceLink variant"),
        }
    }

    #[test]
    fn content_block_resource_roundtrip() {
        let block = AcpContentBlock::Resource {
            resource: serde_json::json!({"content": "data"}),
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: AcpContentBlock = serde_json::from_str(&json).unwrap();
        match deserialized {
            AcpContentBlock::Resource { resource } => {
                assert_eq!(resource["content"], "data");
            }
            _ => panic!("Expected Resource variant"),
        }
    }

    // ── JSON-RPC types serialization ─────────────────────────────────────

    #[test]
    fn json_rpc_response_success_serialization() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0",
            id: 1,
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"status\":\"ok\""));
        // error should be omitted
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn json_rpc_response_error_serialization() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0",
            id: 2,
            result: None,
            error: Some(JsonRpcError {
                code: METHOD_NOT_FOUND,
                message: "Not found".to_string(),
                data: None,
            }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"code\":-32601"));
        assert!(json.contains("\"message\":\"Not found\""));
        // result should be omitted
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn json_rpc_incoming_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"initialize","params":null}"#;
        let msg: JsonRpcIncoming = serde_json::from_str(json).unwrap();
        assert_eq!(msg.id, Some(42));
        assert_eq!(msg.method, Some("initialize".to_string()));
    }

    #[test]
    fn json_rpc_incoming_notification() {
        let json = r#"{"jsonrpc":"2.0","method":"session/update"}"#;
        let msg: JsonRpcIncoming = serde_json::from_str(json).unwrap();
        assert!(msg.id.is_none());
        assert_eq!(msg.method, Some("session/update".to_string()));
    }

    // ── Error code constants ─────────────────────────────────────────────

    #[test]
    fn error_codes_have_correct_values() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
    }

    // ── ACP types serialization ──────────────────────────────────────────

    #[test]
    fn acp_initialize_result_uses_camel_case() {
        let result = AcpInitializeResult {
            protocol_version: 1,
            agent_info: AcpAgentInfo {
                name: "test".to_string(),
                version: "1.0".to_string(),
            },
            agent_capabilities: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"protocolVersion\":1"));
        assert!(json.contains("\"agentInfo\""));
        // agentCapabilities should be omitted when None
        assert!(!json.contains("agentCapabilities"));
    }

    #[test]
    fn acp_session_new_result_uses_camel_case() {
        let result = AcpSessionNewResult {
            session_id: "abc-123".to_string(),
            models: None,
            modes: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"sessionId\":\"abc-123\""));
        // Optional fields omitted
        assert!(!json.contains("\"models\""));
        assert!(!json.contains("\"modes\""));
    }

    #[test]
    fn acp_session_prompt_params_deserializes() {
        let json = r#"{
            "sessionId": "sess-1",
            "prompt": [{"type": "text", "text": "Hello"}],
            "metadata": {"temperature": 0.5}
        }"#;
        let params: AcpSessionPromptParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.session_id, "sess-1");
        assert_eq!(params.prompt.len(), 1);
        assert!(params.metadata.is_some());
    }

    #[test]
    fn acp_session_prompt_params_without_metadata() {
        let json = r#"{
            "sessionId": "sess-2",
            "prompt": [{"type": "text", "text": "Hi"}]
        }"#;
        let params: AcpSessionPromptParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.session_id, "sess-2");
        assert!(params.metadata.is_none());
    }

    #[test]
    fn token_usage_serializes() {
        let usage = TokenUsage::new(10, 20);
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"prompt_tokens\":10"));
        assert!(json.contains("\"completion_tokens\":20"));
        assert!(json.contains("\"total_tokens\":30"));
    }
}
