//! Conversation state management.
//!
//! Full conversation buffer that tracks multi-turn AI interactions.
//! Accumulates user messages, assistant responses, and tool results.
//! Used by the agentic loop (simse-bridge) to build prompts for the ACP server.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Role of a conversation message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
	System,
	User,
	Assistant,
	ToolResult,
}

/// A single conversation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationMessage {
	pub role: ConversationRole,
	pub content: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_call_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_name: Option<String>,
}

/// Options for constructing a `ConversationBuffer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationOptions {
	/// Optional system prompt to prepend to all message lists.
	pub system_prompt: Option<String>,
	/// Maximum number of non-system messages to retain (0 = unlimited).
	pub max_messages: usize,
	/// Approximate max character budget before auto-compact triggers. Default: 100_000.
	pub auto_compact_chars: usize,
}

impl Default for ConversationOptions {
	fn default() -> Self {
		Self {
			system_prompt: None,
			max_messages: 0,
			auto_compact_chars: 100_000,
		}
	}
}

// ---------------------------------------------------------------------------
// ConversationBuffer
// ---------------------------------------------------------------------------

/// Conversation buffer with auto-compaction and trimming support.
///
/// Tracks a multi-turn conversation between user, assistant, and tool results.
/// Supports a system prompt that is prepended to the output but stored separately.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationBuffer {
	system_prompt: Option<String>,
	messages: Vec<ConversationMessage>,
	max_messages: usize,
	auto_compact_chars: usize,
}

impl ConversationBuffer {
	/// Create a new conversation buffer with the given options.
	pub fn new(options: ConversationOptions) -> Self {
		Self {
			system_prompt: options.system_prompt,
			messages: Vec::new(),
			max_messages: options.max_messages,
			auto_compact_chars: options.auto_compact_chars,
		}
	}

	/// Add a user message to the conversation.
	pub fn add_user(&mut self, content: &str) {
		self.messages.push(ConversationMessage {
			role: ConversationRole::User,
			content: content.to_string(),
			tool_call_id: None,
			tool_name: None,
		});
		self.trim_if_needed();
	}

	/// Add an assistant message to the conversation.
	pub fn add_assistant(&mut self, content: &str) {
		self.messages.push(ConversationMessage {
			role: ConversationRole::Assistant,
			content: content.to_string(),
			tool_call_id: None,
			tool_name: None,
		});
		self.trim_if_needed();
	}

	/// Add a tool result message to the conversation.
	pub fn add_tool_result(&mut self, tool_call_id: &str, tool_name: &str, content: &str) {
		self.messages.push(ConversationMessage {
			role: ConversationRole::ToolResult,
			content: content.to_string(),
			tool_call_id: Some(tool_call_id.to_string()),
			tool_name: Some(tool_name.to_string()),
		});
		self.trim_if_needed();
	}

	/// Set the system prompt (replaces any existing system prompt).
	pub fn set_system_prompt(&mut self, prompt: &str) {
		self.system_prompt = Some(prompt.to_string());
	}

	/// Load messages from a saved session.
	///
	/// Clears existing messages and replays the provided list.
	/// System-role messages are extracted and used to set the system prompt.
	/// All other messages are pushed into the buffer.
	pub fn load_messages(&mut self, msgs: &[ConversationMessage]) {
		self.messages.clear();
		for msg in msgs {
			if msg.role == ConversationRole::System {
				self.system_prompt = Some(msg.content.clone());
			} else {
				self.messages.push(msg.clone());
			}
		}
	}

	/// Return all messages with the system prompt prepended (if set).
	pub fn to_messages(&self) -> Vec<ConversationMessage> {
		let mut result = Vec::new();
		if let Some(ref prompt) = self.system_prompt {
			result.push(ConversationMessage {
				role: ConversationRole::System,
				content: prompt.clone(),
				tool_call_id: None,
				tool_name: None,
			});
		}
		result.extend(self.messages.iter().cloned());
		result
	}

	/// Serialize the conversation to a human-readable string.
	///
	/// Each message is formatted as `[Role]\ncontent` and joined by double newlines.
	/// Tool results use the format `[Tool Result: {tool_name or tool_call_id}]`.
	pub fn serialize(&self) -> String {
		let all_messages = self.to_messages();
		all_messages
			.iter()
			.map(|msg| Self::format_message(msg))
			.collect::<Vec<_>>()
			.join("\n\n")
	}

	/// Clear all messages but preserve the system prompt.
	pub fn clear(&mut self) {
		self.messages.clear();
	}

	/// Replace all messages with a single user message containing the summary.
	pub fn compact(&mut self, summary: &str) {
		self.messages.clear();
		self.messages.push(ConversationMessage {
			role: ConversationRole::User,
			content: format!("[Conversation summary]\n{summary}"),
			tool_call_id: None,
			tool_name: None,
		});
	}

	/// Count of non-system messages in the buffer.
	pub fn message_count(&self) -> usize {
		self.messages.len()
	}

	/// Approximate character count of the entire conversation.
	///
	/// Includes the system prompt length plus the sum of all message content lengths.
	pub fn estimated_chars(&self) -> usize {
		let system_len = self.system_prompt.as_ref().map_or(0, |p| p.len());
		let msg_len: usize = self.messages.iter().map(|m| m.content.len()).sum();
		system_len + msg_len
	}

	/// Returns true when the estimated character count exceeds the auto-compact threshold.
	pub fn needs_compaction(&self) -> bool {
		self.estimated_chars() > self.auto_compact_chars
	}

	// -----------------------------------------------------------------------
	// Private helpers
	// -----------------------------------------------------------------------

	/// Trim oldest non-system messages when `max_messages` is exceeded.
	fn trim_if_needed(&mut self) {
		if self.max_messages == 0 {
			return;
		}
		// Count non-system messages (all messages in self.messages are non-system
		// since system prompt is stored separately).
		while self.messages.len() > self.max_messages {
			self.messages.remove(0);
		}
	}

	/// Format a single message for serialization.
	fn format_message(msg: &ConversationMessage) -> String {
		match msg.role {
			ConversationRole::System => format!("[System]\n{}", msg.content),
			ConversationRole::User => format!("[User]\n{}", msg.content),
			ConversationRole::Assistant => format!("[Assistant]\n{}", msg.content),
			ConversationRole::ToolResult => {
				let label = msg
					.tool_name
					.as_deref()
					.or(msg.tool_call_id.as_deref())
					.unwrap_or("unknown");
				format!("[Tool Result: {label}]\n{}", msg.content)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Backward-compatible type aliases and free functions
// ---------------------------------------------------------------------------

/// Alias for backward compatibility.
pub type Role = ConversationRole;

/// Alias for backward compatibility.
pub type Message = ConversationMessage;

/// Alias for backward compatibility.
pub type Conversation = ConversationBuffer;

/// Create a new conversation (backward-compatible free function).
pub fn new_conversation(
	system_prompt: Option<String>,
	max_messages: Option<usize>,
	auto_compact_chars: Option<usize>,
) -> ConversationBuffer {
	ConversationBuffer::new(ConversationOptions {
		system_prompt,
		max_messages: max_messages.unwrap_or(0),
		auto_compact_chars: auto_compact_chars.unwrap_or(100_000),
	})
}

/// Add a user message (backward-compatible free function).
pub fn add_user(conv: &mut ConversationBuffer, content: String) {
	conv.add_user(&content);
}

/// Add an assistant message (backward-compatible free function).
pub fn add_assistant(conv: &mut ConversationBuffer, content: String) {
	conv.add_assistant(&content);
}

/// Add a tool result (backward-compatible free function).
pub fn add_tool_result(
	conv: &mut ConversationBuffer,
	tool_call_id: String,
	tool_name: String,
	content: String,
) {
	conv.add_tool_result(&tool_call_id, &tool_name, &content);
}

/// Get all messages including system prompt (backward-compatible free function).
pub fn to_messages(conv: &ConversationBuffer) -> Vec<ConversationMessage> {
	conv.to_messages()
}

/// Estimated character count (backward-compatible free function).
pub fn estimated_chars(conv: &ConversationBuffer) -> usize {
	conv.estimated_chars()
}

/// Check if compaction is needed (backward-compatible free function).
pub fn needs_compaction(conv: &ConversationBuffer) -> bool {
	conv.needs_compaction()
}

/// Clear messages (backward-compatible free function).
pub fn clear(conv: &mut ConversationBuffer) {
	conv.clear();
}

/// Compact with summary (backward-compatible free function).
pub fn compact(conv: &mut ConversationBuffer, summary: String) {
	conv.compact(&summary);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	// 1. new_default_options — default state is empty
	#[test]
	fn new_default_options() {
		let buf = ConversationBuffer::new(ConversationOptions::default());
		assert_eq!(buf.message_count(), 0);
		assert!(buf.system_prompt.is_none());
		assert_eq!(buf.to_messages().len(), 0);
		assert_eq!(buf.estimated_chars(), 0);
		assert!(!buf.needs_compaction());
	}

	// 2. add_user_and_count — add user message, count is 1
	#[test]
	fn add_user_and_count() {
		let mut buf = ConversationBuffer::new(ConversationOptions::default());
		buf.add_user("Hello world");
		assert_eq!(buf.message_count(), 1);
		assert_eq!(buf.messages[0].role, ConversationRole::User);
		assert_eq!(buf.messages[0].content, "Hello world");
	}

	// 3. add_assistant — adds assistant message
	#[test]
	fn add_assistant_message() {
		let mut buf = ConversationBuffer::new(ConversationOptions::default());
		buf.add_assistant("I can help with that");
		assert_eq!(buf.message_count(), 1);
		assert_eq!(buf.messages[0].role, ConversationRole::Assistant);
		assert_eq!(buf.messages[0].content, "I can help with that");
	}

	// 4. add_tool_result_with_fields — tool_call_id and tool_name preserved
	#[test]
	fn add_tool_result_with_fields() {
		let mut buf = ConversationBuffer::new(ConversationOptions::default());
		buf.add_tool_result("call_123", "read_file", "file contents here");
		assert_eq!(buf.message_count(), 1);
		let msg = &buf.messages[0];
		assert_eq!(msg.role, ConversationRole::ToolResult);
		assert_eq!(msg.content, "file contents here");
		assert_eq!(msg.tool_call_id.as_deref(), Some("call_123"));
		assert_eq!(msg.tool_name.as_deref(), Some("read_file"));
	}

	// 5. serialize_format — verify serialize produces correct [Role]\ncontent\n\n... format
	#[test]
	fn serialize_format() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			system_prompt: Some("You are helpful.".to_string()),
			..ConversationOptions::default()
		});
		buf.add_user("Hello");
		buf.add_assistant("Hi there");
		buf.add_tool_result("tc1", "bash", "output");

		let serialized = buf.serialize();
		let expected = "[System]\nYou are helpful.\n\n\
		               [User]\nHello\n\n\
		               [Assistant]\nHi there\n\n\
		               [Tool Result: bash]\noutput";
		assert_eq!(serialized, expected);
	}

	// 6. compact_replaces_messages — compact clears and inserts summary
	#[test]
	fn compact_replaces_messages() {
		let mut buf = ConversationBuffer::new(ConversationOptions::default());
		buf.add_user("msg1");
		buf.add_assistant("msg2");
		buf.add_user("msg3");
		buf.compact("This is a summary of the conversation.");

		assert_eq!(buf.message_count(), 1);
		assert_eq!(buf.messages[0].role, ConversationRole::User);
		assert_eq!(
			buf.messages[0].content,
			"[Conversation summary]\nThis is a summary of the conversation."
		);
	}

	// 7. needs_compaction_threshold — returns true when chars exceed threshold
	#[test]
	fn needs_compaction_threshold() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			auto_compact_chars: 10,
			..ConversationOptions::default()
		});
		assert!(!buf.needs_compaction());

		buf.add_user(&"a".repeat(20));
		assert!(buf.needs_compaction());
	}

	// 8. trim_oldest_when_max_exceeded — with max_messages=2, adding 3 messages trims oldest
	#[test]
	fn trim_oldest_when_max_exceeded() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			max_messages: 2,
			..ConversationOptions::default()
		});
		buf.add_user("first");
		buf.add_user("second");
		buf.add_user("third");

		assert_eq!(buf.message_count(), 2);
		assert_eq!(buf.messages[0].content, "second");
		assert_eq!(buf.messages[1].content, "third");
	}

	// 9. load_messages_extracts_system — system messages become system_prompt
	#[test]
	fn load_messages_extracts_system() {
		let mut buf = ConversationBuffer::new(ConversationOptions::default());

		let msgs = vec![
			ConversationMessage {
				role: ConversationRole::System,
				content: "Be helpful".to_string(),
				tool_call_id: None,
				tool_name: None,
			},
			ConversationMessage {
				role: ConversationRole::User,
				content: "Hello".to_string(),
				tool_call_id: None,
				tool_name: None,
			},
			ConversationMessage {
				role: ConversationRole::Assistant,
				content: "Hi".to_string(),
				tool_call_id: None,
				tool_name: None,
			},
		];

		buf.load_messages(&msgs);

		assert_eq!(buf.system_prompt.as_deref(), Some("Be helpful"));
		assert_eq!(buf.message_count(), 2);
		assert_eq!(buf.messages[0].role, ConversationRole::User);
		assert_eq!(buf.messages[1].role, ConversationRole::Assistant);
	}

	// 10. clear_preserves_system_prompt — clear removes messages but keeps system_prompt
	#[test]
	fn clear_preserves_system_prompt() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			system_prompt: Some("System prompt here".to_string()),
			..ConversationOptions::default()
		});
		buf.add_user("hello");
		buf.add_assistant("hi");

		assert_eq!(buf.message_count(), 2);
		buf.clear();

		assert_eq!(buf.message_count(), 0);
		assert_eq!(buf.system_prompt.as_deref(), Some("System prompt here"));
	}

	// 11. to_messages_includes_system — system prompt prepended to output
	#[test]
	fn to_messages_includes_system() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			system_prompt: Some("You are an AI.".to_string()),
			..ConversationOptions::default()
		});
		buf.add_user("Hello");

		let msgs = buf.to_messages();
		assert_eq!(msgs.len(), 2);
		assert_eq!(msgs[0].role, ConversationRole::System);
		assert_eq!(msgs[0].content, "You are an AI.");
		assert_eq!(msgs[1].role, ConversationRole::User);
		assert_eq!(msgs[1].content, "Hello");
	}

	// 12. set_system_prompt — replaces existing system prompt
	#[test]
	fn set_system_prompt_replaces() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			system_prompt: Some("Old prompt".to_string()),
			..ConversationOptions::default()
		});
		buf.set_system_prompt("New prompt");
		assert_eq!(buf.system_prompt.as_deref(), Some("New prompt"));

		let msgs = buf.to_messages();
		assert_eq!(msgs[0].content, "New prompt");
	}

	// 13. estimated_chars_includes_system — system prompt counted in estimated chars
	#[test]
	fn estimated_chars_includes_system() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			system_prompt: Some("12345".to_string()),
			..ConversationOptions::default()
		});
		buf.add_user("abc"); // 3 chars

		assert_eq!(buf.estimated_chars(), 8); // 5 + 3
	}

	// 14. serialize_tool_result_fallback — tool result uses tool_call_id when tool_name is None
	#[test]
	fn serialize_tool_result_fallback() {
		let mut buf = ConversationBuffer::new(ConversationOptions::default());
		buf.messages.push(ConversationMessage {
			role: ConversationRole::ToolResult,
			content: "result data".to_string(),
			tool_call_id: Some("tc_abc".to_string()),
			tool_name: None,
		});

		let serialized = buf.serialize();
		assert_eq!(serialized, "[Tool Result: tc_abc]\nresult data");
	}

	// 15. backward compat — old free functions still work
	#[test]
	fn backward_compat_free_functions() {
		let mut conv = new_conversation(Some("sys".into()), None, None);
		add_user(&mut conv, "hello".into());
		add_assistant(&mut conv, "hi".into());
		add_tool_result(&mut conv, "tc1".into(), "bash".into(), "output".into());

		let msgs = to_messages(&conv);
		assert_eq!(msgs.len(), 4); // system + 3 messages
		assert_eq!(msgs[0].role, ConversationRole::System);

		assert_eq!(estimated_chars(&conv), 3 + 5 + 2 + 6); // sys + hello + hi + output
		assert!(!needs_compaction(&conv));

		clear(&mut conv);
		assert_eq!(conv.message_count(), 0);
	}

	// 16. compact_backward_compat — old compact function
	#[test]
	fn compact_backward_compat() {
		let mut conv = new_conversation(None, None, None);
		add_user(&mut conv, "msg1".into());
		compact(&mut conv, "summary".into());

		let msgs = to_messages(&conv);
		assert_eq!(msgs.len(), 1);
		assert!(msgs[0].content.contains("[Conversation summary]"));
		assert!(msgs[0].content.contains("summary"));
	}

	// 17. empty serialize — no messages produces empty string
	#[test]
	fn serialize_empty() {
		let buf = ConversationBuffer::new(ConversationOptions::default());
		assert_eq!(buf.serialize(), "");
	}

	// 18. trim does not affect count when below max
	#[test]
	fn trim_no_op_when_below_max() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			max_messages: 5,
			..ConversationOptions::default()
		});
		buf.add_user("one");
		buf.add_user("two");
		assert_eq!(buf.message_count(), 2);
	}

	// 19. load_messages clears existing messages
	#[test]
	fn load_messages_clears_existing() {
		let mut buf = ConversationBuffer::new(ConversationOptions::default());
		buf.add_user("old message");
		assert_eq!(buf.message_count(), 1);

		buf.load_messages(&[ConversationMessage {
			role: ConversationRole::User,
			content: "new message".to_string(),
			tool_call_id: None,
			tool_name: None,
		}]);

		assert_eq!(buf.message_count(), 1);
		assert_eq!(buf.messages[0].content, "new message");
	}

	// 20. needs_compaction false when equal to threshold
	#[test]
	fn needs_compaction_false_at_threshold() {
		let mut buf = ConversationBuffer::new(ConversationOptions {
			auto_compact_chars: 5,
			..ConversationOptions::default()
		});
		buf.add_user("12345"); // exactly 5 chars
		assert!(!buf.needs_compaction()); // not strictly greater
	}
}
