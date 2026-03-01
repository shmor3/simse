use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 error codes
// ---------------------------------------------------------------------------

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const MCP_ERROR: i32 = -32000;

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
// MCP protocol — initialize
// ---------------------------------------------------------------------------

/// Implementation info exchanged during the initialize handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationInfo {
	pub name: String,
	pub version: String,
}

/// Root capabilities advertised by the client.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootCapabilities {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub list_changed: Option<bool>,
}

/// Capabilities advertised by the client during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub roots: Option<RootCapabilities>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub sampling: Option<serde_json::Value>,
}

/// Capabilities advertised by the server during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tools: Option<serde_json::Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub resources: Option<serde_json::Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub prompts: Option<serde_json::Value>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub logging: Option<serde_json::Value>,
}

/// Parameters for the `initialize` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeParams {
	pub protocol_version: String,
	pub capabilities: ClientCapabilities,
	pub client_info: ImplementationInfo,
}

/// Result of the `initialize` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeResult {
	pub protocol_version: String,
	pub capabilities: ServerCapabilities,
	pub server_info: ImplementationInfo,
}

// ---------------------------------------------------------------------------
// MCP protocol — tools
// ---------------------------------------------------------------------------

/// Annotations providing hints about a tool's behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub read_only_hint: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub destructive_hint: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub idempotent_hint: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub open_world_hint: Option<bool>,
}

/// Tool information returned from `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	pub input_schema: serde_json::Value,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub annotations: Option<ToolAnnotations>,
}

/// Parameters for `tools/call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallParams {
	pub name: String,
	#[serde(default)]
	pub arguments: serde_json::Value,
}

/// Result of `tools/call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
	pub content: Vec<ContentItem>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub is_error: Option<bool>,
}

// ---------------------------------------------------------------------------
// MCP protocol — content items
// ---------------------------------------------------------------------------

/// Resource content embedded in responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
	pub uri: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub text: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub blob: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mime_type: Option<String>,
}

/// Tagged content item union — discriminated by the `type` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
	/// Plain text content.
	Text {
		text: String,
	},
	/// Base64-encoded image content.
	Image {
		data: String,
		#[serde(rename = "mimeType")]
		mime_type: String,
	},
	/// An embedded resource.
	Resource {
		resource: ResourceContent,
	},
}

// ---------------------------------------------------------------------------
// MCP protocol — resources
// ---------------------------------------------------------------------------

/// Resource information returned from `resources/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
	pub uri: String,
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mime_type: Option<String>,
}

/// Resource template information returned from `resources/templates/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplateInfo {
	pub uri_template: String,
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mime_type: Option<String>,
}

/// Parameters for `resources/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceParams {
	pub uri: String,
}

/// Result of `resources/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
	pub contents: Vec<ResourceContent>,
}

// ---------------------------------------------------------------------------
// MCP protocol — prompts
// ---------------------------------------------------------------------------

/// Prompt argument definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub required: Option<bool>,
}

/// Prompt information returned from `prompts/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub arguments: Option<Vec<PromptArgument>>,
}

/// Parameters for `prompts/get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptParams {
	pub name: String,
	#[serde(default)]
	pub arguments: serde_json::Value,
}

/// A message within a prompt result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMessage {
	pub role: String,
	pub content: serde_json::Value,
}

/// Result of `prompts/get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
	pub messages: Vec<PromptMessage>,
}

// ---------------------------------------------------------------------------
// MCP protocol — logging
// ---------------------------------------------------------------------------

/// Severity levels for MCP logging messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoggingLevel {
	Debug,
	Info,
	Notice,
	Warning,
	Error,
	Critical,
	Alert,
	Emergency,
}

/// A structured logging message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggingMessage {
	pub level: LoggingLevel,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub logger: Option<String>,
	pub data: serde_json::Value,
}

// ---------------------------------------------------------------------------
// MCP protocol — completions
// ---------------------------------------------------------------------------

/// Reference for completion context — either a resource or a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CompletionRef {
	/// Reference to a resource.
	#[serde(rename = "ref/resource")]
	ResourceRef { uri: String },
	/// Reference to a prompt.
	#[serde(rename = "ref/prompt")]
	PromptRef { name: String },
}

/// Argument being completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionArg {
	pub name: String,
	pub value: String,
}

/// Result of a completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionResult {
	pub values: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub has_more: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub total: Option<u64>,
}

// ---------------------------------------------------------------------------
// MCP protocol — roots
// ---------------------------------------------------------------------------

/// A workspace root advertised to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
	pub uri: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub name: Option<String>,
}

// ---------------------------------------------------------------------------
// MCP protocol — configuration types
// ---------------------------------------------------------------------------

/// Transport type for MCP connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
	Stdio,
	Http,
}

/// Configuration for connecting to an external MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConnection {
	pub name: String,
	pub transport: TransportType,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub command: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub args: Option<Vec<String>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub env: Option<std::collections::HashMap<String, String>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub url: Option<String>,
}

/// Configuration for the MCP client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientConfig {
	pub servers: Vec<ServerConnection>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub client_name: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub client_version: Option<String>,
}

/// Configuration for the MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
	pub name: String,
	pub version: String,
	pub transport: TransportType,
}

// ---------------------------------------------------------------------------
// MCP protocol — server-side registration definitions
// ---------------------------------------------------------------------------

/// A tool definition for server-side registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
	pub name: String,
	pub description: String,
	pub input_schema: serde_json::Value,
}

/// A resource definition for server-side registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDefinition {
	pub uri: String,
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mime_type: Option<String>,
}

/// A prompt definition for server-side registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDefinition {
	pub name: String,
	pub description: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub arguments: Option<Vec<PromptArgument>>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// -- JSON-RPC framing --

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
	fn test_json_rpc_notification() {
		let notif =
			JsonRpcNotification::new("notifications/tools/list_changed", None);
		let json = serde_json::to_string(&notif).unwrap();
		assert!(json.contains(r#""jsonrpc":"2.0""#));
		assert!(json.contains("notifications/tools/list_changed"));
		assert!(!json.contains(r#""params""#));
	}

	// -- Initialize --

	#[test]
	fn test_mcp_initialize_params_roundtrip() {
		let params = McpInitializeParams {
			protocol_version: "2024-11-05".into(),
			capabilities: ClientCapabilities {
				roots: Some(RootCapabilities {
					list_changed: Some(true),
				}),
				sampling: None,
			},
			client_info: ImplementationInfo {
				name: "simse".into(),
				version: "1.0.0".into(),
			},
		};
		let json = serde_json::to_string(&params).unwrap();
		assert!(json.contains("protocolVersion"));
		assert!(json.contains("clientInfo"));
		assert!(json.contains("listChanged"));

		let decoded: McpInitializeParams = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.protocol_version, "2024-11-05");
		assert_eq!(decoded.client_info.name, "simse");
		assert!(decoded.capabilities.roots.unwrap().list_changed.unwrap());
	}

	#[test]
	fn test_mcp_initialize_result_roundtrip() {
		let result = McpInitializeResult {
			protocol_version: "2024-11-05".into(),
			capabilities: ServerCapabilities {
				tools: Some(serde_json::json!({})),
				resources: None,
				prompts: None,
				logging: None,
			},
			server_info: ImplementationInfo {
				name: "test-server".into(),
				version: "0.1.0".into(),
			},
		};
		let json = serde_json::to_string(&result).unwrap();
		assert!(json.contains("serverInfo"));
		assert!(json.contains("protocolVersion"));

		let decoded: McpInitializeResult = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.server_info.name, "test-server");
		assert!(decoded.capabilities.tools.is_some());
	}

	// -- Tools --

	#[test]
	fn test_tool_info_roundtrip() {
		let tool = ToolInfo {
			name: "read_file".into(),
			description: Some("Read file contents".into()),
			input_schema: serde_json::json!({
				"type": "object",
				"properties": {
					"path": { "type": "string" }
				},
				"required": ["path"]
			}),
			annotations: Some(ToolAnnotations {
				title: Some("Read File".into()),
				read_only_hint: Some(true),
				destructive_hint: Some(false),
				idempotent_hint: Some(true),
				open_world_hint: None,
			}),
		};
		let json = serde_json::to_string(&tool).unwrap();
		assert!(json.contains("inputSchema"));
		assert!(json.contains("readOnlyHint"));
		assert!(json.contains("destructiveHint"));
		assert!(json.contains("idempotentHint"));
		assert!(!json.contains("openWorldHint"));

		let decoded: ToolInfo = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.name, "read_file");
		assert!(decoded.annotations.unwrap().read_only_hint.unwrap());
	}

	#[test]
	fn test_tool_call_params_roundtrip() {
		let params = ToolCallParams {
			name: "read_file".into(),
			arguments: serde_json::json!({"path": "/tmp/test.txt"}),
		};
		let json = serde_json::to_string(&params).unwrap();
		let decoded: ToolCallParams = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.name, "read_file");
		assert_eq!(decoded.arguments["path"], "/tmp/test.txt");
	}

	#[test]
	fn test_tool_call_result_roundtrip() {
		let result = ToolCallResult {
			content: vec![ContentItem::Text {
				text: "file contents here".into(),
			}],
			is_error: Some(false),
		};
		let json = serde_json::to_string(&result).unwrap();
		assert!(json.contains("isError"));

		let decoded: ToolCallResult = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.content.len(), 1);
		assert_eq!(decoded.is_error, Some(false));
	}

	// -- Content items --

	#[test]
	fn test_content_item_text_roundtrip() {
		let item = ContentItem::Text {
			text: "hello".into(),
		};
		let json = serde_json::to_string(&item).unwrap();
		assert!(json.contains(r#""type":"text""#));
		assert!(json.contains(r#""text":"hello""#));

		let decoded: ContentItem = serde_json::from_str(&json).unwrap();
		match decoded {
			ContentItem::Text { text } => assert_eq!(text, "hello"),
			_ => panic!("expected Text variant"),
		}
	}

	#[test]
	fn test_content_item_image_roundtrip() {
		let item = ContentItem::Image {
			data: "iVBORw0KGgo=".into(),
			mime_type: "image/png".into(),
		};
		let json = serde_json::to_string(&item).unwrap();
		assert!(json.contains(r#""type":"image""#));
		assert!(json.contains("mimeType"));
		assert!(json.contains("iVBORw0KGgo="));

		let decoded: ContentItem = serde_json::from_str(&json).unwrap();
		match decoded {
			ContentItem::Image { data, mime_type } => {
				assert_eq!(data, "iVBORw0KGgo=");
				assert_eq!(mime_type, "image/png");
			}
			_ => panic!("expected Image variant"),
		}
	}

	#[test]
	fn test_content_item_resource_roundtrip() {
		let item = ContentItem::Resource {
			resource: ResourceContent {
				uri: "file:///test.txt".into(),
				text: Some("content".into()),
				blob: None,
				mime_type: Some("text/plain".into()),
			},
		};
		let json = serde_json::to_string(&item).unwrap();
		assert!(json.contains(r#""type":"resource""#));

		let decoded: ContentItem = serde_json::from_str(&json).unwrap();
		match decoded {
			ContentItem::Resource { resource } => {
				assert_eq!(resource.uri, "file:///test.txt");
				assert_eq!(resource.text.as_deref(), Some("content"));
				assert_eq!(resource.mime_type.as_deref(), Some("text/plain"));
			}
			_ => panic!("expected Resource variant"),
		}
	}

	// -- Resources --

	#[test]
	fn test_resource_info_roundtrip() {
		let resource = ResourceInfo {
			uri: "file:///workspace/README.md".into(),
			name: "README.md".into(),
			description: Some("Project readme".into()),
			mime_type: Some("text/markdown".into()),
		};
		let json = serde_json::to_string(&resource).unwrap();
		assert!(json.contains("mimeType"));

		let decoded: ResourceInfo = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.uri, "file:///workspace/README.md");
		assert_eq!(decoded.name, "README.md");
		assert_eq!(decoded.mime_type.as_deref(), Some("text/markdown"));
	}

	#[test]
	fn test_resource_template_info_roundtrip() {
		let template = ResourceTemplateInfo {
			uri_template: "file:///{path}".into(),
			name: "File".into(),
			description: None,
			mime_type: None,
		};
		let json = serde_json::to_string(&template).unwrap();
		assert!(json.contains("uriTemplate"));

		let decoded: ResourceTemplateInfo = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.uri_template, "file:///{path}");
	}

	#[test]
	fn test_read_resource_roundtrip() {
		let params = ReadResourceParams {
			uri: "file:///test.txt".into(),
		};
		let json = serde_json::to_string(&params).unwrap();
		let decoded: ReadResourceParams = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.uri, "file:///test.txt");

		let result = ReadResourceResult {
			contents: vec![ResourceContent {
				uri: "file:///test.txt".into(),
				text: Some("hello".into()),
				blob: None,
				mime_type: None,
			}],
		};
		let json = serde_json::to_string(&result).unwrap();
		let decoded: ReadResourceResult = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.contents.len(), 1);
		assert_eq!(decoded.contents[0].text.as_deref(), Some("hello"));
	}

	// -- Prompts --

	#[test]
	fn test_prompt_info_roundtrip() {
		let prompt = PromptInfo {
			name: "summarize".into(),
			description: Some("Summarize content".into()),
			arguments: Some(vec![
				PromptArgument {
					name: "content".into(),
					description: Some("The content to summarize".into()),
					required: Some(true),
				},
				PromptArgument {
					name: "style".into(),
					description: None,
					required: Some(false),
				},
			]),
		};
		let json = serde_json::to_string(&prompt).unwrap();

		let decoded: PromptInfo = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.name, "summarize");
		let args = decoded.arguments.unwrap();
		assert_eq!(args.len(), 2);
		assert_eq!(args[0].name, "content");
		assert!(args[0].required.unwrap());
	}

	#[test]
	fn test_get_prompt_roundtrip() {
		let params = GetPromptParams {
			name: "summarize".into(),
			arguments: serde_json::json!({"content": "some text"}),
		};
		let json = serde_json::to_string(&params).unwrap();
		let decoded: GetPromptParams = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.name, "summarize");

		let result = GetPromptResult {
			messages: vec![PromptMessage {
				role: "user".into(),
				content: serde_json::json!("Please summarize: some text"),
			}],
		};
		let json = serde_json::to_string(&result).unwrap();
		let decoded: GetPromptResult = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.messages.len(), 1);
		assert_eq!(decoded.messages[0].role, "user");
	}

	// -- Logging --

	#[test]
	fn test_logging_level_serde() {
		let json = r#""debug""#;
		let level: LoggingLevel = serde_json::from_str(json).unwrap();
		assert_eq!(level, LoggingLevel::Debug);

		let json = r#""warning""#;
		let level: LoggingLevel = serde_json::from_str(json).unwrap();
		assert_eq!(level, LoggingLevel::Warning);

		let serialized = serde_json::to_string(&LoggingLevel::Emergency).unwrap();
		assert_eq!(serialized, r#""emergency""#);

		let serialized = serde_json::to_string(&LoggingLevel::Critical).unwrap();
		assert_eq!(serialized, r#""critical""#);
	}

	#[test]
	fn test_logging_message_roundtrip() {
		let msg = LoggingMessage {
			level: LoggingLevel::Info,
			logger: Some("mcp-server".into()),
			data: serde_json::json!("Server started on port 3000"),
		};
		let json = serde_json::to_string(&msg).unwrap();
		let decoded: LoggingMessage = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.level, LoggingLevel::Info);
		assert_eq!(decoded.logger.as_deref(), Some("mcp-server"));
	}

	// -- Completions --

	#[test]
	fn test_completion_ref_resource_roundtrip() {
		let cref = CompletionRef::ResourceRef {
			uri: "file:///workspace".into(),
		};
		let json = serde_json::to_string(&cref).unwrap();
		assert!(json.contains(r#""type":"ref/resource""#));
		assert!(json.contains("file:///workspace"));

		let decoded: CompletionRef = serde_json::from_str(&json).unwrap();
		match decoded {
			CompletionRef::ResourceRef { uri } => assert_eq!(uri, "file:///workspace"),
			_ => panic!("expected ResourceRef variant"),
		}
	}

	#[test]
	fn test_completion_ref_prompt_roundtrip() {
		let cref = CompletionRef::PromptRef {
			name: "summarize".into(),
		};
		let json = serde_json::to_string(&cref).unwrap();
		assert!(json.contains(r#""type":"ref/prompt""#));
		assert!(json.contains("summarize"));

		let decoded: CompletionRef = serde_json::from_str(&json).unwrap();
		match decoded {
			CompletionRef::PromptRef { name } => assert_eq!(name, "summarize"),
			_ => panic!("expected PromptRef variant"),
		}
	}

	#[test]
	fn test_completion_result_roundtrip() {
		let result = CompletionResult {
			values: vec!["option1".into(), "option2".into()],
			has_more: Some(true),
			total: Some(10),
		};
		let json = serde_json::to_string(&result).unwrap();
		assert!(json.contains("hasMore"));

		let decoded: CompletionResult = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.values.len(), 2);
		assert!(decoded.has_more.unwrap());
		assert_eq!(decoded.total, Some(10));
	}

	// -- Roots --

	#[test]
	fn test_root_roundtrip() {
		let root = Root {
			uri: "file:///workspace".into(),
			name: Some("workspace".into()),
		};
		let json = serde_json::to_string(&root).unwrap();
		let decoded: Root = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.uri, "file:///workspace");
		assert_eq!(decoded.name.as_deref(), Some("workspace"));
	}

	// -- Transport and config --

	#[test]
	fn test_transport_type_serde() {
		let json = r#""stdio""#;
		let t: TransportType = serde_json::from_str(json).unwrap();
		assert_eq!(t, TransportType::Stdio);

		let json = r#""http""#;
		let t: TransportType = serde_json::from_str(json).unwrap();
		assert_eq!(t, TransportType::Http);

		let serialized = serde_json::to_string(&TransportType::Stdio).unwrap();
		assert_eq!(serialized, r#""stdio""#);
	}

	#[test]
	fn test_server_connection_roundtrip() {
		let conn = ServerConnection {
			name: "my-server".into(),
			transport: TransportType::Stdio,
			command: Some("node".into()),
			args: Some(vec!["server.js".into()]),
			env: None,
			url: None,
		};
		let json = serde_json::to_string(&conn).unwrap();
		let decoded: ServerConnection = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.name, "my-server");
		assert_eq!(decoded.transport, TransportType::Stdio);
		assert_eq!(decoded.command.as_deref(), Some("node"));
		assert_eq!(decoded.args.as_ref().unwrap(), &["server.js"]);
	}

	#[test]
	fn test_server_connection_http() {
		let json = r#"{"name":"remote","transport":"http","url":"http://localhost:3000/mcp"}"#;
		let conn: ServerConnection = serde_json::from_str(json).unwrap();
		assert_eq!(conn.name, "remote");
		assert_eq!(conn.transport, TransportType::Http);
		assert_eq!(conn.url.as_deref(), Some("http://localhost:3000/mcp"));
		assert!(conn.command.is_none());
	}

	#[test]
	fn test_mcp_client_config_roundtrip() {
		let config = McpClientConfig {
			servers: vec![ServerConnection {
				name: "tools".into(),
				transport: TransportType::Stdio,
				command: Some("npx".into()),
				args: Some(vec!["-y".into(), "@mcp/tools".into()]),
				env: None,
				url: None,
			}],
			client_name: Some("simse".into()),
			client_version: Some("1.0.0".into()),
		};
		let json = serde_json::to_string(&config).unwrap();
		assert!(json.contains("clientName"));
		assert!(json.contains("clientVersion"));

		let decoded: McpClientConfig = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.servers.len(), 1);
		assert_eq!(decoded.client_name.as_deref(), Some("simse"));
	}

	#[test]
	fn test_mcp_server_config_roundtrip() {
		let config = McpServerConfig {
			name: "simse-mcp".into(),
			version: "0.1.0".into(),
			transport: TransportType::Stdio,
		};
		let json = serde_json::to_string(&config).unwrap();
		let decoded: McpServerConfig = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.name, "simse-mcp");
		assert_eq!(decoded.transport, TransportType::Stdio);
	}

	// -- Registration definitions --

	#[test]
	fn test_tool_definition_roundtrip() {
		let def = ToolDefinition {
			name: "search".into(),
			description: "Search for items".into(),
			input_schema: serde_json::json!({
				"type": "object",
				"properties": {
					"query": { "type": "string" }
				}
			}),
		};
		let json = serde_json::to_string(&def).unwrap();
		assert!(json.contains("inputSchema"));

		let decoded: ToolDefinition = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.name, "search");
	}

	#[test]
	fn test_resource_definition_roundtrip() {
		let def = ResourceDefinition {
			uri: "file:///config.json".into(),
			name: "Config".into(),
			description: Some("Application configuration".into()),
			mime_type: Some("application/json".into()),
		};
		let json = serde_json::to_string(&def).unwrap();
		assert!(json.contains("mimeType"));

		let decoded: ResourceDefinition = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.uri, "file:///config.json");
		assert_eq!(decoded.mime_type.as_deref(), Some("application/json"));
	}

	#[test]
	fn test_prompt_definition_roundtrip() {
		let def = PromptDefinition {
			name: "analyze".into(),
			description: "Analyze code".into(),
			arguments: Some(vec![PromptArgument {
				name: "code".into(),
				description: Some("Source code to analyze".into()),
				required: Some(true),
			}]),
		};
		let json = serde_json::to_string(&def).unwrap();

		let decoded: PromptDefinition = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.name, "analyze");
		assert_eq!(decoded.arguments.unwrap().len(), 1);
	}

	// -- Error codes --

	#[test]
	fn test_error_code_constants() {
		assert_eq!(INTERNAL_ERROR, -32603);
		assert_eq!(METHOD_NOT_FOUND, -32601);
		assert_eq!(INVALID_PARAMS, -32602);
		assert_eq!(MCP_ERROR, -32000);
	}
}
