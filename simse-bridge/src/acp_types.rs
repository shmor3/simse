//! ACP protocol types: content blocks, session management, streaming, permissions, tool calls.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ACP content block — tagged enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
	Text { text: String },
	Data { data: serde_json::Value },
}

/// Parameters for session/prompt requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams {
	pub session_id: String,
	#[serde(rename = "prompt")]
	pub content: Vec<ContentBlock>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub metadata: Option<PromptMetadata>,
}

/// Metadata attached to a prompt.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMetadata {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_id: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub system_prompt: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub temperature: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub max_tokens: Option<u32>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_p: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub top_k: Option<u32>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub stop_sequences: Vec<String>,
}

/// Result from session/prompt.
///
/// The ACP protocol returns `stopReason` and optional `usage` in the response.
/// Content arrives via `session/update` notifications (streaming deltas).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptResult {
	pub stop_reason: String,
	#[serde(default)]
	pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultMetadata {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
	#[serde(default)]
	pub prompt_tokens: u64,
	#[serde(default)]
	pub completion_tokens: u64,
}

/// Agent info returned by initialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
	pub name: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub version: Option<String>,
}

/// Initialize response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
	pub protocol_version: u32,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub agent_info: Option<AgentInfo>,
}

/// Session/new response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewResult {
	pub session_id: String,
}

/// A session update notification payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdate {
	pub session_id: String,
	pub update: UpdatePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePayload {
	pub session_update: String,
	#[serde(default, deserialize_with = "deserialize_content_one_or_many")]
	pub content: Vec<ContentBlock>,
}

/// Deserialize content as either a single ContentBlock or an array.
/// The ACP protocol sends a single object for chunks but some implementations
/// may send an array.
fn deserialize_content_one_or_many<'de, D>(
	deserializer: D,
) -> Result<Vec<ContentBlock>, D::Error>
where
	D: serde::Deserializer<'de>,
{
	use serde::de;

	struct ContentVisitor;

	impl<'de> de::Visitor<'de> for ContentVisitor {
		type Value = Vec<ContentBlock>;

		fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
			formatter.write_str("a content block or array of content blocks")
		}

		fn visit_seq<A>(self, seq: A) -> Result<Vec<ContentBlock>, A::Error>
		where
			A: de::SeqAccess<'de>,
		{
			Vec::<ContentBlock>::deserialize(de::value::SeqAccessDeserializer::new(seq))
		}

		fn visit_map<M>(self, map: M) -> Result<Vec<ContentBlock>, M::Error>
		where
			M: de::MapAccess<'de>,
		{
			let block =
				ContentBlock::deserialize(de::value::MapAccessDeserializer::new(map))?;
			Ok(vec![block])
		}
	}

	deserializer.deserialize_any(ContentVisitor)
}

/// Options for generate requests.
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
	pub agent_id: Option<String>,
	pub server_name: Option<String>,
	pub system_prompt: Option<String>,
	pub temperature: Option<f64>,
	pub max_tokens: Option<u32>,
	pub top_p: Option<f64>,
	pub top_k: Option<u32>,
	pub stop_sequences: Vec<String>,
}

/// A streaming event from generate_stream.
#[derive(Debug, Clone)]
pub enum StreamEvent {
	Delta(String),
	ToolCall {
		id: String,
		name: String,
		args: String,
	},
	ToolCallUpdate {
		id: String,
		status: String,
		summary: Option<String>,
	},
	Complete(SessionPromptResult),
	Usage(TokenUsage),
	/// Transport or protocol error encountered during streaming.
	///
	/// Sent before the channel closes so consumers can distinguish a clean
	/// end-of-stream from a failure.
	Error(String),
}

/// Result from a generate (non-streaming) call.
#[derive(Debug, Clone)]
pub struct GenerateResult {
	pub content: String,
	pub stop_reason: String,
	pub usage: Option<TokenUsage>,
}

/// Result from an embed call.
#[derive(Debug, Clone)]
pub struct EmbedResult {
	pub embeddings: Vec<Vec<f32>>,
	pub prompt_tokens: u64,
}

/// ACP server connection info.
#[derive(Debug, Clone)]
pub struct AcpServerInfo {
	pub command: String,
	pub args: Vec<String>,
	pub cwd: Option<String>,
	pub env: HashMap<String, String>,
	pub timeout_ms: u64,
	pub init_timeout_ms: u64,
}

impl Default for AcpServerInfo {
	fn default() -> Self {
		Self {
			command: String::new(),
			args: Vec::new(),
			cwd: None,
			env: HashMap::new(),
			timeout_ms: 60_000,
			init_timeout_ms: 30_000,
		}
	}
}

/// A permission request from the ACP agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestParams {
	pub session_id: String,
	pub tool_name: String,
	pub args: serde_json::Value,
	pub options: Vec<PermissionOptionDef>,
}

/// An option in a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOptionDef {
	pub id: String,
	pub label: String,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn content_block_text_serializes() {
		let block = ContentBlock::Text {
			text: "hello".into(),
		};
		let json = serde_json::to_value(&block).unwrap();
		assert_eq!(json["type"], "text");
		assert_eq!(json["text"], "hello");
	}

	#[test]
	fn content_block_data_serializes() {
		let block = ContentBlock::Data {
			data: serde_json::json!({"key": "value"}),
		};
		let json = serde_json::to_value(&block).unwrap();
		assert_eq!(json["type"], "data");
		assert_eq!(json["data"]["key"], "value");
	}

	#[test]
	fn content_block_text_deserializes() {
		let json = r#"{"type":"text","text":"world"}"#;
		let block: ContentBlock = serde_json::from_str(json).unwrap();
		match block {
			ContentBlock::Text { text } => assert_eq!(text, "world"),
			_ => panic!("Expected Text variant"),
		}
	}

	#[test]
	fn content_block_data_deserializes() {
		let json = r#"{"type":"data","data":42}"#;
		let block: ContentBlock = serde_json::from_str(json).unwrap();
		match block {
			ContentBlock::Data { data } => assert_eq!(data, serde_json::json!(42)),
			_ => panic!("Expected Data variant"),
		}
	}

	#[test]
	fn session_prompt_params_serializes_camel_case() {
		let params = SessionPromptParams {
			session_id: "sess-1".into(),
			content: vec![ContentBlock::Text {
				text: "hi".into(),
			}],
			metadata: None,
		};
		let json = serde_json::to_value(&params).unwrap();
		assert_eq!(json["sessionId"], "sess-1");
		// Must not contain snake_case key
		assert!(json.get("session_id").is_none());
	}

	#[test]
	fn initialize_result_deserializes() {
		let json = r#"{
			"protocolVersion": 1,
			"agentInfo": {
				"name": "test-agent",
				"version": "0.1.0"
			}
		}"#;
		let result: InitializeResult = serde_json::from_str(json).unwrap();
		assert_eq!(result.protocol_version, 1);
		let info = result.agent_info.unwrap();
		assert_eq!(info.name, "test-agent");
		assert_eq!(info.version, Some("0.1.0".into()));
	}

	#[test]
	fn initialize_result_deserializes_without_agent_info() {
		let json = r#"{"protocolVersion": 1}"#;
		let result: InitializeResult = serde_json::from_str(json).unwrap();
		assert_eq!(result.protocol_version, 1);
		assert!(result.agent_info.is_none());
	}

	#[test]
	fn token_usage_deserializes() {
		let json = r#"{"promptTokens": 100, "completionTokens": 50}"#;
		let usage: TokenUsage = serde_json::from_str(json).unwrap();
		assert_eq!(usage.prompt_tokens, 100);
		assert_eq!(usage.completion_tokens, 50);
	}

	#[test]
	fn token_usage_defaults_to_zero() {
		let json = r#"{}"#;
		let usage: TokenUsage = serde_json::from_str(json).unwrap();
		assert_eq!(usage.prompt_tokens, 0);
		assert_eq!(usage.completion_tokens, 0);
	}

	#[test]
	fn generate_options_defaults() {
		let opts = GenerateOptions::default();
		assert!(opts.agent_id.is_none());
		assert!(opts.server_name.is_none());
		assert!(opts.system_prompt.is_none());
		assert!(opts.temperature.is_none());
		assert!(opts.max_tokens.is_none());
	}

	#[test]
	fn acp_server_info_defaults() {
		let info = AcpServerInfo::default();
		assert_eq!(info.timeout_ms, 60_000);
		assert_eq!(info.init_timeout_ms, 30_000);
		assert!(info.command.is_empty());
		assert!(info.args.is_empty());
		assert!(info.cwd.is_none());
		assert!(info.env.is_empty());
	}

	#[test]
	fn session_new_result_deserializes() {
		let json = r#"{"sessionId": "abc-123"}"#;
		let result: SessionNewResult = serde_json::from_str(json).unwrap();
		assert_eq!(result.session_id, "abc-123");
	}

	#[test]
	fn session_update_deserializes() {
		let json = r#"{
			"sessionId": "sess-1",
			"update": {
				"sessionUpdate": "agent_message_chunk",
				"content": [{"type": "text", "text": "chunk"}]
			}
		}"#;
		let update: SessionUpdate = serde_json::from_str(json).unwrap();
		assert_eq!(update.session_id, "sess-1");
		assert_eq!(update.update.session_update, "agent_message_chunk");
		assert_eq!(update.update.content.len(), 1);
	}

	#[test]
	fn prompt_metadata_skip_none_fields() {
		let meta = PromptMetadata::default();
		let json = serde_json::to_value(&meta).unwrap();
		// All None fields should be skipped
		assert!(json.get("agentId").is_none());
		assert!(json.get("systemPrompt").is_none());
		assert!(json.get("temperature").is_none());
		assert!(json.get("maxTokens").is_none());
		assert!(json.get("topP").is_none());
		assert!(json.get("topK").is_none());
		assert!(json.get("stopSequences").is_none());
	}

	#[test]
	fn prompt_metadata_with_sampling_params_deserializes() {
		let json = r#"{
			"temperature": 0.8,
			"topP": 0.95,
			"topK": 40,
			"stopSequences": ["STOP", "END"],
			"maxTokens": 2048
		}"#;
		let meta: PromptMetadata = serde_json::from_str(json).unwrap();
		assert_eq!(meta.temperature, Some(0.8));
		assert_eq!(meta.top_p, Some(0.95));
		assert_eq!(meta.top_k, Some(40));
		assert_eq!(meta.stop_sequences, vec!["STOP", "END"]);
		assert_eq!(meta.max_tokens, Some(2048));
	}

	#[test]
	fn prompt_metadata_sampling_params_default_when_absent() {
		let json = r#"{"temperature": 0.5}"#;
		let meta: PromptMetadata = serde_json::from_str(json).unwrap();
		assert_eq!(meta.temperature, Some(0.5));
		assert!(meta.top_p.is_none());
		assert!(meta.top_k.is_none());
		assert!(meta.stop_sequences.is_empty());
	}

	#[test]
	fn result_metadata_with_usage() {
		let json = r#"{
			"usage": {
				"promptTokens": 200,
				"completionTokens": 100
			}
		}"#;
		let meta: ResultMetadata = serde_json::from_str(json).unwrap();
		let usage = meta.usage.unwrap();
		assert_eq!(usage.prompt_tokens, 200);
		assert_eq!(usage.completion_tokens, 100);
	}

	#[test]
	fn session_prompt_result_deserializes() {
		let json = r#"{
			"stopReason": "end_turn"
		}"#;
		let result: SessionPromptResult = serde_json::from_str(json).unwrap();
		assert_eq!(result.stop_reason, "end_turn");
		assert!(result.usage.is_none());
	}

	#[test]
	fn session_prompt_result_with_usage() {
		let json = r#"{
			"stopReason": "end_turn",
			"usage": {"promptTokens": 50, "completionTokens": 25}
		}"#;
		let result: SessionPromptResult = serde_json::from_str(json).unwrap();
		assert_eq!(result.stop_reason, "end_turn");
		let usage = result.usage.unwrap();
		assert_eq!(usage.prompt_tokens, 50);
		assert_eq!(usage.completion_tokens, 25);
	}
}
