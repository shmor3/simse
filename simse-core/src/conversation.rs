//! Conversation buffer for multi-turn agentic interactions.
//!
//! Ports `src/ai/conversation/conversation.ts` + `types.ts` (~438 lines).
//!
//! Accumulates messages (user, assistant, tool results) and provides
//! compaction, serialization, trimming, and context-budget tracking.

use std::time::{SystemTime, UNIX_EPOCH};

use im::Vector;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Role
// ---------------------------------------------------------------------------

/// Message role within a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
	System,
	User,
	Assistant,
	#[serde(rename = "tool_result")]
	ToolResult,
}

// ---------------------------------------------------------------------------
// ConversationMessage
// ---------------------------------------------------------------------------

/// A single message in a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
	pub role: Role,
	pub content: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_call_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub timestamp: Option<u64>,
}

// ---------------------------------------------------------------------------
// ConversationOptions
// ---------------------------------------------------------------------------

/// Options for creating a `Conversation`.
#[derive(Debug, Clone, Default)]
pub struct ConversationOptions {
	/// Initial system prompt.
	pub system_prompt: Option<String>,
	/// Maximum number of non-system messages to keep (0 = unlimited).
	pub max_messages: Option<usize>,
	/// Approximate max character budget before auto-compact triggers.
	/// Default: 100_000 (~25k tokens).
	pub auto_compact_chars: Option<usize>,
	/// Total context window size in tokens. Used for `context_usage_percent`.
	pub context_window_tokens: Option<usize>,
}

// ---------------------------------------------------------------------------
// Conversation
// ---------------------------------------------------------------------------

/// Conversation buffer that accumulates messages for multi-turn interactions.
///
/// Tracks user messages, assistant responses, and tool results to build the
/// full conversation context for each ACP call.
///
/// Uses `im::Vector` for persistent (structural sharing) message storage,
/// enabling cheap cloning and functional-style state transitions.
#[derive(Debug, Clone)]
pub struct Conversation {
	messages: Vector<ConversationMessage>,
	system_prompt: Option<String>,
	max_messages: usize,
	auto_compact_chars: usize,
	context_window_tokens: Option<usize>,
}

/// Return the current Unix timestamp in milliseconds.
fn now_millis() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64
}

impl Conversation {
	/// Create a new conversation buffer with optional configuration.
	pub fn new(options: Option<ConversationOptions>) -> Self {
		let opts = options.unwrap_or_default();
		Self {
			messages: Vector::new(),
			system_prompt: opts.system_prompt,
			max_messages: opts.max_messages.unwrap_or(0),
			auto_compact_chars: opts.auto_compact_chars.unwrap_or(100_000),
			context_window_tokens: opts.context_window_tokens,
		}
	}

	// -- Mutators (owned-return) -------------------------------------------

	/// Add a user message. Returns the updated conversation.
	pub fn add_user(mut self, content: &str) -> Self {
		self.messages.push_back(ConversationMessage {
			role: Role::User,
			content: content.to_string(),
			tool_call_id: None,
			tool_name: None,
			timestamp: Some(now_millis()),
		});
		self.trim_if_needed()
	}

	/// Add an assistant message. Returns the updated conversation.
	pub fn add_assistant(mut self, content: &str) -> Self {
		self.messages.push_back(ConversationMessage {
			role: Role::Assistant,
			content: content.to_string(),
			tool_call_id: None,
			tool_name: None,
			timestamp: Some(now_millis()),
		});
		self.trim_if_needed()
	}

	/// Add a tool result message. Returns the updated conversation.
	pub fn add_tool_result(mut self, tool_call_id: &str, tool_name: &str, content: &str) -> Self {
		self.messages.push_back(ConversationMessage {
			role: Role::ToolResult,
			content: content.to_string(),
			tool_call_id: Some(tool_call_id.to_string()),
			tool_name: Some(tool_name.to_string()),
			timestamp: Some(now_millis()),
		});
		self.trim_if_needed()
	}

	/// Set the system prompt. Returns the updated conversation.
	pub fn set_system_prompt(mut self, prompt: String) -> Self {
		self.system_prompt = Some(prompt);
		self
	}

	/// Clear all messages (does not clear the system prompt). Returns the updated conversation.
	pub fn clear(mut self) -> Self {
		self.messages = Vector::new();
		self
	}

	/// Replace all messages with a single user summary message.
	///
	/// Used for conversation compaction: clears all existing messages and
	/// inserts a `[Conversation summary]` user message. Returns the updated conversation.
	pub fn compact(mut self, summary: &str) -> Self {
		self.messages = Vector::new();
		self.messages.push_back(ConversationMessage {
			role: Role::User,
			content: format!("[Conversation summary]\n{summary}"),
			tool_call_id: None,
			tool_name: None,
			timestamp: Some(now_millis()),
		});
		self
	}

	/// Replace all non-system messages with the provided slice.
	///
	/// System-role messages in `new_messages` are filtered out since the
	/// system prompt is managed separately via [`set_system_prompt`].
	/// Returns the updated conversation.
	pub fn replace_messages(mut self, new_messages: &[ConversationMessage]) -> Self {
		self.messages = new_messages
			.iter()
			.filter(|msg| msg.role != Role::System)
			.cloned()
			.collect();
		self
	}

	/// Load messages directly (used for restoring state). Returns the updated conversation.
	pub fn load_messages(mut self, msgs: Vec<ConversationMessage>) -> Self {
		self.messages = msgs.into_iter().collect();
		self
	}

	// -- Queries -----------------------------------------------------------

	/// Get the current system prompt, if any.
	pub fn system_prompt(&self) -> Option<&str> {
		self.system_prompt.as_deref()
	}

	/// Get a reference to the conversation messages (excludes system prompt).
	pub fn messages(&self) -> &Vector<ConversationMessage> {
		&self.messages
	}

	/// Get all messages with the system prompt prepended (if set).
	pub fn to_messages(&self) -> Vec<ConversationMessage> {
		let mut result = Vec::new();
		if let Some(ref prompt) = self.system_prompt {
			result.push(ConversationMessage {
				role: Role::System,
				content: prompt.clone(),
				tool_call_id: None,
				tool_name: None,
				timestamp: None,
			});
		}
		result.extend(self.messages.iter().cloned());
		result
	}

	/// Number of non-system messages.
	pub fn message_count(&self) -> usize {
		self.messages.len()
	}

	/// Approximate character count of the entire conversation (system prompt + messages).
	pub fn estimated_chars(&self) -> usize {
		let system_len = self.system_prompt.as_ref().map_or(0, |p| p.len());
		let msg_len: usize = self.messages.iter().map(|m| m.content.len()).sum();
		system_len + msg_len
	}

	/// Approximate token count (chars / 4 by default).
	pub fn estimated_tokens(&self) -> usize {
		self.estimated_chars().div_ceil(4)
	}

	/// Whether the conversation exceeds the auto-compact character threshold.
	pub fn needs_compaction(&self) -> bool {
		self.estimated_chars() > self.auto_compact_chars
	}

	/// Percentage of context window used (0-100). Returns 0 when
	/// `context_window_tokens` is not configured.
	pub fn context_usage_percent(&self) -> usize {
		match self.context_window_tokens {
			Some(window) if window > 0 => {
				let pct = (self.estimated_tokens() * 100) / window;
				pct.min(100)
			}
			_ => 0,
		}
	}

	/// Format the conversation as a human-readable string.
	///
	/// Each message is prefixed with its role in brackets.
	pub fn serialize(&self) -> String {
		let all = self.to_messages();
		all.iter().map(format_message).collect::<Vec<_>>().join("\n\n")
	}

	/// Export conversation as a structured JSON string (for persistence).
	pub fn to_json(&self) -> String {
		let payload = ConversationJson {
			system_prompt: self.system_prompt.clone(),
			messages: self.messages.iter().cloned().collect(),
		};
		serde_json::to_string(&payload).expect("ConversationJson serialization is infallible")
	}

	/// Replace conversation content from a JSON string produced by [`to_json`].
	/// Returns the updated conversation.
	pub fn from_json(mut self, json: &str) -> Self {
		let data: ConversationJson = match serde_json::from_str(json) {
			Ok(d) => d,
			Err(e) => {
				tracing::warn!("failed to parse conversation JSON: {e}");
				return self;
			}
		};
		if let Some(prompt) = data.system_prompt
			&& !prompt.is_empty() {
				self.system_prompt = Some(prompt);
			}
		self.messages = data
			.messages
			.into_iter()
			.filter(|msg| msg.role != Role::System)
			.collect();
		self
	}

	// -- Internal ----------------------------------------------------------

	/// Trim oldest messages when the count exceeds `max_messages`.
	fn trim_if_needed(mut self) -> Self {
		if self.max_messages > 0 && self.messages.len() > self.max_messages {
			let excess = self.messages.len() - self.max_messages;
			self.messages = self.messages.skip(excess);
		}
		self
	}
}

// ---------------------------------------------------------------------------
// Serialization helper
// ---------------------------------------------------------------------------

/// Internal struct for JSON serialization of a conversation snapshot.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationJson {
	#[serde(default)]
	system_prompt: Option<String>,
	#[serde(default)]
	messages: Vec<ConversationMessage>,
}

/// Format a single message for human-readable output.
fn format_message(msg: &ConversationMessage) -> String {
	match msg.role {
		Role::System => format!("[System]\n{}", msg.content),
		Role::User => format!("[User]\n{}", msg.content),
		Role::Assistant => format!("[Assistant]\n{}", msg.content),
		Role::ToolResult => {
			let label = msg
				.tool_name
				.as_deref()
				.or(msg.tool_call_id.as_deref())
				.unwrap_or("unknown");
			format!("[Tool Result: {label}]\n{}", msg.content)
		}
	}
}
