//! ACP (Agent Client Protocol) client wrapping the low-level bridge primitives.
//!
//! Provides a high-level async API for ACP operations: session management,
//! text generation (streaming and non-streaming), embeddings, and permissions.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::Mutex;

use crate::acp_types::{
	AcpServerInfo, AgentInfo, ContentBlock, EmbedResult, GenerateOptions, GenerateResult,
	InitializeResult, PromptMetadata, SessionNewResult, SessionPromptParams, SessionPromptResult,
	SessionUpdate, StreamEvent,
};
use crate::client::{spawn_bridge, BridgeConfig, BridgeError, BridgeProcess};
use crate::protocol::JsonRpcNotification;

/// ACP-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum AcpError {
	#[error("Bridge error: {0}")]
	Bridge(#[from] BridgeError),
	#[error("Protocol error: {0}")]
	Protocol(String),
	#[error("Not initialized")]
	NotInitialized,
	#[error("Session not found: {0}")]
	SessionNotFound(String),
	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),
}

/// High-level ACP client wrapping a `BridgeProcess`.
///
/// The bridge process is behind `Arc<Mutex<>>` to allow shared ownership,
/// though all operations require exclusive access to the subprocess pipes.
pub struct AcpClient {
	bridge: Arc<Mutex<BridgeProcess>>,
	agent_info: Option<AgentInfo>,
}

impl AcpClient {
	/// Spawn a bridge subprocess and perform the ACP initialize handshake.
	///
	/// Returns an initialized `AcpClient` ready for session creation.
	pub async fn connect(server: AcpServerInfo) -> Result<Self, AcpError> {
		let config = BridgeConfig {
			command: server.command,
			args: server.args,
			data_dir: String::new(),
			timeout_ms: server.timeout_ms,
		};

		let mut bridge = spawn_bridge(&config).await?;

		// Perform initialize handshake with init-specific timeout
		let init_timeout = Duration::from_millis(server.init_timeout_ms);
		let resp = tokio::time::timeout(
			init_timeout,
			crate::client::request(&mut bridge, "initialize", None),
		)
		.await
		.map_err(|_| AcpError::Bridge(BridgeError::Timeout))?
		.map_err(AcpError::Bridge)?;

		// Check for RPC error
		if let Some(err) = resp.error {
			return Err(AcpError::Protocol(format!(
				"Initialize failed: {} (code {})",
				err.message, err.code
			)));
		}

		// Parse initialize result
		let agent_info = resp
			.result
			.as_ref()
			.and_then(|r| serde_json::from_value::<InitializeResult>(r.clone()).ok())
			.and_then(|init| init.agent_info);

		Ok(Self {
			bridge: Arc::new(Mutex::new(bridge)),
			agent_info,
		})
	}

	/// Check if the underlying child process is still running.
	pub async fn is_healthy(&self) -> bool {
		let bridge = self.bridge.lock().await;
		crate::client::is_healthy(&bridge)
	}

	/// Return agent info from the initialize response, if available.
	pub fn agent_info(&self) -> Option<&AgentInfo> {
		self.agent_info.as_ref()
	}

	/// Create a new ACP session. Returns the session ID.
	pub async fn new_session(&self) -> Result<String, AcpError> {
		let mut bridge = self.bridge.lock().await;
		let resp = crate::client::request(&mut bridge, "session/new", None).await?;

		if let Some(err) = resp.error {
			return Err(AcpError::Protocol(format!(
				"session/new failed: {} (code {})",
				err.message, err.code
			)));
		}

		let result: SessionNewResult = serde_json::from_value(
			resp.result
				.ok_or_else(|| AcpError::Protocol("session/new returned no result".into()))?,
		)?;

		Ok(result.session_id)
	}

	/// Non-streaming text generation.
	///
	/// Sends a `session/prompt` request and waits for the complete response.
	pub async fn generate(
		&self,
		session_id: &str,
		prompt: &str,
		options: GenerateOptions,
	) -> Result<GenerateResult, AcpError> {
		let params = self.build_prompt_params(session_id, prompt, &options);
		let params_value = serde_json::to_value(&params)?;

		let mut bridge = self.bridge.lock().await;
		let resp =
			crate::client::request(&mut bridge, "session/prompt", Some(params_value)).await?;

		if let Some(err) = resp.error {
			return Err(AcpError::Protocol(format!(
				"session/prompt failed: {} (code {})",
				err.message, err.code
			)));
		}

		let result: SessionPromptResult = serde_json::from_value(
			resp.result
				.ok_or_else(|| AcpError::Protocol("session/prompt returned no result".into()))?,
		)?;

		let content = extract_text_content(&result.content);
		let usage = result
			.metadata
			.as_ref()
			.and_then(|m| m.usage.clone());

		Ok(GenerateResult {
			content,
			stop_reason: result.stop_reason,
			usage,
		})
	}

	/// Streaming text generation.
	///
	/// Sends a `session/prompt` request and returns a channel receiver that
	/// yields `StreamEvent`s as notifications arrive. The final response is
	/// sent as `StreamEvent::Complete`.
	pub async fn generate_stream(
		&self,
		session_id: &str,
		prompt: &str,
		options: GenerateOptions,
	) -> Result<mpsc::UnboundedReceiver<StreamEvent>, AcpError> {
		let params = self.build_prompt_params(session_id, prompt, &options);
		let params_value = serde_json::to_value(&params)?;

		let (notif_tx, mut notif_rx) = mpsc::unbounded_channel::<JsonRpcNotification>();
		let (event_tx, event_rx) = mpsc::unbounded_channel::<StreamEvent>();

		let bridge = Arc::clone(&self.bridge);

		// Spawn a concurrent task that progressively forwards notifications
		// to StreamEvents as they arrive (not buffered until request completes).
		let forward_tx = event_tx.clone();
		let forward_handle = tokio::spawn(async move {
			while let Some(notif) = notif_rx.recv().await {
				if parse_notification(&notif)
					.is_some_and(|event| forward_tx.send(event).is_err())
				{
					break; // consumer dropped
				}
			}
		});

		// Spawn a task that drives the streaming request
		tokio::spawn(async move {
			let mut guard = bridge.lock().await;
			let resp = crate::client::request_streaming(
				&mut guard,
				"session/prompt",
				Some(params_value),
				notif_tx,
			)
			.await;
			// Release the lock so other operations can proceed
			drop(guard);

			// Wait for the forwarding task to finish draining any remaining
			// notifications (it will end once notif_tx is dropped above).
			let _ = forward_handle.await;

			// Process the final response
			match resp {
				Ok(resp) => {
					if let Some(Ok(result)) = resp.result.map(
						serde_json::from_value::<SessionPromptResult>,
					) {
						// Send usage if available
						if let Some(usage) =
							result.metadata.as_ref().and_then(|m| m.usage.clone())
						{
							let _ = event_tx.send(StreamEvent::Usage(usage));
						}
						let _ = event_tx.send(StreamEvent::Complete(result));
					}
				}
				Err(e) => {
					let _ = event_tx.send(StreamEvent::Error(e.to_string()));
				}
			}
		});

		Ok(event_rx)
	}

	/// Generate embeddings for a list of texts.
	///
	/// Sends a `session/prompt` with a single data content block containing
	/// the embed action, texts array, and optional model name.
	pub async fn embed(
		&self,
		session_id: &str,
		texts: &[String],
		model: Option<&str>,
	) -> Result<EmbedResult, AcpError> {
		let mut data = serde_json::json!({
			"action": "embed",
			"texts": texts,
		});
		if let Some(m) = model {
			data["model"] = serde_json::Value::String(m.to_string());
		}

		let content = vec![ContentBlock::Data { data }];

		let params = SessionPromptParams {
			session_id: session_id.to_string(),
			content,
			metadata: None,
		};
		let params_value = serde_json::to_value(&params)?;

		let mut bridge = self.bridge.lock().await;
		let resp =
			crate::client::request(&mut bridge, "session/prompt", Some(params_value)).await?;

		if let Some(err) = resp.error {
			return Err(AcpError::Protocol(format!(
				"embed failed: {} (code {})",
				err.message, err.code
			)));
		}

		let result: SessionPromptResult = serde_json::from_value(
			resp.result
				.ok_or_else(|| AcpError::Protocol("embed returned no result".into()))?,
		)?;

		// Extract embeddings from data content blocks
		let mut embeddings = Vec::new();
		for block in &result.content {
			if let ContentBlock::Data { data } = block
				&& let Some(embedding) = data.get("embedding")
				&& let Ok(vec) = serde_json::from_value::<Vec<f32>>(embedding.clone())
			{
				embeddings.push(vec);
			}
		}

		let prompt_tokens = result
			.metadata
			.as_ref()
			.and_then(|m| m.usage.as_ref())
			.map(|u| u.prompt_tokens)
			.unwrap_or(0);

		Ok(EmbedResult {
			embeddings,
			prompt_tokens,
		})
	}

	/// Set a session config option (e.g., mode or model switching).
	///
	/// Sends `session/set_config_option` with the specified config option ID and group ID.
	pub async fn set_session_config(
		&self,
		session_id: &str,
		config_option_id: &str,
		group_id: &str,
	) -> Result<(), AcpError> {
		let params = serde_json::json!({
			"sessionId": session_id,
			"configOptionId": config_option_id,
			"groupId": group_id,
		});

		let mut bridge = self.bridge.lock().await;
		let resp =
			crate::client::request(&mut bridge, "session/set_config_option", Some(params)).await?;

		if let Some(err) = resp.error {
			return Err(AcpError::Protocol(format!(
				"set_config_option failed: {} (code {})",
				err.message, err.code
			)));
		}

		Ok(())
	}

	/// Respond to a permission request from the agent.
	///
	/// This is sent as a JSON-RPC notification (fire-and-forget), not a request.
	pub async fn respond_permission(
		&self,
		request_id: &str,
		option_id: &str,
	) -> Result<(), AcpError> {
		let notification = serde_json::json!({
			"jsonrpc": "2.0",
			"method": "session/permission_response",
			"params": {
				"requestId": request_id,
				"outcome": {
					"outcome": "selected",
					"optionId": option_id
				}
			}
		});

		let line = serde_json::to_string(&notification)?;
		let mut bridge = self.bridge.lock().await;
		crate::client::send_line(&mut bridge, &line).await?;

		Ok(())
	}

	/// Build `SessionPromptParams` from a prompt string and options.
	fn build_prompt_params(
		&self,
		session_id: &str,
		prompt: &str,
		options: &GenerateOptions,
	) -> SessionPromptParams {
		let content = vec![ContentBlock::Text {
			text: prompt.to_string(),
		}];

		let has_metadata = options.agent_id.is_some()
			|| options.system_prompt.is_some()
			|| options.temperature.is_some()
			|| options.max_tokens.is_some()
			|| options.top_p.is_some()
			|| options.top_k.is_some()
			|| !options.stop_sequences.is_empty();

		let metadata = if has_metadata {
			Some(PromptMetadata {
				agent_id: options.agent_id.clone(),
				system_prompt: options.system_prompt.clone(),
				temperature: options.temperature,
				max_tokens: options.max_tokens,
				top_p: options.top_p,
				top_k: options.top_k,
				stop_sequences: options.stop_sequences.clone(),
			})
		} else {
			None
		};

		SessionPromptParams {
			session_id: session_id.to_string(),
			content,
			metadata,
		}
	}
}

/// Extract concatenated text from a slice of content blocks.
///
/// Joins all `Text` blocks with no separator. `Data` blocks are skipped.
pub fn extract_text_content(blocks: &[ContentBlock]) -> String {
	let mut result = String::new();
	for block in blocks {
		if let ContentBlock::Text { text } = block {
			result.push_str(text);
		}
	}
	result
}

/// Parse a JSON-RPC notification into a `StreamEvent`, if applicable.
///
/// Recognizes `session/update` notifications and maps the update type to
/// the corresponding `StreamEvent` variant. Returns `None` for unrecognized
/// notification methods.
fn parse_notification(notif: &JsonRpcNotification) -> Option<StreamEvent> {
	if notif.method != "session/update" {
		return None;
	}

	let params = notif.params.as_ref()?;
	let update: SessionUpdate = serde_json::from_value(params.clone()).ok()?;

	match update.update.session_update.as_str() {
		"agent_message_chunk" => {
			let text = extract_text_content(&update.update.content);
			if text.is_empty() {
				None
			} else {
				Some(StreamEvent::Delta(text))
			}
		}
		"tool_call" => {
			// Extract tool call info from data blocks
			for block in &update.update.content {
				if let ContentBlock::Data { data } = block {
					let id = data
						.get("id")
						.and_then(|v| v.as_str())
						.unwrap_or("")
						.to_string();
					let name = data
						.get("name")
						.and_then(|v| v.as_str())
						.unwrap_or("")
						.to_string();
					let args = data
						.get("args")
						.map(|v| v.to_string())
						.unwrap_or_default();
					return Some(StreamEvent::ToolCall { id, name, args });
				}
			}
			None
		}
		"tool_call_update" => {
			for block in &update.update.content {
				if let ContentBlock::Data { data } = block {
					let id = data
						.get("id")
						.and_then(|v| v.as_str())
						.unwrap_or("")
						.to_string();
					let status = data
						.get("status")
						.and_then(|v| v.as_str())
						.unwrap_or("")
						.to_string();
					let summary = data
						.get("summary")
						.and_then(|v| v.as_str())
						.map(String::from);
					return Some(StreamEvent::ToolCallUpdate {
						id,
						status,
						summary,
					});
				}
			}
			None
		}
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::acp_types::*;

	#[test]
	fn extract_text_from_content_blocks() {
		let blocks = vec![
			ContentBlock::Text {
				text: "Hello ".into(),
			},
			ContentBlock::Data {
				data: serde_json::json!({"ignored": true}),
			},
			ContentBlock::Text {
				text: "world".into(),
			},
		];
		assert_eq!(extract_text_content(&blocks), "Hello world");
	}

	#[test]
	fn extract_text_from_empty_blocks() {
		let blocks: Vec<ContentBlock> = vec![];
		assert_eq!(extract_text_content(&blocks), "");
	}

	#[test]
	fn extract_text_from_only_data_blocks() {
		let blocks = vec![ContentBlock::Data {
			data: serde_json::json!(42),
		}];
		assert_eq!(extract_text_content(&blocks), "");
	}

	#[test]
	fn parse_notification_agent_message_chunk() {
		let notif = JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "session/update".into(),
			params: Some(serde_json::json!({
				"sessionId": "s1",
				"update": {
					"sessionUpdate": "agent_message_chunk",
					"content": [{"type": "text", "text": "chunk data"}]
				}
			})),
		};
		let event = parse_notification(&notif).unwrap();
		match event {
			StreamEvent::Delta(text) => assert_eq!(text, "chunk data"),
			_ => panic!("Expected Delta event"),
		}
	}

	#[test]
	fn parse_notification_tool_call() {
		let notif = JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "session/update".into(),
			params: Some(serde_json::json!({
				"sessionId": "s1",
				"update": {
					"sessionUpdate": "tool_call",
					"content": [{"type": "data", "data": {
						"id": "tc-1",
						"name": "search",
						"args": {"query": "test"}
					}}]
				}
			})),
		};
		let event = parse_notification(&notif).unwrap();
		match event {
			StreamEvent::ToolCall { id, name, args } => {
				assert_eq!(id, "tc-1");
				assert_eq!(name, "search");
				assert!(args.contains("test"));
			}
			_ => panic!("Expected ToolCall event"),
		}
	}

	#[test]
	fn parse_notification_tool_call_update() {
		let notif = JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "session/update".into(),
			params: Some(serde_json::json!({
				"sessionId": "s1",
				"update": {
					"sessionUpdate": "tool_call_update",
					"content": [{"type": "data", "data": {
						"id": "tc-1",
						"status": "completed",
						"summary": "Done"
					}}]
				}
			})),
		};
		let event = parse_notification(&notif).unwrap();
		match event {
			StreamEvent::ToolCallUpdate {
				id,
				status,
				summary,
			} => {
				assert_eq!(id, "tc-1");
				assert_eq!(status, "completed");
				assert_eq!(summary, Some("Done".into()));
			}
			_ => panic!("Expected ToolCallUpdate event"),
		}
	}

	#[test]
	fn parse_notification_ignores_non_session_update() {
		let notif = JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "some/other/method".into(),
			params: Some(serde_json::json!({"data": "irrelevant"})),
		};
		assert!(parse_notification(&notif).is_none());
	}

	#[test]
	fn parse_notification_ignores_unknown_update_type() {
		let notif = JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "session/update".into(),
			params: Some(serde_json::json!({
				"sessionId": "s1",
				"update": {
					"sessionUpdate": "unknown_type",
					"content": []
				}
			})),
		};
		assert!(parse_notification(&notif).is_none());
	}

	#[test]
	fn parse_notification_no_params() {
		let notif = JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "session/update".into(),
			params: None,
		};
		assert!(parse_notification(&notif).is_none());
	}

	#[test]
	fn build_prompt_params_basic() {
		// We can't call build_prompt_params directly without an AcpClient,
		// but we can test the public extract_text_content + types integration
		let params = SessionPromptParams {
			session_id: "test-session".into(),
			content: vec![ContentBlock::Text {
				text: "Hello".into(),
			}],
			metadata: None,
		};
		let json = serde_json::to_value(&params).unwrap();
		assert_eq!(json["sessionId"], "test-session");
		assert_eq!(json["content"][0]["type"], "text");
		assert_eq!(json["content"][0]["text"], "Hello");
		// metadata should be absent (skip_serializing_if)
		assert!(json.get("metadata").is_none());
	}

	#[test]
	fn build_prompt_params_with_metadata() {
		let params = SessionPromptParams {
			session_id: "s1".into(),
			content: vec![ContentBlock::Text {
				text: "prompt".into(),
			}],
			metadata: Some(PromptMetadata {
				agent_id: Some("agent-1".into()),
				system_prompt: Some("You are helpful".into()),
				temperature: Some(0.7),
				max_tokens: Some(1000),
				top_p: Some(0.9),
				top_k: Some(40),
				stop_sequences: vec!["STOP".into()],
			}),
		};
		let json = serde_json::to_value(&params).unwrap();
		let meta = &json["metadata"];
		assert_eq!(meta["agentId"], "agent-1");
		assert_eq!(meta["systemPrompt"], "You are helpful");
		assert_eq!(meta["temperature"], 0.7);
		assert_eq!(meta["maxTokens"], 1000);
		assert_eq!(meta["topP"], 0.9);
		assert_eq!(meta["topK"], 40);
		assert_eq!(meta["stopSequences"][0], "STOP");
	}

	#[test]
	fn acp_error_display() {
		let err = AcpError::NotInitialized;
		assert_eq!(format!("{err}"), "Not initialized");

		let err = AcpError::SessionNotFound("sess-123".into());
		assert_eq!(format!("{err}"), "Session not found: sess-123");

		let err = AcpError::Protocol("bad handshake".into());
		assert_eq!(format!("{err}"), "Protocol error: bad handshake");
	}

	#[test]
	fn generate_result_structure() {
		let result = GenerateResult {
			content: "Hello world".into(),
			stop_reason: "end_turn".into(),
			usage: Some(TokenUsage {
				prompt_tokens: 10,
				completion_tokens: 20,
			}),
		};
		assert_eq!(result.content, "Hello world");
		assert_eq!(result.stop_reason, "end_turn");
		let usage = result.usage.unwrap();
		assert_eq!(usage.prompt_tokens, 10);
		assert_eq!(usage.completion_tokens, 20);
	}

	#[test]
	fn embed_result_structure() {
		let result = EmbedResult {
			embeddings: vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
			prompt_tokens: 42,
		};
		assert_eq!(result.embeddings.len(), 2);
		assert_eq!(result.embeddings[0], vec![0.1, 0.2, 0.3]);
		assert_eq!(result.prompt_tokens, 42);
	}

	#[test]
	fn stream_event_error_variant() {
		let event = StreamEvent::Error("connection lost".into());
		match event {
			StreamEvent::Error(msg) => assert_eq!(msg, "connection lost"),
			_ => panic!("Expected Error event"),
		}
	}

	#[test]
	fn embed_data_block_format_without_model() {
		// Simulate what the embed method builds: a single data block with
		// action + texts, no agent_id in metadata
		let texts = vec!["hello".to_string(), "world".to_string()];
		let data = serde_json::json!({
			"action": "embed",
			"texts": texts,
		});
		let content = vec![ContentBlock::Data { data }];
		let params = SessionPromptParams {
			session_id: "s1".into(),
			content,
			metadata: None,
		};
		let json = serde_json::to_value(&params).unwrap();
		// metadata should be absent
		assert!(json.get("metadata").is_none());
		// data block should contain action and texts
		let block = &json["content"][0];
		assert_eq!(block["data"]["action"], "embed");
		assert_eq!(block["data"]["texts"][0], "hello");
		assert_eq!(block["data"]["texts"][1], "world");
		// model should not be present
		assert!(block["data"].get("model").is_none());
	}

	#[test]
	fn embed_data_block_format_with_model() {
		let texts = vec!["hello".to_string()];
		let mut data = serde_json::json!({
			"action": "embed",
			"texts": texts,
		});
		data["model"] = serde_json::Value::String("text-embedding-3-small".into());
		let content = vec![ContentBlock::Data { data }];
		let params = SessionPromptParams {
			session_id: "s1".into(),
			content,
			metadata: None,
		};
		let json = serde_json::to_value(&params).unwrap();
		let block = &json["content"][0];
		assert_eq!(block["data"]["action"], "embed");
		assert_eq!(block["data"]["model"], "text-embedding-3-small");
		assert_eq!(block["data"]["texts"][0], "hello");
		// No metadata with agent_id
		assert!(json.get("metadata").is_none());
	}

	#[test]
	fn prompt_metadata_sampling_params_serialize() {
		let meta = PromptMetadata {
			temperature: Some(0.5),
			top_p: Some(0.95),
			top_k: Some(50),
			stop_sequences: vec!["END".into(), "STOP".into()],
			..Default::default()
		};
		let json = serde_json::to_value(&meta).unwrap();
		assert_eq!(json["temperature"], 0.5);
		assert_eq!(json["topP"], 0.95);
		assert_eq!(json["topK"], 50);
		assert_eq!(json["stopSequences"][0], "END");
		assert_eq!(json["stopSequences"][1], "STOP");
	}

	#[test]
	fn prompt_metadata_sampling_params_skip_empty() {
		let meta = PromptMetadata::default();
		let json = serde_json::to_value(&meta).unwrap();
		// New fields should also be skipped when None/empty
		assert!(json.get("topP").is_none());
		assert!(json.get("topK").is_none());
		assert!(json.get("stopSequences").is_none());
	}

	#[test]
	fn generate_options_new_fields_default() {
		let opts = GenerateOptions::default();
		assert!(opts.top_p.is_none());
		assert!(opts.top_k.is_none());
		assert!(opts.stop_sequences.is_empty());
	}
}
