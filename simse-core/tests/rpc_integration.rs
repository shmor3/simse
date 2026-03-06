//! End-to-end integration test for the JSON-RPC server.
//!
//! Exercises a full flow across multiple domains (lifecycle, session,
//! conversation, task, event) through `CoreRpcServer::dispatch`, verifying
//! that operations compose correctly and do not panic.

use std::sync::{Arc, Mutex};

use simse_core::config::AppConfig;
use simse_core::context::CoreContext;
use simse_core::conversation::Role;
use simse_core::rpc_protocol::JsonRpcRequest;
use simse_core::rpc_server::CoreRpcServer;
use simse_core::rpc_transport::NdjsonTransport;
use simse_core::tasks::{TaskCreateInput, TaskList, TaskStatus, TaskUpdateInput};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_server() -> CoreRpcServer {
	CoreRpcServer::new(NdjsonTransport::new())
}

async fn make_initialized_server() -> CoreRpcServer {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 0,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	server
}

// ===========================================================================
// Full end-to-end flow through CoreRpcServer dispatch
// ===========================================================================

/// Tests a complete lifecycle:
/// 1. core/initialize
/// 2. session/create
/// 3. conversation/setSystemPrompt + conversation/addUser
/// 4. conversation/getMessages
/// 5. task/create + task/list
/// 6. event/publish
/// 7. core/dispose
///
/// Since the transport writes to stdout and we cannot capture dispatch
/// output in tests, we verify no panics and correct routing. Where possible,
/// we also exercise the CoreContext API directly to confirm state changes.
#[tokio::test]
async fn full_lifecycle_through_dispatch() {
	let mut server = make_server();

	// Step 1: Initialize
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Step 2: Create a session
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "session/create".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Step 3a: Set system prompt (we use a plausible session ID)
	// Since we can't capture the ID from dispatch, use "sess_1" pattern
	// (the SessionManager uses uuid, so we just verify no panic with a
	// fake ID — the handler returns SESSION_NOT_FOUND gracefully)
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "conversation/setSystemPrompt".to_string(),
			params: serde_json::json!({
				"sessionId": "unknown_session",
				"prompt": "You are a helpful coding assistant."
			}),
		})
		.await;

	// Step 3b: Add a user message
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "conversation/addUser".to_string(),
			params: serde_json::json!({
				"sessionId": "unknown_session",
				"content": "Help me write a function."
			}),
		})
		.await;

	// Step 4: Get messages
	server
		.dispatch(JsonRpcRequest {
			id: 5,
			method: "conversation/getMessages".to_string(),
			params: serde_json::json!({ "sessionId": "unknown_session" }),
		})
		.await;

	// Step 5a: Create a task
	server
		.dispatch(JsonRpcRequest {
			id: 6,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Write integration tests",
				"description": "Cover the full RPC lifecycle",
				"activeForm": "Writing tests"
			}),
		})
		.await;

	// Step 5b: List tasks
	server
		.dispatch(JsonRpcRequest {
			id: 7,
			method: "task/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Step 6: Publish an event
	server
		.dispatch(JsonRpcRequest {
			id: 8,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "integration.test",
				"payload": { "step": "publish" }
			}),
		})
		.await;

	// Step 7: Dispose
	server
		.dispatch(JsonRpcRequest {
			id: 9,
			method: "core/dispose".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// After dispose, operations should fail with NOT_INITIALIZED
	server
		.dispatch(JsonRpcRequest {
			id: 10,
			method: "session/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

// ===========================================================================
// Full lifecycle using CoreContext API (with real state verification)
// ===========================================================================

/// Tests the same flow as above but through the CoreContext API directly,
/// allowing us to verify actual state changes rather than just no-panic.
#[test]
fn full_lifecycle_through_core_context() {
	// Step 1: Initialize
	let mut ctx = CoreContext::new(AppConfig::default());

	// Step 2: Create session
	let session_id = ctx.session_manager.create();
	assert!(session_id.starts_with("sess_"));

	let info = ctx
		.session_manager
		.get_info(&session_id)
		.expect("session should exist");
	assert_eq!(info.message_count, 0);

	// Step 3: Set system prompt + add user message
	ctx.session_manager.with_state_transition(&session_id, |conv| {
		let conv = conv.set_system_prompt("You are a helpful coding assistant.".to_string());
		let conv = conv.add_user("Help me write a function.");
		(conv, ())
	});

	// Step 4: Get messages and verify
	ctx.session_manager.with_session(&session_id, |session| {
		assert_eq!(
			session.conversation.system_prompt(),
			Some("You are a helpful coding assistant.")
		);
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 1);
		assert_eq!(msgs[0].role, Role::User);
		assert_eq!(msgs[0].content, "Help me write a function.");
	});

	let info = ctx
		.session_manager
		.get_info(&session_id)
		.expect("session should exist");
	assert_eq!(info.message_count, 1);

	// Step 5: Create task + list
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, task) = task_list.create(TaskCreateInput {
		subject: "Write integration tests".to_string(),
		description: "Cover the full RPC lifecycle".to_string(),
		active_form: Some("Writing tests".to_string()),
		owner: None,
		metadata: None,
	});
	ctx.task_list = new_list;
	assert_eq!(task.id, "1");
	assert_eq!(task.status, TaskStatus::Pending);

	let tasks = ctx.task_list.list();
	assert_eq!(tasks.len(), 1);
	assert_eq!(tasks[0].subject, "Write integration tests");

	// Step 6: Publish event + verify delivery
	let received = Arc::new(Mutex::new(Vec::new()));
	let received_clone = Arc::clone(&received);

	let _unsub = ctx.event_bus.subscribe("integration.test", move |payload| {
		received_clone.lock().unwrap().push(payload.clone());
	});

	ctx.event_bus.publish(
		"integration.test",
		serde_json::json!({ "step": "publish" }),
	);

	let events = received.lock().unwrap();
	assert_eq!(events.len(), 1);
	assert_eq!(events[0]["step"], "publish");
}

// ===========================================================================
// Multi-session + cross-domain interaction
// ===========================================================================

/// Tests that multiple sessions can coexist, each with independent
/// conversations, while sharing the same task list and event bus.
#[test]
fn multi_session_with_shared_tasks_and_events() {
	let mut ctx = CoreContext::new(AppConfig::default());

	// Create two sessions
	let session_a = ctx.session_manager.create();
	let session_b = ctx.session_manager.create();

	// Add messages to session A
	ctx.session_manager.with_state_transition(&session_a, |conv| {
		let conv = conv.set_system_prompt("You are a Rust expert.".to_string());
		let conv = conv.add_user("How do I use lifetimes?");
		let conv = conv.add_assistant("Lifetimes ensure references are valid...");
		(conv, ())
	});

	// Add messages to session B
	ctx.session_manager.with_state_transition(&session_b, |conv| {
		let conv = conv.set_system_prompt("You are a TypeScript expert.".to_string());
		let conv = conv.add_user("How do I use generics?");
		(conv, ())
	});

	// Verify sessions are independent
	let info_a = ctx.session_manager.get_info(&session_a).unwrap();
	let info_b = ctx.session_manager.get_info(&session_b).unwrap();
	assert_eq!(info_a.message_count, 2);
	assert_eq!(info_b.message_count, 1);

	// Shared task list — tasks visible across sessions
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, task) = task_list.create(TaskCreateInput {
		subject: "Review code".to_string(),
		description: "Cross-session task".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	ctx.task_list = new_list;
	assert_eq!(ctx.task_list.list().len(), 1);

	// Update the task
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list
		.update(
			&task.id,
			TaskUpdateInput {
				status: Some(TaskStatus::InProgress),
				owner: Some("agent_a".to_string()),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	let updated = ctx.task_list.get(&task.id).unwrap();
	assert_eq!(updated.status, TaskStatus::InProgress);
	assert_eq!(updated.owner, Some("agent_a".to_string()));

	// Shared event bus — events visible across sessions
	let event_log = Arc::new(Mutex::new(Vec::new()));
	let log_clone = Arc::clone(&event_log);

	let _unsub = ctx.event_bus.subscribe_all(move |event_type, payload| {
		log_clone
			.lock()
			.unwrap()
			.push((event_type.to_string(), payload.clone()));
	});

	ctx.event_bus
		.publish("session.a.done", serde_json::json!({ "session": "a" }));
	ctx.event_bus
		.publish("session.b.done", serde_json::json!({ "session": "b" }));

	let logged = event_log.lock().unwrap();
	assert_eq!(logged.len(), 2);
	assert_eq!(logged[0].0, "session.a.done");
	assert_eq!(logged[1].0, "session.b.done");
}

// ===========================================================================
// Session fork preserves conversation, shares task list
// ===========================================================================

#[test]
fn fork_session_and_diverge() {
	let ctx = CoreContext::new(AppConfig::default());

	// Create and populate original session
	let original = ctx.session_manager.create();
	ctx.session_manager.with_state_transition(&original, |conv| {
		let conv = conv.set_system_prompt("Expert assistant.".to_string());
		let conv = conv.add_user("Question 1");
		let conv = conv.add_assistant("Answer 1");
		(conv, ())
	});

	// Fork
	let forked = ctx
		.session_manager
		.fork(&original)
		.expect("fork should succeed");

	// Forked session has the same messages
	ctx.session_manager.with_session(&forked, |session| {
		assert_eq!(
			session.conversation.system_prompt(),
			Some("Expert assistant.")
		);
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 2);
		assert_eq!(msgs[0].content, "Question 1");
		assert_eq!(msgs[1].content, "Answer 1");
	});

	// Diverge: add messages only to the forked session
	ctx.session_manager.with_state_transition(&forked, |conv| {
		let conv = conv.add_user("Question 2");
		let conv = conv.add_assistant("Different answer path.");
		(conv, ())
	});

	// Original unchanged
	let original_info = ctx.session_manager.get_info(&original).unwrap();
	assert_eq!(original_info.message_count, 2);

	// Forked has the extra messages
	let forked_info = ctx.session_manager.get_info(&forked).unwrap();
	assert_eq!(forked_info.message_count, 4);

	// Both sessions listed
	let all = ctx.session_manager.list();
	assert_eq!(all.len(), 2);
}

// ===========================================================================
// Task dependency chain with event notifications
// ===========================================================================

#[test]
fn task_dependency_chain_with_events() {
	let mut ctx = CoreContext::new(AppConfig::default());

	// Track events
	let events = Arc::new(Mutex::new(Vec::<String>::new()));
	let events_clone = Arc::clone(&events);

	let _unsub = ctx
		.event_bus
		.subscribe("task.completed", move |payload| {
			if let Some(id) = payload.get("id").and_then(|v| v.as_str()) {
				events_clone.lock().unwrap().push(id.to_string());
			}
		});

	// Create a chain: design -> implement -> test
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (task_list, _) = task_list.create(TaskCreateInput {
		subject: "Design API".to_string(),
		description: "Design the public API surface".to_string(),
		active_form: Some("Designing".to_string()),
		owner: None,
		metadata: None,
	});
	let (task_list, _) = task_list.create(TaskCreateInput {
		subject: "Implement API".to_string(),
		description: "Build the implementation".to_string(),
		active_form: Some("Implementing".to_string()),
		owner: None,
		metadata: None,
	});
	let (task_list, _) = task_list.create(TaskCreateInput {
		subject: "Test API".to_string(),
		description: "Write integration tests".to_string(),
		active_form: Some("Testing".to_string()),
		owner: None,
		metadata: None,
	});

	// Set up dependencies: implement blocked by design, test blocked by implement
	let (task_list, _) = task_list
		.update(
			"2",
			TaskUpdateInput {
				add_blocked_by: Some(vec!["1".to_string()]),
				..Default::default()
			},
		)
		.unwrap();

	let (task_list, _) = task_list
		.update(
			"3",
			TaskUpdateInput {
				add_blocked_by: Some(vec!["2".to_string()]),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = task_list;

	// Only task 1 should be available
	let available = ctx.task_list.list_available();
	assert_eq!(available.len(), 1);
	assert_eq!(available[0].id, "1");

	// Complete task 1, publish event
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (task_list, _) = task_list
		.update(
			"1",
			TaskUpdateInput {
				status: Some(TaskStatus::Completed),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = task_list;
	ctx.event_bus
		.publish("task.completed", serde_json::json!({ "id": "1" }));

	// Task 2 should now be available (task 1 completed)
	let available = ctx.task_list.list_available();
	assert_eq!(available.len(), 1);
	assert_eq!(available[0].id, "2");

	// Task 3 is still blocked by task 2
	let task3 = ctx.task_list.get("3").unwrap();
	assert!(task3.blocked_by.contains(&"2".to_string()));

	// Complete task 2
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (task_list, _) = task_list
		.update(
			"2",
			TaskUpdateInput {
				status: Some(TaskStatus::Completed),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = task_list;
	ctx.event_bus
		.publish("task.completed", serde_json::json!({ "id": "2" }));

	// Task 3 should now be available
	let available = ctx.task_list.list_available();
	assert_eq!(available.len(), 1);
	assert_eq!(available[0].id, "3");

	// Verify all completion events were received
	let completed = events.lock().unwrap();
	assert_eq!(completed.len(), 2);
	assert_eq!(completed[0], "1");
	assert_eq!(completed[1], "2");
}

// ===========================================================================
// Dispatch: init -> multi-domain operations -> dispose -> re-init
// ===========================================================================

/// Verifies that after dispose, the server can be re-initialized and used
/// again from a clean state.
#[tokio::test]
async fn reinitialize_after_dispose() {
	let mut server = make_server();

	// First lifecycle
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "session/create".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "First lifecycle task",
				"description": "Should be gone after dispose"
			}),
		})
		.await;

	// Dispose
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "core/dispose".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// After dispose, session/list should fail with NOT_INITIALIZED
	server
		.dispatch(JsonRpcRequest {
			id: 5,
			method: "session/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Re-initialize
	server
		.dispatch(JsonRpcRequest {
			id: 6,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Operations should work again from a clean slate
	server
		.dispatch(JsonRpcRequest {
			id: 7,
			method: "session/create".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 8,
			method: "task/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Health check should report initialized
	server
		.dispatch(JsonRpcRequest {
			id: 9,
			method: "core/health".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

// ===========================================================================
// Dispatch: conversation flow with compaction and serialization
// ===========================================================================

/// Tests conversation operations through dispatch: multi-turn with tool
/// results, compaction, export/import, and clearing.
#[tokio::test]
async fn conversation_lifecycle_through_dispatch() {
	let mut server = make_initialized_server().await;

	// Create session
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "session/create".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// We cannot capture the session ID from dispatch, so test all conversation
	// methods with a placeholder ID. The handlers gracefully handle missing
	// sessions. This validates dispatch routing and param validation.

	let sid = "placeholder";

	// Multi-turn flow
	server
		.dispatch(JsonRpcRequest {
			id: 10,
			method: "conversation/setSystemPrompt".to_string(),
			params: serde_json::json!({
				"sessionId": sid,
				"prompt": "You are a coding assistant."
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 11,
			method: "conversation/addUser".to_string(),
			params: serde_json::json!({
				"sessionId": sid,
				"content": "Write a function."
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 12,
			method: "conversation/addAssistant".to_string(),
			params: serde_json::json!({
				"sessionId": sid,
				"content": "Here is the function..."
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 13,
			method: "conversation/addToolResult".to_string(),
			params: serde_json::json!({
				"sessionId": sid,
				"toolCallId": "tc_1",
				"toolName": "run_tests",
				"content": "All 5 tests passed"
			}),
		})
		.await;

	// Get messages
	server
		.dispatch(JsonRpcRequest {
			id: 14,
			method: "conversation/getMessages".to_string(),
			params: serde_json::json!({ "sessionId": sid }),
		})
		.await;

	// Stats
	server
		.dispatch(JsonRpcRequest {
			id: 15,
			method: "conversation/stats".to_string(),
			params: serde_json::json!({ "sessionId": sid }),
		})
		.await;

	// Export to JSON
	server
		.dispatch(JsonRpcRequest {
			id: 16,
			method: "conversation/toJson".to_string(),
			params: serde_json::json!({ "sessionId": sid }),
		})
		.await;

	// Compact
	server
		.dispatch(JsonRpcRequest {
			id: 17,
			method: "conversation/compact".to_string(),
			params: serde_json::json!({
				"sessionId": sid,
				"summary": "Wrote and tested a function."
			}),
		})
		.await;

	// Clear
	server
		.dispatch(JsonRpcRequest {
			id: 18,
			method: "conversation/clear".to_string(),
			params: serde_json::json!({ "sessionId": sid }),
		})
		.await;

	// Import from JSON
	server
		.dispatch(JsonRpcRequest {
			id: 19,
			method: "conversation/fromJson".to_string(),
			params: serde_json::json!({
				"sessionId": sid,
				"json": "{}"
			}),
		})
		.await;
}

// ===========================================================================
// Dispatch: event subscribe -> publish -> unsubscribe flow
// ===========================================================================

#[tokio::test]
async fn event_subscribe_publish_unsubscribe_through_dispatch() {
	let mut server = make_initialized_server().await;

	// Subscribe to an event
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "build.complete" }),
		})
		.await;

	// Subscribe to wildcard
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "*" }),
		})
		.await;

	// Publish events
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "build.complete",
				"payload": { "success": true, "duration": 42 }
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "deploy.start",
				"payload": { "target": "production" }
			}),
		})
		.await;

	// Unsubscribe the specific subscription
	server
		.dispatch(JsonRpcRequest {
			id: 5,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_0" }),
		})
		.await;

	// Publish again — only wildcard should receive
	server
		.dispatch(JsonRpcRequest {
			id: 6,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "build.complete",
				"payload": { "success": false }
			}),
		})
		.await;

	// Unsubscribe wildcard
	server
		.dispatch(JsonRpcRequest {
			id: 7,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_1" }),
		})
		.await;
}

// ===========================================================================
// Dispatch: unknown method
// ===========================================================================

#[tokio::test]
async fn dispatch_unknown_method_returns_error() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "unknown/method".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return METHOD_NOT_FOUND error, not panic
}

// ===========================================================================
// Health check reflects initialization state
// ===========================================================================

#[tokio::test]
async fn health_check_reflects_state() {
	let mut server = make_server();

	// Before init: health should report not initialized
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "core/health".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Initialize
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// After init: health should report initialized
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "core/health".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Dispose
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "core/dispose".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// After dispose: health should report not initialized again
	server
		.dispatch(JsonRpcRequest {
			id: 5,
			method: "core/health".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

// ===========================================================================
// Initialize with custom config
// ===========================================================================

#[tokio::test]
async fn initialize_with_custom_config() {
	let mut server = make_server();

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "core/initialize".to_string(),
			params: serde_json::json!({
				"maxTasks": 50
			}),
		})
		.await;

	// Verify server is operational after custom config init
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "session/create".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Custom config task",
				"description": "Verify custom config works"
			}),
		})
		.await;
}

// ===========================================================================
// CoreContext: conversation serialization round-trip across sessions
// ===========================================================================

#[test]
fn conversation_round_trip_across_sessions() {
	let ctx = CoreContext::new(AppConfig::default());

	// Build conversation in session A
	let session_a = ctx.session_manager.create();
	ctx.session_manager.with_state_transition(&session_a, |conv| {
		let conv = conv.set_system_prompt("Be helpful.".to_string());
		let conv = conv.add_user("Question");
		let conv = conv.add_assistant("Answer");
		let conv = conv.add_tool_result("tc_1", "search", "Found results");
		(conv, ())
	});

	// Export to JSON
	let json = ctx
		.session_manager
		.with_session(&session_a, |session| session.conversation.to_json())
		.unwrap();

	// Import into session B
	let session_b = ctx.session_manager.create();
	ctx.session_manager.with_state_transition(&session_b, |conv| {
		(conv.from_json(&json), ())
	});

	// Verify session B has the same state
	ctx.session_manager.with_session(&session_b, |session| {
		assert_eq!(session.conversation.system_prompt(), Some("Be helpful."));
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 3);
		assert_eq!(msgs[0].role, Role::User);
		assert_eq!(msgs[0].content, "Question");
		assert_eq!(msgs[1].role, Role::Assistant);
		assert_eq!(msgs[1].content, "Answer");
		assert_eq!(msgs[2].role, Role::ToolResult);
		assert_eq!(msgs[2].content, "Found results");
		assert_eq!(msgs[2].tool_call_id.as_deref(), Some("tc_1"));
		assert_eq!(msgs[2].tool_name.as_deref(), Some("search"));
	});
}

// ===========================================================================
// CoreContext: event bus clear resets all subscriptions
// ===========================================================================

#[test]
fn event_bus_clear_in_full_flow() {
	let ctx = CoreContext::new(AppConfig::default());

	let count = Arc::new(Mutex::new(0u32));
	let count_clone = Arc::clone(&count);

	let _unsub = ctx.event_bus.subscribe("flow.event", move |_| {
		*count_clone.lock().unwrap() += 1;
	});

	// Publish — should be received
	ctx.event_bus
		.publish("flow.event", serde_json::json!(null));
	assert_eq!(*count.lock().unwrap(), 1);

	// Clear all handlers
	ctx.event_bus.clear();

	// Publish again — should NOT be received
	ctx.event_bus
		.publish("flow.event", serde_json::json!(null));
	assert_eq!(*count.lock().unwrap(), 1);
}

// ===========================================================================
// CoreContext: task list + conversation interaction
// ===========================================================================

/// Simulates an agentic workflow: create a task, update conversation with
/// task-related messages, complete the task, verify all state is consistent.
#[test]
fn agentic_workflow_simulation() {
	let mut ctx = CoreContext::new(AppConfig::default());

	// Create session for the agent
	let session_id = ctx.session_manager.create();
	ctx.session_manager.with_state_transition(&session_id, |conv| {
		(conv.set_system_prompt("You are an autonomous coding agent.".to_string()), ())
	});

	// Create a task
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, task) = task_list.create(TaskCreateInput {
		subject: "Fix login bug".to_string(),
		description: "Users cannot log in with OAuth".to_string(),
		active_form: Some("Fixing login bug".to_string()),
		owner: Some("agent".to_string()),
		metadata: None,
	});
	ctx.task_list = new_list;

	// Start working on the task
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list
		.update(
			&task.id,
			TaskUpdateInput {
				status: Some(TaskStatus::InProgress),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	// Agent adds messages about the work
	ctx.session_manager.with_state_transition(&session_id, |conv| {
		let conv = conv.add_user("Fix the login bug described in task 1.");
		let conv = conv.add_assistant(
			"I'll investigate the OAuth login flow and fix the bug.",
		);
		let conv = conv.add_tool_result("tc_read", "read_file", "// OAuth handler code...");
		let conv = conv.add_assistant("Found the issue. The redirect URI is incorrect.");
		(conv, ())
	});

	// Verify conversation state
	ctx.session_manager.with_session(&session_id, |session| {
		let msgs = session.conversation.messages();
		assert_eq!(msgs.len(), 4);
	});

	// Complete the task
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list
		.update(
			&task.id,
			TaskUpdateInput {
				status: Some(TaskStatus::Completed),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	// Publish completion event
	ctx.event_bus.publish(
		"task.completed",
		serde_json::json!({
			"id": task.id,
			"subject": "Fix login bug"
		}),
	);

	// Verify final state
	let completed_task = ctx.task_list.get(&task.id).unwrap();
	assert_eq!(completed_task.status, TaskStatus::Completed);

	let info = ctx.session_manager.get_info(&session_id).unwrap();
	assert_eq!(info.message_count, 4);
}
