//! Conversation state management.

use serde::{Deserialize, Serialize};

/// Role of a conversation message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
	System,
	User,
	Assistant,
	ToolResult,
}

/// A single conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
	pub role: Role,
	pub content: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_call_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_name: Option<String>,
}

/// Conversation buffer with auto-compaction support.
#[derive(Debug, Clone)]
pub struct Conversation {
	pub system_prompt: Option<String>,
	pub messages: Vec<Message>,
	pub max_messages: Option<usize>,
	pub auto_compact_chars: usize,
}

/// Create a new conversation.
pub fn new_conversation(
	system_prompt: Option<String>,
	max_messages: Option<usize>,
	auto_compact_chars: Option<usize>,
) -> Conversation {
	Conversation {
		system_prompt,
		messages: Vec::new(),
		max_messages,
		auto_compact_chars: auto_compact_chars.unwrap_or(100_000),
	}
}

pub fn add_user(conv: &mut Conversation, content: String) {
	conv.messages.push(Message {
		role: Role::User,
		content,
		tool_call_id: None,
		tool_name: None,
	});
}

pub fn add_assistant(conv: &mut Conversation, content: String) {
	conv.messages.push(Message {
		role: Role::Assistant,
		content,
		tool_call_id: None,
		tool_name: None,
	});
}

pub fn add_tool_result(
	conv: &mut Conversation,
	tool_call_id: String,
	tool_name: String,
	content: String,
) {
	conv.messages.push(Message {
		role: Role::ToolResult,
		content,
		tool_call_id: Some(tool_call_id),
		tool_name: Some(tool_name),
	});
}

pub fn to_messages(conv: &Conversation) -> Vec<Message> {
	let mut msgs = Vec::new();
	if let Some(ref prompt) = conv.system_prompt {
		msgs.push(Message {
			role: Role::System,
			content: prompt.clone(),
			tool_call_id: None,
			tool_name: None,
		});
	}
	msgs.extend(conv.messages.iter().cloned());
	msgs
}

pub fn estimated_chars(conv: &Conversation) -> usize {
	conv.messages.iter().map(|m| m.content.len()).sum()
}

pub fn needs_compaction(conv: &Conversation) -> bool {
	estimated_chars(conv) > conv.auto_compact_chars
}

pub fn clear(conv: &mut Conversation) {
	conv.messages.clear();
}

pub fn compact(conv: &mut Conversation, summary: String) {
	conv.messages.clear();
	conv.messages.push(Message {
		role: Role::Assistant,
		content: summary,
		tool_call_id: None,
		tool_name: None,
	});
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn new_conversation_is_empty() {
		let conv = new_conversation(None, None, None);
		assert_eq!(to_messages(&conv).len(), 0);
	}

	#[test]
	fn new_conversation_with_system_prompt() {
		let conv = new_conversation(Some("You are helpful".into()), None, None);
		let msgs = to_messages(&conv);
		assert_eq!(msgs.len(), 1);
		assert_eq!(msgs[0].role, Role::System);
	}

	#[test]
	fn add_messages() {
		let mut conv = new_conversation(None, None, None);
		add_user(&mut conv, "hello".into());
		add_assistant(&mut conv, "hi".into());
		let msgs = to_messages(&conv);
		assert_eq!(msgs.len(), 2);
		assert_eq!(msgs[0].role, Role::User);
		assert_eq!(msgs[1].role, Role::Assistant);
	}

	#[test]
	fn compaction_replaces_messages() {
		let mut conv = new_conversation(None, None, None);
		add_user(&mut conv, "msg1".into());
		add_assistant(&mut conv, "msg2".into());
		compact(&mut conv, "summary".into());
		let msgs = to_messages(&conv);
		assert_eq!(msgs.len(), 1);
		assert_eq!(msgs[0].content, "summary");
	}

	#[test]
	fn needs_compaction_threshold() {
		let mut conv = new_conversation(None, None, Some(10));
		add_user(&mut conv, "a".repeat(20));
		assert!(needs_compaction(&conv));
	}
}
