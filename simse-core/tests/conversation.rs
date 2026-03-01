use simse_core::conversation::*;

#[test]
fn test_add_and_retrieve_messages() {
	let mut conv = Conversation::new(None);
	conv.add_user("hello");
	conv.add_assistant("hi there");
	let msgs = conv.messages();
	assert_eq!(msgs.len(), 2);
	assert_eq!(msgs[0].role, Role::User);
	assert_eq!(msgs[1].role, Role::Assistant);
}

#[test]
fn test_system_prompt() {
	let mut conv = Conversation::new(None);
	conv.set_system_prompt("you are helpful".into());
	let all = conv.to_messages();
	assert_eq!(all[0].role, Role::System);
	assert_eq!(all[0].content, "you are helpful");
}

#[test]
fn test_tool_result() {
	let mut conv = Conversation::new(None);
	conv.add_tool_result("call_1", "search", "found 3 results");
	let msgs = conv.messages();
	assert_eq!(msgs[0].role, Role::ToolResult);
	assert_eq!(msgs[0].tool_call_id.as_deref(), Some("call_1"));
}

#[test]
fn test_max_messages_trimming() {
	let mut conv = Conversation::new(Some(ConversationOptions {
		max_messages: Some(2),
		..Default::default()
	}));
	conv.add_user("1");
	conv.add_assistant("2");
	conv.add_user("3");
	// Oldest non-system message should be trimmed
	assert_eq!(conv.message_count(), 2);
	assert_eq!(conv.messages()[0].content, "2");
}

#[test]
fn test_compact() {
	let mut conv = Conversation::new(None);
	conv.add_user("first");
	conv.add_assistant("response");
	conv.compact("summary of conversation");
	assert_eq!(conv.message_count(), 1);
	assert!(conv.messages()[0].content.contains("summary"));
}

#[test]
fn test_serialize_deserialize() {
	let mut conv = Conversation::new(None);
	conv.set_system_prompt("system".into());
	conv.add_user("hello");
	conv.add_assistant("world");
	let json = conv.to_json();
	let mut conv2 = Conversation::new(None);
	conv2.from_json(&json);
	assert_eq!(conv2.messages().len(), 2);
	assert_eq!(conv2.system_prompt(), Some("system"));
}

#[test]
fn test_needs_compaction() {
	let mut conv = Conversation::new(Some(ConversationOptions {
		auto_compact_chars: Some(50),
		..Default::default()
	}));
	conv.add_user(&"x".repeat(60));
	assert!(conv.needs_compaction());
}

#[test]
fn test_estimated_chars() {
	let mut conv = Conversation::new(None);
	conv.set_system_prompt("abc".into());
	conv.add_user("defgh");
	assert_eq!(conv.estimated_chars(), 8); // 3 + 5
}

#[test]
fn test_clear() {
	let mut conv = Conversation::new(None);
	conv.add_user("test");
	conv.clear();
	assert_eq!(conv.message_count(), 0);
}

#[test]
fn test_replace_messages() {
	let mut conv = Conversation::new(None);
	conv.add_user("old");
	let new_msgs = vec![ConversationMessage {
		role: Role::User,
		content: "new".into(),
		tool_call_id: None,
		tool_name: None,
		timestamp: None,
	}];
	conv.replace_messages(&new_msgs);
	assert_eq!(conv.messages()[0].content, "new");
}

#[test]
fn test_load_messages() {
	let mut conv = Conversation::new(None);
	let msgs = vec![ConversationMessage {
		role: Role::User,
		content: "loaded".into(),
		tool_call_id: None,
		tool_name: None,
		timestamp: Some(12345),
	}];
	conv.load_messages(msgs);
	assert_eq!(conv.messages()[0].content, "loaded");
}

#[test]
fn test_estimated_tokens_default() {
	let mut conv = Conversation::new(None);
	conv.add_user("abcdefgh"); // 8 chars => ceil(8/4) = 2 tokens
	assert_eq!(conv.estimated_tokens(), 2);
}

#[test]
fn test_context_usage_percent_no_window() {
	let conv = Conversation::new(None);
	// No contextWindowTokens configured => 0
	assert_eq!(conv.context_usage_percent(), 0);
}

#[test]
fn test_context_usage_percent_with_window() {
	let mut conv = Conversation::new(Some(ConversationOptions {
		context_window_tokens: Some(100),
		..Default::default()
	}));
	// 40 chars => 10 tokens => 10% of 100
	conv.add_user(&"a".repeat(40));
	assert_eq!(conv.context_usage_percent(), 10);
}

#[test]
fn test_context_usage_percent_capped_at_100() {
	let mut conv = Conversation::new(Some(ConversationOptions {
		context_window_tokens: Some(10),
		..Default::default()
	}));
	// 400 chars => 100 tokens => 1000% => capped to 100
	conv.add_user(&"a".repeat(400));
	assert_eq!(conv.context_usage_percent(), 100);
}

#[test]
fn test_serialize_format() {
	let mut conv = Conversation::new(None);
	conv.set_system_prompt("sys".into());
	conv.add_user("hello");
	conv.add_assistant("world");
	let s = conv.serialize();
	assert!(s.contains("[System]"));
	assert!(s.contains("[User]"));
	assert!(s.contains("[Assistant]"));
	assert!(s.contains("sys"));
	assert!(s.contains("hello"));
	assert!(s.contains("world"));
}

#[test]
fn test_serialize_tool_result_format() {
	let mut conv = Conversation::new(None);
	conv.add_tool_result("id1", "search", "found it");
	let s = conv.serialize();
	assert!(s.contains("[Tool Result: search]"));
	assert!(s.contains("found it"));
}

#[test]
fn test_to_messages_prepends_system() {
	let mut conv = Conversation::new(None);
	conv.set_system_prompt("be helpful".into());
	conv.add_user("hi");
	let all = conv.to_messages();
	assert_eq!(all.len(), 2);
	assert_eq!(all[0].role, Role::System);
	assert_eq!(all[1].role, Role::User);
}

#[test]
fn test_to_messages_no_system() {
	let mut conv = Conversation::new(None);
	conv.add_user("hi");
	let all = conv.to_messages();
	assert_eq!(all.len(), 1);
	assert_eq!(all[0].role, Role::User);
}

#[test]
fn test_trim_preserves_system_messages() {
	let mut conv = Conversation::new(Some(ConversationOptions {
		max_messages: Some(2),
		..Default::default()
	}));
	conv.set_system_prompt("system prompt".into());
	conv.add_user("1");
	conv.add_assistant("2");
	conv.add_user("3");
	// system prompt is separate, non-system messages trimmed to 2
	assert_eq!(conv.message_count(), 2);
	assert_eq!(conv.messages()[0].content, "2");
	assert_eq!(conv.messages()[1].content, "3");
}

#[test]
fn test_compact_creates_user_summary() {
	let mut conv = Conversation::new(None);
	conv.add_user("a");
	conv.add_assistant("b");
	conv.compact("the summary");
	let msgs = conv.messages();
	assert_eq!(msgs.len(), 1);
	assert_eq!(msgs[0].role, Role::User);
	assert_eq!(msgs[0].content, "[Conversation summary]\nthe summary");
}

#[test]
fn test_from_json_skips_system_messages() {
	let json = r#"{"systemPrompt":"sys","messages":[{"role":"system","content":"should skip"},{"role":"user","content":"keep me"}]}"#;
	let mut conv = Conversation::new(None);
	conv.from_json(json);
	assert_eq!(conv.messages().len(), 1);
	assert_eq!(conv.messages()[0].role, Role::User);
	assert_eq!(conv.system_prompt(), Some("sys"));
}

#[test]
fn test_replace_messages_filters_system() {
	let mut conv = Conversation::new(None);
	let new_msgs = vec![
		ConversationMessage {
			role: Role::System,
			content: "should be filtered".into(),
			tool_call_id: None,
			tool_name: None,
			timestamp: None,
		},
		ConversationMessage {
			role: Role::User,
			content: "kept".into(),
			tool_call_id: None,
			tool_name: None,
			timestamp: None,
		},
	];
	conv.replace_messages(&new_msgs);
	assert_eq!(conv.messages().len(), 1);
	assert_eq!(conv.messages()[0].content, "kept");
}

#[test]
fn test_to_json_roundtrip_with_tool_result() {
	let mut conv = Conversation::new(None);
	conv.add_tool_result("tc_1", "my_tool", "result content");
	let json = conv.to_json();
	let mut conv2 = Conversation::new(None);
	conv2.from_json(&json);
	let msgs = conv2.messages();
	assert_eq!(msgs.len(), 1);
	assert_eq!(msgs[0].role, Role::ToolResult);
	assert_eq!(msgs[0].tool_call_id.as_deref(), Some("tc_1"));
	assert_eq!(msgs[0].tool_name.as_deref(), Some("my_tool"));
	assert_eq!(msgs[0].content, "result content");
}

#[test]
fn test_needs_compaction_default_threshold() {
	let mut conv = Conversation::new(None);
	// Default auto_compact_chars is 100_000
	conv.add_user(&"x".repeat(50_000));
	assert!(!conv.needs_compaction());
	conv.add_user(&"y".repeat(60_000));
	assert!(conv.needs_compaction());
}

#[test]
fn test_initial_system_prompt_from_options() {
	let conv = Conversation::new(Some(ConversationOptions {
		system_prompt: Some("from options".into()),
		..Default::default()
	}));
	assert_eq!(conv.system_prompt(), Some("from options"));
}

#[test]
fn test_message_timestamps() {
	let mut conv = Conversation::new(None);
	conv.add_user("hello");
	let msgs = conv.messages();
	// Timestamp should be set (non-None)
	assert!(msgs[0].timestamp.is_some());
}
