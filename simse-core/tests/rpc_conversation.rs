//! Integration tests for conversation JSON-RPC handlers.
//!
//! Tests exercise conversation operations through the `SessionManager` and
//! `CoreContext` API, mirroring the handler logic in `rpc_server.rs`. Dispatch
//! routing tests verify handlers can be invoked without panics.

use simse_core::config::AppConfig;
use simse_core::context::CoreContext;
use simse_core::conversation::Role;
use simse_core::rpc_protocol::JsonRpcRequest;
use simse_core::rpc_server::CoreRpcServer;
use simse_core::rpc_transport::NdjsonTransport;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_ctx() -> CoreContext {
	CoreContext::new(AppConfig::default())
}

fn make_server() -> CoreRpcServer {
	CoreRpcServer::new(NdjsonTransport::new())
}

/// Create a CoreContext and a session, returning both the context and session ID.
fn ctx_with_session() -> (CoreContext, String) {
	let ctx = make_ctx();
	let id = ctx.session_manager.create();
	(ctx, id)
}

/// Initialize a server and create a session, returning the server.
/// (We cannot capture the session ID from dispatch output, so dispatch tests
/// are limited to verifying no-panic behavior.)
async fn init_server() -> CoreRpcServer {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	server
}

// ---------------------------------------------------------------------------
// conversation/addUser
// ---------------------------------------------------------------------------

#[test]
fn conv_add_user_message() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.add_user("Hello, world!"), ())
	});

	let info = ctx.session_manager.get_info(&id).unwrap();
	assert_eq!(info.message_count, 1);

	ctx.session_manager.with_session(&id, |session| {
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 1);
		assert_eq!(msgs[0].role, Role::User);
		assert_eq!(msgs[0].content, "Hello, world!");
		assert!(msgs[0].timestamp.is_some());
	});
}

#[test]
fn conv_add_user_nonexistent_session() {
	let ctx = make_ctx();
	let result = ctx.session_manager.with_state_transition("nonexistent", |conv| {
		(conv.add_user("test"), ())
	});
	assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// conversation/addAssistant
// ---------------------------------------------------------------------------

#[test]
fn conv_add_assistant_message() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.add_assistant("I can help with that."), ())
	});

	ctx.session_manager.with_session(&id, |session| {
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 1);
		assert_eq!(msgs[0].role, Role::Assistant);
		assert_eq!(msgs[0].content, "I can help with that.");
	});
}

// ---------------------------------------------------------------------------
// conversation/addToolResult
// ---------------------------------------------------------------------------

#[test]
fn conv_add_tool_result() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.add_tool_result("call_123", "search", "Found 5 results"), ())
	});

	ctx.session_manager.with_session(&id, |session| {
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 1);
		assert_eq!(msgs[0].role, Role::ToolResult);
		assert_eq!(msgs[0].content, "Found 5 results");
		assert_eq!(msgs[0].tool_call_id.as_deref(), Some("call_123"));
		assert_eq!(msgs[0].tool_name.as_deref(), Some("search"));
	});
}

// ---------------------------------------------------------------------------
// conversation/setSystemPrompt + getMessages
// ---------------------------------------------------------------------------

#[test]
fn conv_set_and_get_system_prompt() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.set_system_prompt("You are a helpful assistant.".to_string()), ())
	});

	ctx.session_manager.with_session(&id, |session| {
		assert_eq!(
			session.conversation.system_prompt(),
			Some("You are a helpful assistant.")
		);
	});
}

#[test]
fn conv_get_messages_excludes_system_prompt() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.set_system_prompt("System prompt".to_string());
		let conv = conv.add_user("Hello");
		(conv, ())
	});

	ctx.session_manager.with_session(&id, |session| {
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 1);
		assert_eq!(msgs[0].role, Role::User);
		// system_prompt is separate
		assert_eq!(session.conversation.system_prompt(), Some("System prompt"));
	});
}

// ---------------------------------------------------------------------------
// conversation/compact
// ---------------------------------------------------------------------------

#[test]
fn conv_compact_replaces_messages() {
	let (ctx, id) = ctx_with_session();

	// Add several messages
	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.add_user("msg 1");
		let conv = conv.add_assistant("msg 2");
		let conv = conv.add_user("msg 3");
		(conv, ())
	});

	// Compact
	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.compact("Summary of the conversation so far."), ())
	});

	ctx.session_manager.with_session(&id, |session| {
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 1);
		assert_eq!(msgs[0].role, Role::User);
		assert!(msgs[0].content.contains("[Conversation summary]"));
		assert!(msgs[0]
			.content
			.contains("Summary of the conversation so far."));
	});
}

// ---------------------------------------------------------------------------
// conversation/clear
// ---------------------------------------------------------------------------

#[test]
fn conv_clear_removes_messages_but_keeps_system_prompt() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.set_system_prompt("Keep me".to_string());
		let conv = conv.add_user("Hello");
		let conv = conv.add_assistant("World");
		(conv, ())
	});

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.clear(), ())
	});

	ctx.session_manager.with_session(&id, |session| {
		assert_eq!(session.conversation.messages().len(), 0);
		assert_eq!(session.conversation.system_prompt(), Some("Keep me"));
	});
}

// ---------------------------------------------------------------------------
// conversation/stats
// ---------------------------------------------------------------------------

#[test]
fn conv_stats_empty_conversation() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_session(&id, |session| {
		assert_eq!(session.conversation.estimated_chars(), 0);
		assert_eq!(session.conversation.estimated_tokens(), 0);
		assert!(!session.conversation.needs_compaction());
		assert_eq!(session.conversation.context_usage_percent(), 0);
	});
}

#[test]
fn conv_stats_with_messages() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.set_system_prompt("System".to_string());
		let conv = conv.add_user("Hello there, this is a test message");
		(conv, ())
	});

	ctx.session_manager.with_session(&id, |session| {
		let chars = session.conversation.estimated_chars();
		assert!(chars > 0);
		let tokens = session.conversation.estimated_tokens();
		// tokens = chars.div_ceil(4)
		assert_eq!(tokens, chars.div_ceil(4));
		assert!(!session.conversation.needs_compaction());
	});
}

#[test]
fn conv_stats_context_usage_percent() {
	let ctx = make_ctx();
	let id = ctx.session_manager.create();

	// We need a conversation with context_window_tokens set.
	// Since Conversation options are set at creation, we use from_json trick
	// or just add enough messages. But context_window_tokens defaults to None
	// so context_usage_percent returns 0.
	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.add_user("test");
		(conv, ())
	});
	ctx.session_manager.with_session(&id, |session| {
		// With default options, context_usage_percent is 0 (no window configured)
		assert_eq!(session.conversation.context_usage_percent(), 0);
	});
}

// ---------------------------------------------------------------------------
// conversation/toJson + fromJson round-trip
// ---------------------------------------------------------------------------

#[test]
fn conv_json_round_trip() {
	let (ctx, id) = ctx_with_session();

	// Build up a conversation
	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.set_system_prompt("Be helpful".to_string());
		let conv = conv.add_user("Question 1");
		let conv = conv.add_assistant("Answer 1");
		let conv = conv.add_tool_result("tc_1", "search", "results");
		(conv, ())
	});

	// Export to JSON
	let json = ctx
		.session_manager
		.with_session(&id, |session| session.conversation.to_json())
		.unwrap();

	// Create a new session and import from JSON
	let id2 = ctx.session_manager.create();
	ctx.session_manager.with_state_transition(&id2, |conv| {
		(conv.from_json(&json), ())
	});

	// Verify the imported session matches
	ctx.session_manager.with_session(&id2, |session| {
		assert_eq!(
			session.conversation.system_prompt(),
			Some("Be helpful")
		);
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 3);
		assert_eq!(msgs[0].role, Role::User);
		assert_eq!(msgs[0].content, "Question 1");
		assert_eq!(msgs[1].role, Role::Assistant);
		assert_eq!(msgs[1].content, "Answer 1");
		assert_eq!(msgs[2].role, Role::ToolResult);
		assert_eq!(msgs[2].content, "results");
		assert_eq!(msgs[2].tool_call_id.as_deref(), Some("tc_1"));
		assert_eq!(msgs[2].tool_name.as_deref(), Some("search"));
	});
}

#[test]
fn conv_from_json_invalid_json_is_noop() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.add_user("existing"), ())
	});

	// Import invalid JSON — should be a no-op (Conversation::from_json logs
	// a warning but does not panic or clear messages)
	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.from_json("not valid json{{{"), ())
	});

	// The original message should remain (from_json only clears on successful parse)
	// Actually, from_json returns early on error, so messages stay
	ctx.session_manager.with_session(&id, |session| {
		assert_eq!(session.conversation.messages().len(), 1);
		assert_eq!(session.conversation.messages()[0].content, "existing");
	});
}

// ---------------------------------------------------------------------------
// Multi-turn conversation flow
// ---------------------------------------------------------------------------

#[test]
fn conv_full_multi_turn_flow() {
	let (ctx, id) = ctx_with_session();

	// Set system prompt
	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.set_system_prompt("You are a coding assistant.".to_string()), ())
	});

	// Turn 1
	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.add_user("Write a function");
		let conv = conv.add_assistant("Here is the function...");
		(conv, ())
	});

	// Turn 2 with tool use
	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.add_user("Run the tests");
		let conv = conv.add_tool_result("tc_run", "run_tests", "All 5 tests passed");
		let conv = conv.add_assistant("All tests passed!");
		(conv, ())
	});

	let info = ctx.session_manager.get_info(&id).unwrap();
	assert_eq!(info.message_count, 5);

	// Verify ordering
	ctx.session_manager.with_session(&id, |session| {
		let msgs = session.conversation.messages();
		assert_eq!(msgs[0].role, Role::User);
		assert_eq!(msgs[1].role, Role::Assistant);
		assert_eq!(msgs[2].role, Role::User);
		assert_eq!(msgs[3].role, Role::ToolResult);
		assert_eq!(msgs[4].role, Role::Assistant);
	});

	// Export, compact, verify
	let json_before = ctx
		.session_manager
		.with_session(&id, |session| session.conversation.to_json())
		.unwrap();

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.compact("Wrote and tested a function."), ())
	});

	let info_after = ctx.session_manager.get_info(&id).unwrap();
	assert_eq!(info_after.message_count, 1);

	// Restore from saved JSON
	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.from_json(&json_before), ())
	});

	let info_restored = ctx.session_manager.get_info(&id).unwrap();
	assert_eq!(info_restored.message_count, 5);
}

// ---------------------------------------------------------------------------
// Conversation messages serialization (for getMessages handler)
// ---------------------------------------------------------------------------

#[test]
fn conv_messages_serialize_to_camel_case() {
	let (ctx, id) = ctx_with_session();

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.add_tool_result("tc_1", "my_tool", "output"), ())
	});

	ctx.session_manager.with_session(&id, |session| {
		let msgs = session.conversation.messages();
		let json = serde_json::to_value(msgs).unwrap();
		let first = &json[0];

		// Verify camelCase field names (from #[serde(rename_all = "camelCase")])
		assert!(first.get("toolCallId").is_some());
		assert!(first.get("toolName").is_some());
		assert_eq!(first["toolCallId"], "tc_1");
		assert_eq!(first["toolName"], "my_tool");
		assert_eq!(first["role"], "tool_result");
		assert_eq!(first["content"], "output");
	});
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — verify no panics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_conv_add_user_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/addUser".to_string(),
			params: serde_json::json!({ "sessionId": "x", "content": "hi" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_add_assistant_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/addAssistant".to_string(),
			params: serde_json::json!({ "sessionId": "x", "content": "hi" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_add_tool_result_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/addToolResult".to_string(),
			params: serde_json::json!({
				"sessionId": "x",
				"toolCallId": "tc_1",
				"content": "result"
			}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_set_system_prompt_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/setSystemPrompt".to_string(),
			params: serde_json::json!({ "sessionId": "x", "prompt": "be nice" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_get_messages_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/getMessages".to_string(),
			params: serde_json::json!({ "sessionId": "x" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_compact_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/compact".to_string(),
			params: serde_json::json!({ "sessionId": "x", "summary": "sum" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_clear_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/clear".to_string(),
			params: serde_json::json!({ "sessionId": "x" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_stats_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/stats".to_string(),
			params: serde_json::json!({ "sessionId": "x" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_to_json_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/toJson".to_string(),
			params: serde_json::json!({ "sessionId": "x" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_from_json_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "conversation/fromJson".to_string(),
			params: serde_json::json!({ "sessionId": "x", "json": "{}" }),
		})
		.await;
}

// ---------------------------------------------------------------------------
// Dispatch routing after init — all conversation methods
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_all_conv_methods_after_init() {
	let mut server = init_server().await;

	// Create a session first
	server
		.dispatch(JsonRpcRequest {
			id: 10,
			method: "session/create".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// We can't capture the session ID from dispatch, so use a fake one.
	// The handlers will return SESSION_NOT_FOUND but should not panic.
	let sid = "fake_session";

	// addUser
	server
		.dispatch(JsonRpcRequest {
			id: 11,
			method: "conversation/addUser".to_string(),
			params: serde_json::json!({ "sessionId": sid, "content": "hello" }),
		})
		.await;

	// addAssistant
	server
		.dispatch(JsonRpcRequest {
			id: 12,
			method: "conversation/addAssistant".to_string(),
			params: serde_json::json!({ "sessionId": sid, "content": "hi" }),
		})
		.await;

	// addToolResult
	server
		.dispatch(JsonRpcRequest {
			id: 13,
			method: "conversation/addToolResult".to_string(),
			params: serde_json::json!({
				"sessionId": sid,
				"toolCallId": "tc_1",
				"toolName": "search",
				"content": "results"
			}),
		})
		.await;

	// setSystemPrompt
	server
		.dispatch(JsonRpcRequest {
			id: 14,
			method: "conversation/setSystemPrompt".to_string(),
			params: serde_json::json!({ "sessionId": sid, "prompt": "be helpful" }),
		})
		.await;

	// getMessages
	server
		.dispatch(JsonRpcRequest {
			id: 15,
			method: "conversation/getMessages".to_string(),
			params: serde_json::json!({ "sessionId": sid }),
		})
		.await;

	// compact
	server
		.dispatch(JsonRpcRequest {
			id: 16,
			method: "conversation/compact".to_string(),
			params: serde_json::json!({ "sessionId": sid, "summary": "summary" }),
		})
		.await;

	// clear
	server
		.dispatch(JsonRpcRequest {
			id: 17,
			method: "conversation/clear".to_string(),
			params: serde_json::json!({ "sessionId": sid }),
		})
		.await;

	// stats
	server
		.dispatch(JsonRpcRequest {
			id: 18,
			method: "conversation/stats".to_string(),
			params: serde_json::json!({ "sessionId": sid }),
		})
		.await;

	// toJson
	server
		.dispatch(JsonRpcRequest {
			id: 19,
			method: "conversation/toJson".to_string(),
			params: serde_json::json!({ "sessionId": sid }),
		})
		.await;

	// fromJson
	server
		.dispatch(JsonRpcRequest {
			id: 20,
			method: "conversation/fromJson".to_string(),
			params: serde_json::json!({ "sessionId": sid, "json": "{}" }),
		})
		.await;
}

// ---------------------------------------------------------------------------
// Dispatch with invalid params
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_conv_add_user_missing_content() {
	let mut server = init_server().await;

	// Missing required "content" field — should return INVALID_PARAMS
	server
		.dispatch(JsonRpcRequest {
			id: 30,
			method: "conversation/addUser".to_string(),
			params: serde_json::json!({ "sessionId": "x" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_add_user_null_params() {
	let mut server = init_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 31,
			method: "conversation/addUser".to_string(),
			params: serde_json::json!(null),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_compact_missing_summary() {
	let mut server = init_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 32,
			method: "conversation/compact".to_string(),
			params: serde_json::json!({ "sessionId": "x" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_from_json_missing_json() {
	let mut server = init_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 33,
			method: "conversation/fromJson".to_string(),
			params: serde_json::json!({ "sessionId": "x" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_conv_tool_result_without_tool_name() {
	let mut server = init_server().await;

	// toolName is optional — should not error on missing toolName
	server
		.dispatch(JsonRpcRequest {
			id: 34,
			method: "conversation/addToolResult".to_string(),
			params: serde_json::json!({
				"sessionId": "x",
				"toolCallId": "tc_1",
				"content": "result"
			}),
		})
		.await;
}
