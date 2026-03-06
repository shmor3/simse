use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 error codes
// ---------------------------------------------------------------------------

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const ACP_ERROR: i32 = -32000;

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 framing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
	pub id: u64,
	pub method: String,
	#[serde(default)]
	pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
	pub jsonrpc: &'static str,
	pub id: u64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub result: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
	pub fn success(id: u64, result: serde_json::Value) -> Self {
		Self {
			jsonrpc: "2.0",
			id,
			result: Some(result),
			error: None,
		}
	}

	pub fn error(id: u64, code: i32, message: impl Into<String>) -> Self {
		Self {
			jsonrpc: "2.0",
			id,
			result: None,
			error: Some(JsonRpcError {
				code,
				message: message.into(),
				data: None,
			}),
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
	pub code: i32,
	pub message: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
	pub jsonrpc: &'static str,
	pub method: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
	pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
		Self {
			jsonrpc: "2.0",
			method: method.into(),
			params,
		}
	}
}

// ---------------------------------------------------------------------------
// ACP protocol — initialize
// ---------------------------------------------------------------------------

/// Client info sent during the initialize handshake.
///
/// Wire format uses `client_info` (snake_case) at the top level of
/// `InitializeParams`, but the inner fields are plain camelCase-free.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
	pub name: String,
	pub version: String,
}

/// Agent (server) info returned from initialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
	pub name: String,
	pub version: String,
}

/// Parameters for the `initialize` request.
///
/// NOTE: The ACP wire format mixes snake_case (`client_info`) and
/// camelCase (`protocolVersion`) in this particular message. We use
/// explicit `#[serde(rename)]` for the mixed fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
	pub protocol_version: u32,
	#[serde(rename = "client_info")]
	pub client_info: ClientInfo,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub capabilities: Option<serde_json::Value>,
}

/// Capabilities advertised by the agent during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub load_session: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub prompt_capabilities: Option<serde_json::Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub session_capabilities: Option<serde_json::Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mcp_capabilities: Option<serde_json::Value>,
}

/// Result of the `initialize` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
	pub protocol_version: u32,
	pub agent_info: AgentInfo,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_capabilities: Option<AgentCapabilities>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub auth_methods: Option<Vec<serde_json::Value>>,
}

// ---------------------------------------------------------------------------
// ACP protocol — client capabilities
// ---------------------------------------------------------------------------

/// Client capabilities sent during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub permissions: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub streaming: Option<bool>,
}

// ---------------------------------------------------------------------------
// ACP protocol — sessions
// ---------------------------------------------------------------------------

/// Parameters for `session/new`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewParams {
	pub cwd: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mcp_servers: Option<Vec<serde_json::Value>>,
}

/// Model information returned in session/new response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
	pub model_id: String,
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
}

/// Collection of available models and the current selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsInfo {
	pub available_models: Vec<ModelInfo>,
	pub current_model_id: String,
}

/// Mode information returned in session/new response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeInfo {
	pub id: String,
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
}

/// Collection of available modes and the current selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModesInfo {
	pub current_mode_id: String,
	pub available_modes: Vec<ModeInfo>,
}

/// Session info returned from `session/new`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
	pub session_id: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub models: Option<ModelsInfo>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub modes: Option<ModesInfo>,
}

/// Entry in the session list (`session/list` response).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListEntry {
	pub session_id: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub created_at: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub last_active_at: Option<String>,
}

// ---------------------------------------------------------------------------
// ACP protocol — content blocks
// ---------------------------------------------------------------------------

/// Resource embedded in a content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceData {
	pub uri: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mime_type: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub text: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub blob: Option<String>,
}

/// Tagged content block union — discriminated by the `type` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
	/// Plain text content.
	Text {
		text: String,
	},
	/// A reference/link to a resource.
	ResourceLink {
		uri: String,
		name: String,
		#[serde(default, skip_serializing_if = "Option::is_none", rename = "mimeType")]
		mime_type: Option<String>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		title: Option<String>,
		#[serde(default, skip_serializing_if = "Option::is_none")]
		description: Option<String>,
	},
	/// An embedded resource with optional text/blob content.
	Resource {
		resource: ResourceData,
	},
	/// Non-standard data block (deprecated).
	Data {
		data: serde_json::Value,
		#[serde(default, skip_serializing_if = "Option::is_none", rename = "mimeType")]
		mime_type: Option<String>,
	},
}

// ---------------------------------------------------------------------------
// ACP protocol — prompt (generation)
// ---------------------------------------------------------------------------

/// Stop reason returned by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
	EndTurn,
	MaxTokens,
	MaxTurnRequests,
	Refusal,
	Cancelled,
	StopSequence,
	ToolUse,
}

/// Parameters for `session/prompt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams {
	pub session_id: String,
	pub prompt: Vec<ContentBlock>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub metadata: Option<serde_json::Value>,
}

/// Result of `session/prompt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptResult {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub content: Option<Vec<ContentBlock>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub stop_reason: Option<StopReason>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// ACP protocol — session update notifications
// ---------------------------------------------------------------------------

/// Payload within a session/update notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdate {
	pub session_update: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub content: Option<serde_json::Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub metadata: Option<serde_json::Value>,
	/// Extra fields that don't map to known struct fields.
	#[serde(flatten)]
	pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Parameters for `session/update` notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdateParams {
	pub session_id: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub update: Option<SessionUpdate>,
}

// ---------------------------------------------------------------------------
// ACP protocol — permission requests
// ---------------------------------------------------------------------------

/// Tool call details attached to a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionToolCall {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tool_call_id: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub kind: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub raw_input: Option<serde_json::Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub status: Option<String>,
}

/// An option presented in a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
	pub option_id: String,
	pub kind: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,
	/// Deprecated: use `name` instead. Kept for backward compatibility.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
}

/// Parameters for `session/request_permission`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestParams {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tool_call: Option<PermissionToolCall>,
	#[serde(default)]
	pub options: Vec<PermissionOption>,
}

/// Outcome of a permission decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOutcome {
	/// Either "selected" or "cancelled".
	pub outcome: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub option_id: Option<String>,
}

/// Result sent back to the agent for a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResult {
	pub outcome: PermissionOutcome,
}

/// Permission policy for tool use requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum PermissionPolicy {
	AutoApprove,
	#[default]
 Prompt,
	Deny,
}


// ---------------------------------------------------------------------------
// ACP protocol — tool calls
// ---------------------------------------------------------------------------

/// Kind of tool call action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallKind {
	Read,
	Edit,
	Delete,
	Move,
	Search,
	Execute,
	Think,
	Fetch,
	Other,
}

/// Status of a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
	Pending,
	InProgress,
	Completed,
	Failed,
	Cancelled,
}

/// A tool call from a session/update notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
	pub tool_call_id: String,
	pub title: String,
	pub kind: ToolCallKind,
	pub status: ToolCallStatus,
}

/// A tool call progress update from a session/update notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallUpdate {
	pub tool_call_id: String,
	pub status: ToolCallStatus,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub content: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// ACP protocol — token usage
// ---------------------------------------------------------------------------

/// Token usage tracking.
///
/// Accepts both camelCase (`promptTokens`) and snake_case
/// (`prompt_tokens`) for deserialization — different ACP servers may
/// use either convention.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
	#[serde(alias = "prompt_tokens")]
	pub prompt_tokens: u64,
	#[serde(alias = "completion_tokens")]
	pub completion_tokens: u64,
	#[serde(alias = "total_tokens")]
	pub total_tokens: u64,
}

// ---------------------------------------------------------------------------
// ACP protocol — sampling parameters
// ---------------------------------------------------------------------------

/// Sampling parameters for generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingParams {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub temperature: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub max_tokens: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_p: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_k: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub stop_sequences: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// ACP protocol — config options
// ---------------------------------------------------------------------------

/// Parameters for `session/set_config_option`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetConfigOptionParams {
	pub session_id: String,
	pub config_option_id: String,
	pub group_id: String,
}

// ---------------------------------------------------------------------------
// ACP protocol — streaming chunk types
// ---------------------------------------------------------------------------

/// An incremental text delta from a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDelta {
	pub text: String,
}

/// Final event emitted when a stream completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamComplete {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub usage: Option<TokenUsage>,
}

/// Discriminated union of streaming chunk types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
	/// Incremental text delta.
	Delta {
		text: String,
	},
	/// Stream completed with optional usage.
	Complete {
		#[serde(default, skip_serializing_if = "Option::is_none")]
		usage: Option<TokenUsage>,
	},
	/// A new tool call started.
	ToolCall {
		#[serde(rename = "toolCall")]
		tool_call: ToolCall,
	},
	/// Progress update on an existing tool call.
	ToolCallUpdate {
		update: ToolCallUpdate,
	},
}

// ---------------------------------------------------------------------------
// ACP protocol — agent info (synthetic, from config)
// ---------------------------------------------------------------------------

/// Agent info derived from configuration (not from the wire protocol).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfoEntry {
	pub id: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// ACP protocol — server status (multi-ACP discovery)
// ---------------------------------------------------------------------------

/// Status of a connected ACP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
	pub name: String,
	pub connected: bool,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub current_model: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub available_models: Option<Vec<ModelInfo>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_content_block_text_roundtrip() {
		let block = ContentBlock::Text {
			text: "hello".into(),
		};
		let json = serde_json::to_string(&block).unwrap();
		assert!(json.contains(r#""type":"text""#));
		assert!(json.contains(r#""text":"hello""#));

		let decoded: ContentBlock = serde_json::from_str(&json).unwrap();
		match decoded {
			ContentBlock::Text { text } => assert_eq!(text, "hello"),
			_ => panic!("expected Text variant"),
		}
	}

	#[test]
	fn test_content_block_resource_link_roundtrip() {
		let json = r#"{"type":"resource_link","uri":"file:///a.txt","name":"a.txt"}"#;
		let block: ContentBlock = serde_json::from_str(json).unwrap();
		match block {
			ContentBlock::ResourceLink { uri, name, .. } => {
				assert_eq!(uri, "file:///a.txt");
				assert_eq!(name, "a.txt");
			}
			_ => panic!("expected ResourceLink variant"),
		}
	}

	#[test]
	fn test_content_block_resource_roundtrip() {
		let json =
			r#"{"type":"resource","resource":{"uri":"file:///b.txt","text":"contents"}}"#;
		let block: ContentBlock = serde_json::from_str(json).unwrap();
		match block {
			ContentBlock::Resource { resource } => {
				assert_eq!(resource.uri, "file:///b.txt");
				assert_eq!(resource.text.as_deref(), Some("contents"));
			}
			_ => panic!("expected Resource variant"),
		}
	}

	#[test]
	fn test_content_block_data_roundtrip() {
		let json = r#"{"type":"data","data":{"key":"val"},"mimeType":"application/json"}"#;
		let block: ContentBlock = serde_json::from_str(json).unwrap();
		match block {
			ContentBlock::Data { data, mime_type } => {
				assert_eq!(data["key"], "val");
				assert_eq!(mime_type.as_deref(), Some("application/json"));
			}
			_ => panic!("expected Data variant"),
		}
	}

	#[test]
	fn test_stop_reason_serde() {
		let json = r#""end_turn""#;
		let reason: StopReason = serde_json::from_str(json).unwrap();
		assert_eq!(reason, StopReason::EndTurn);

		let json = r#""tool_use""#;
		let reason: StopReason = serde_json::from_str(json).unwrap();
		assert_eq!(reason, StopReason::ToolUse);

		let serialized = serde_json::to_string(&StopReason::MaxTokens).unwrap();
		assert_eq!(serialized, r#""max_tokens""#);
	}

	#[test]
	fn test_token_usage_camel_case() {
		let json = r#"{"promptTokens":10,"completionTokens":20,"totalTokens":30}"#;
		let usage: TokenUsage = serde_json::from_str(json).unwrap();
		assert_eq!(usage.prompt_tokens, 10);
		assert_eq!(usage.completion_tokens, 20);
		assert_eq!(usage.total_tokens, 30);
	}

	#[test]
	fn test_token_usage_snake_case_alias() {
		let json = r#"{"prompt_tokens":5,"completion_tokens":15,"total_tokens":20}"#;
		let usage: TokenUsage = serde_json::from_str(json).unwrap();
		assert_eq!(usage.prompt_tokens, 5);
		assert_eq!(usage.completion_tokens, 15);
		assert_eq!(usage.total_tokens, 20);
	}

	#[test]
	fn test_initialize_params_mixed_case() {
		let params = InitializeParams {
			protocol_version: 1,
			client_info: ClientInfo {
				name: "simse".into(),
				version: "1.0.0".into(),
			},
			capabilities: None,
		};
		let json = serde_json::to_string(&params).unwrap();
		// protocolVersion should be camelCase
		assert!(json.contains("protocolVersion"));
		// client_info should stay snake_case (explicit rename)
		assert!(json.contains("client_info"));

		// Round-trip
		let decoded: InitializeParams = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.protocol_version, 1);
		assert_eq!(decoded.client_info.name, "simse");
	}

	#[test]
	fn test_session_info_camel_case() {
		let json = r#"{"sessionId":"abc-123"}"#;
		let info: SessionInfo = serde_json::from_str(json).unwrap();
		assert_eq!(info.session_id, "abc-123");
		assert!(info.models.is_none());
		assert!(info.modes.is_none());
	}

	#[test]
	fn test_permission_outcome_roundtrip() {
		let result = PermissionResult {
			outcome: PermissionOutcome {
				outcome: "selected".into(),
				option_id: Some("allow_once".into()),
			},
		};
		let json = serde_json::to_string(&result).unwrap();
		assert!(json.contains(r#""outcome":"selected""#));
		assert!(json.contains("optionId"));

		let decoded: PermissionResult = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.outcome.outcome, "selected");
		assert_eq!(decoded.outcome.option_id.as_deref(), Some("allow_once"));
	}

	#[test]
	fn test_permission_policy_serde() {
		let json = r#""auto-approve""#;
		let policy: PermissionPolicy = serde_json::from_str(json).unwrap();
		assert_eq!(policy, PermissionPolicy::AutoApprove);

		let json = r#""prompt""#;
		let policy: PermissionPolicy = serde_json::from_str(json).unwrap();
		assert_eq!(policy, PermissionPolicy::Prompt);

		let serialized = serde_json::to_string(&PermissionPolicy::Deny).unwrap();
		assert_eq!(serialized, r#""deny""#);
	}

	#[test]
	fn test_tool_call_kind_serde() {
		let json = r#""in_progress""#;
		let status: ToolCallStatus = serde_json::from_str(json).unwrap();
		assert_eq!(status, ToolCallStatus::InProgress);

		let json = r#""think""#;
		let kind: ToolCallKind = serde_json::from_str(json).unwrap();
		assert_eq!(kind, ToolCallKind::Think);
	}

	#[test]
	fn test_stream_chunk_delta() {
		let json = r#"{"type":"delta","text":"hello "}"#;
		let chunk: StreamChunk = serde_json::from_str(json).unwrap();
		match chunk {
			StreamChunk::Delta { text } => assert_eq!(text, "hello "),
			_ => panic!("expected Delta variant"),
		}
	}

	#[test]
	fn test_stream_chunk_complete() {
		let json =
			r#"{"type":"complete","usage":{"promptTokens":10,"completionTokens":5,"totalTokens":15}}"#;
		let chunk: StreamChunk = serde_json::from_str(json).unwrap();
		match chunk {
			StreamChunk::Complete { usage } => {
				let u = usage.unwrap();
				assert_eq!(u.prompt_tokens, 10);
				assert_eq!(u.total_tokens, 15);
			}
			_ => panic!("expected Complete variant"),
		}
	}

	#[test]
	fn test_json_rpc_response_success() {
		let resp = JsonRpcResponse::success(42, serde_json::json!({"ok": true}));
		let json = serde_json::to_string(&resp).unwrap();
		assert!(json.contains(r#""id":42"#));
		assert!(json.contains(r#""ok":true"#));
		assert!(!json.contains("error"));
	}

	#[test]
	fn test_json_rpc_response_error() {
		let resp = JsonRpcResponse::error(1, INTERNAL_ERROR, "something broke");
		let json = serde_json::to_string(&resp).unwrap();
		assert!(json.contains(r#""code":-32603"#));
		assert!(json.contains("something broke"));
		assert!(!json.contains(r#""result""#));
	}

	#[test]
	fn test_sampling_params_roundtrip() {
		let params = SamplingParams {
			temperature: Some(0.7),
			max_tokens: Some(1024),
			top_p: None,
			top_k: None,
			stop_sequences: Some(vec!["STOP".into()]),
		};
		let json = serde_json::to_string(&params).unwrap();
		assert!(json.contains("maxTokens"));
		assert!(json.contains("stopSequences"));
		assert!(!json.contains("topP"));
		assert!(!json.contains("topK"));

		let decoded: SamplingParams = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.temperature, Some(0.7));
		assert_eq!(decoded.max_tokens, Some(1024));
		assert_eq!(decoded.stop_sequences.unwrap(), vec!["STOP"]);
	}

	#[test]
	fn test_set_config_option_params() {
		let params = SetConfigOptionParams {
			session_id: "sess-1".into(),
			config_option_id: "mode".into(),
			group_id: "fast".into(),
		};
		let json = serde_json::to_string(&params).unwrap();
		assert!(json.contains("sessionId"));
		assert!(json.contains("configOptionId"));
		assert!(json.contains("groupId"));
	}

	#[test]
	fn test_session_update_with_extras() {
		let json = r#"{"sessionUpdate":"delta","content":"hi","customField":42}"#;
		let update: SessionUpdate = serde_json::from_str(json).unwrap();
		assert_eq!(update.session_update, "delta");
		assert!(update.extra.contains_key("customField"));
		assert_eq!(update.extra["customField"], 42);
	}
}
