use std::sync::Arc;

use simse_core::conversation::{ConversationMessage, Role};
use simse_core::hooks::*;
use simse_core::tools::types::{ToolCallRequest, ToolCallResult};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_request(name: &str, args: &str) -> ToolCallRequest {
	ToolCallRequest {
		id: "call-1".into(),
		name: name.into(),
		arguments: serde_json::json!({ "input": args }),
	}
}

fn make_result(output: &str) -> ToolCallResult {
	ToolCallResult {
		id: "call-1".into(),
		name: "test_tool".into(),
		output: output.into(),
		is_error: false,
		duration_ms: Some(10),
		diff: None,
	}
}

fn make_message(role: Role, content: &str) -> ConversationMessage {
	ConversationMessage {
		role,
		content: content.into(),
		tool_call_id: None,
		tool_name: None,
		timestamp: None,
	}
}

// ===========================================================================
// Before hooks
// ===========================================================================

#[tokio::test]
async fn before_no_hooks_returns_original() {
	let hooks = HookSystem::new();
	let req = make_request("read_file", "/tmp/a.txt");
	let result = hooks.run_before(req.clone()).await;
	match result {
		BeforeHookResult::Continue(r) => {
			assert_eq!(r.name, "read_file");
			assert_eq!(r.arguments, req.arguments);
		}
		BeforeHookResult::Blocked(_) => panic!("expected Continue"),
	}
}

#[tokio::test]
async fn before_passthrough() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_before(Arc::new(|req| {
		Box::pin(async move { BeforeHookResult::Continue(req) })
	}));

	let req = make_request("read_file", "/tmp/a.txt");
	let result = hooks.run_before(req.clone()).await;
	match result {
		BeforeHookResult::Continue(r) => assert_eq!(r.name, "read_file"),
		BeforeHookResult::Blocked(_) => panic!("expected Continue"),
	}
}

#[tokio::test]
async fn before_modifies_request() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_before(Arc::new(|mut req| {
		Box::pin(async move {
			req.name = "modified_tool".into();
			BeforeHookResult::Continue(req)
		})
	}));

	let req = make_request("original_tool", "args");
	match hooks.run_before(req).await {
		BeforeHookResult::Continue(r) => assert_eq!(r.name, "modified_tool"),
		BeforeHookResult::Blocked(_) => panic!("expected Continue"),
	}
}

#[tokio::test]
async fn before_blocks_execution() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_before(Arc::new(|_req| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "denied".into(),
			})
		})
	}));

	let req = make_request("dangerous_tool", "args");
	match hooks.run_before(req).await {
		BeforeHookResult::Continue(_) => panic!("expected Blocked"),
		BeforeHookResult::Blocked(b) => assert_eq!(b.reason, "denied"),
	}
}

#[tokio::test]
async fn before_block_stops_chain_early() {
	let hooks = HookSystem::new();

	// First hook blocks
	let _unsub1 = hooks.register_before(Arc::new(|_req| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "first blocks".into(),
			})
		})
	}));

	// Second hook would modify — but should never run
	let _unsub2 = hooks.register_before(Arc::new(|mut req| {
		Box::pin(async move {
			req.name = "should_not_reach".into();
			BeforeHookResult::Continue(req)
		})
	}));

	let req = make_request("tool", "args");
	match hooks.run_before(req).await {
		BeforeHookResult::Blocked(b) => assert_eq!(b.reason, "first blocks"),
		BeforeHookResult::Continue(r) => {
			panic!("expected Blocked, got Continue with name={}", r.name)
		}
	}
}

#[tokio::test]
async fn before_chains_modifications() {
	let hooks = HookSystem::new();

	// First hook appends "_1"
	let _unsub1 = hooks.register_before(Arc::new(|mut req| {
		Box::pin(async move {
			req.name = format!("{}_1", req.name);
			BeforeHookResult::Continue(req)
		})
	}));

	// Second hook appends "_2"
	let _unsub2 = hooks.register_before(Arc::new(|mut req| {
		Box::pin(async move {
			req.name = format!("{}_2", req.name);
			BeforeHookResult::Continue(req)
		})
	}));

	let req = make_request("tool", "args");
	match hooks.run_before(req).await {
		BeforeHookResult::Continue(r) => assert_eq!(r.name, "tool_1_2"),
		BeforeHookResult::Blocked(_) => panic!("expected Continue"),
	}
}

// ===========================================================================
// After hooks
// ===========================================================================

#[tokio::test]
async fn after_no_hooks_returns_original() {
	let hooks = HookSystem::new();
	let req = make_request("tool", "args");
	let res = make_result("original output");
	let result = hooks.run_after(req, res).await;
	assert_eq!(result.output, "original output");
}

#[tokio::test]
async fn after_passthrough() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_after(Arc::new(|ctx| {
		Box::pin(async move { ctx.result })
	}));

	let req = make_request("tool", "args");
	let res = make_result("output");
	let result = hooks.run_after(req, res).await;
	assert_eq!(result.output, "output");
}

#[tokio::test]
async fn after_modifies_result() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_after(Arc::new(|ctx| {
		Box::pin(async move {
			let mut result = ctx.result;
			result.output = format!("modified: {}", result.output);
			result
		})
	}));

	let req = make_request("tool", "args");
	let res = make_result("original");
	let result = hooks.run_after(req, res).await;
	assert_eq!(result.output, "modified: original");
}

#[tokio::test]
async fn after_chains_results() {
	let hooks = HookSystem::new();

	let _unsub1 = hooks.register_after(Arc::new(|ctx| {
		Box::pin(async move {
			let mut result = ctx.result;
			result.output = format!("{}+first", result.output);
			result
		})
	}));

	let _unsub2 = hooks.register_after(Arc::new(|ctx| {
		Box::pin(async move {
			let mut result = ctx.result;
			result.output = format!("{}+second", result.output);
			result
		})
	}));

	let req = make_request("tool", "args");
	let res = make_result("base");
	let result = hooks.run_after(req, res).await;
	assert_eq!(result.output, "base+first+second");
}

#[tokio::test]
async fn after_receives_original_request() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_after(Arc::new(|ctx| {
		Box::pin(async move {
			let mut result = ctx.result;
			result.output = format!("tool={}", ctx.request.name);
			result
		})
	}));

	let req = make_request("special_tool", "args");
	let res = make_result("ignored");
	let result = hooks.run_after(req, res).await;
	assert_eq!(result.output, "tool=special_tool");
}

// ===========================================================================
// Validate hooks
// ===========================================================================

#[tokio::test]
async fn validate_no_hooks_returns_empty() {
	let hooks = HookSystem::new();
	let req = make_request("tool", "args");
	let res = make_result("output");
	let messages = hooks.run_validate(req, res).await;
	assert!(messages.is_empty());
}

#[tokio::test]
async fn validate_single_handler() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_validate(Arc::new(|_ctx| {
		Box::pin(async move { vec!["warning: output too short".into()] })
	}));

	let req = make_request("tool", "args");
	let res = make_result("x");
	let messages = hooks.run_validate(req, res).await;
	assert_eq!(messages, vec!["warning: output too short"]);
}

#[tokio::test]
async fn validate_concatenates_messages() {
	let hooks = HookSystem::new();

	let _unsub1 = hooks.register_validate(Arc::new(|_ctx| {
		Box::pin(async move { vec!["msg1".into(), "msg2".into()] })
	}));

	let _unsub2 = hooks.register_validate(Arc::new(|_ctx| {
		Box::pin(async move { vec!["msg3".into()] })
	}));

	let req = make_request("tool", "args");
	let res = make_result("output");
	let messages = hooks.run_validate(req, res).await;
	assert_eq!(messages, vec!["msg1", "msg2", "msg3"]);
}

#[tokio::test]
async fn validate_empty_array_from_handler_is_fine() {
	let hooks = HookSystem::new();

	let _unsub1 = hooks.register_validate(Arc::new(|_ctx| {
		Box::pin(async move { vec![] })
	}));

	let _unsub2 = hooks.register_validate(Arc::new(|_ctx| {
		Box::pin(async move { vec!["only this".into()] })
	}));

	let req = make_request("tool", "args");
	let res = make_result("output");
	let messages = hooks.run_validate(req, res).await;
	assert_eq!(messages, vec!["only this"]);
}

// ===========================================================================
// Prompt transform hooks
// ===========================================================================

#[tokio::test]
async fn prompt_transform_no_hooks_returns_original() {
	let hooks = HookSystem::new();
	let result = hooks
		.run_prompt_transform("You are a helpful assistant.".into())
		.await;
	assert_eq!(result, "You are a helpful assistant.");
}

#[tokio::test]
async fn prompt_transform_single() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_prompt_transform(Arc::new(|prompt| {
		Box::pin(async move { format!("{prompt}\nBe concise.") })
	}));

	let result = hooks.run_prompt_transform("You are helpful.".into()).await;
	assert_eq!(result, "You are helpful.\nBe concise.");
}

#[tokio::test]
async fn prompt_transform_chains() {
	let hooks = HookSystem::new();

	let _unsub1 = hooks.register_prompt_transform(Arc::new(|prompt| {
		Box::pin(async move { format!("{prompt}+A") })
	}));

	let _unsub2 = hooks.register_prompt_transform(Arc::new(|prompt| {
		Box::pin(async move { format!("{prompt}+B") })
	}));

	let result = hooks.run_prompt_transform("base".into()).await;
	assert_eq!(result, "base+A+B");
}

// ===========================================================================
// Messages transform hooks
// ===========================================================================

#[tokio::test]
async fn messages_transform_no_hooks_returns_original() {
	let hooks = HookSystem::new();
	let msgs = vec![make_message(Role::User, "hello")];
	let result = hooks.run_messages_transform(msgs).await;
	assert_eq!(result.len(), 1);
	assert_eq!(result[0].content, "hello");
}

#[tokio::test]
async fn messages_transform_adds_message() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_messages_transform(Arc::new(|mut msgs| {
		Box::pin(async move {
			msgs.push(ConversationMessage {
				role: Role::System,
				content: "injected".into(),
				tool_call_id: None,
				tool_name: None,
				timestamp: None,
			});
			msgs
		})
	}));

	let msgs = vec![make_message(Role::User, "hello")];
	let result = hooks.run_messages_transform(msgs).await;
	assert_eq!(result.len(), 2);
	assert_eq!(result[1].content, "injected");
}

#[tokio::test]
async fn messages_transform_chains() {
	let hooks = HookSystem::new();

	// First hook uppercases content
	let _unsub1 = hooks.register_messages_transform(Arc::new(|msgs| {
		Box::pin(async move {
			msgs.into_iter()
				.map(|mut m| {
					m.content = m.content.to_uppercase();
					m
				})
				.collect()
		})
	}));

	// Second hook adds suffix
	let _unsub2 = hooks.register_messages_transform(Arc::new(|msgs| {
		Box::pin(async move {
			msgs.into_iter()
				.map(|mut m| {
					m.content = format!("{}!", m.content);
					m
				})
				.collect()
		})
	}));

	let msgs = vec![make_message(Role::User, "hello")];
	let result = hooks.run_messages_transform(msgs).await;
	assert_eq!(result[0].content, "HELLO!");
}

// ===========================================================================
// Compacting hooks
// ===========================================================================

#[tokio::test]
async fn compacting_no_hooks_returns_original() {
	let hooks = HookSystem::new();
	let msgs = vec![make_message(Role::User, "hello")];
	let result = hooks.run_compacting(msgs, "summary".into()).await;
	assert_eq!(result, "summary");
}

#[tokio::test]
async fn compacting_modifies_summary() {
	let hooks = HookSystem::new();
	let _unsub = hooks.register_compacting(Arc::new(|ctx| {
		Box::pin(async move {
			format!("{} ({}msg)", ctx.summary, ctx.messages.len())
		})
	}));

	let msgs = vec![
		make_message(Role::User, "a"),
		make_message(Role::Assistant, "b"),
	];
	let result = hooks.run_compacting(msgs, "sum".into()).await;
	assert_eq!(result, "sum (2msg)");
}

#[tokio::test]
async fn compacting_chains_with_original_messages() {
	let hooks = HookSystem::new();

	let _unsub1 = hooks.register_compacting(Arc::new(|ctx| {
		Box::pin(async move {
			format!("{}+first({})", ctx.summary, ctx.messages.len())
		})
	}));

	let _unsub2 = hooks.register_compacting(Arc::new(|ctx| {
		Box::pin(async move {
			// Should still see original message count
			format!("{}+second({})", ctx.summary, ctx.messages.len())
		})
	}));

	let msgs = vec![make_message(Role::User, "hello")];
	let result = hooks.run_compacting(msgs, "base".into()).await;
	assert_eq!(result, "base+first(1)+second(1)");
}

// ===========================================================================
// Register / unregister lifecycle
// ===========================================================================

#[tokio::test]
async fn unsubscribe_before() {
	let hooks = HookSystem::new();
	let unsub = hooks.register_before(Arc::new(|_req| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "blocked".into(),
			})
		})
	}));

	// Should block
	let req = make_request("tool", "args");
	assert!(matches!(
		hooks.run_before(req).await,
		BeforeHookResult::Blocked(_)
	));

	// Unsubscribe
	unsub();

	// Should pass through now
	let req = make_request("tool", "args");
	assert!(matches!(
		hooks.run_before(req).await,
		BeforeHookResult::Continue(_)
	));
}

#[tokio::test]
async fn unsubscribe_after() {
	let hooks = HookSystem::new();
	let unsub = hooks.register_after(Arc::new(|ctx| {
		Box::pin(async move {
			let mut result = ctx.result;
			result.output = "modified".into();
			result
		})
	}));

	let req = make_request("tool", "args");
	let res = make_result("original");
	assert_eq!(hooks.run_after(req.clone(), res).await.output, "modified");

	unsub();

	let res = make_result("original");
	assert_eq!(hooks.run_after(req, res).await.output, "original");
}

#[tokio::test]
async fn unsubscribe_validate() {
	let hooks = HookSystem::new();
	let unsub = hooks.register_validate(Arc::new(|_ctx| {
		Box::pin(async move { vec!["error".into()] })
	}));

	let req = make_request("tool", "args");
	let res = make_result("output");
	assert_eq!(hooks.run_validate(req.clone(), res).await.len(), 1);

	unsub();

	let res = make_result("output");
	assert!(hooks.run_validate(req, res).await.is_empty());
}

#[tokio::test]
async fn unsubscribe_prompt_transform() {
	let hooks = HookSystem::new();
	let unsub = hooks.register_prompt_transform(Arc::new(|_prompt| {
		Box::pin(async move { "replaced".into() })
	}));

	assert_eq!(
		hooks.run_prompt_transform("original".into()).await,
		"replaced"
	);

	unsub();

	assert_eq!(
		hooks.run_prompt_transform("original".into()).await,
		"original"
	);
}

#[tokio::test]
async fn unsubscribe_messages_transform() {
	let hooks = HookSystem::new();
	let unsub = hooks.register_messages_transform(Arc::new(|_msgs| {
		Box::pin(async move { vec![] }) // clear all messages
	}));

	let msgs = vec![make_message(Role::User, "hello")];
	assert!(hooks.run_messages_transform(msgs).await.is_empty());

	unsub();

	let msgs = vec![make_message(Role::User, "hello")];
	assert_eq!(hooks.run_messages_transform(msgs).await.len(), 1);
}

#[tokio::test]
async fn unsubscribe_compacting() {
	let hooks = HookSystem::new();
	let unsub = hooks.register_compacting(Arc::new(|_ctx| {
		Box::pin(async move { "replaced".into() })
	}));

	let msgs = vec![make_message(Role::User, "hello")];
	assert_eq!(
		hooks.run_compacting(msgs, "original".into()).await,
		"replaced"
	);

	unsub();

	let msgs = vec![make_message(Role::User, "hello")];
	assert_eq!(
		hooks.run_compacting(msgs, "original".into()).await,
		"original"
	);
}

#[tokio::test]
async fn unsubscribe_is_idempotent() {
	let hooks = HookSystem::new();
	let unsub = hooks.register_before(Arc::new(|_req| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "blocked".into(),
			})
		})
	}));

	unsub();
	unsub(); // second call is a no-op
	unsub(); // third call is a no-op

	let req = make_request("tool", "args");
	assert!(matches!(
		hooks.run_before(req).await,
		BeforeHookResult::Continue(_)
	));
}

// ===========================================================================
// Multiple hooks execute in registration order
// ===========================================================================

#[tokio::test]
async fn hooks_execute_in_registration_order() {
	let hooks = HookSystem::new();
	let order = Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));

	let o1 = order.clone();
	let _unsub1 = hooks.register_prompt_transform(Arc::new(move |prompt| {
		let o = o1.clone();
		Box::pin(async move {
			o.lock().unwrap().push(1);
			prompt
		})
	}));

	let o2 = order.clone();
	let _unsub2 = hooks.register_prompt_transform(Arc::new(move |prompt| {
		let o = o2.clone();
		Box::pin(async move {
			o.lock().unwrap().push(2);
			prompt
		})
	}));

	let o3 = order.clone();
	let _unsub3 = hooks.register_prompt_transform(Arc::new(move |prompt| {
		let o = o3.clone();
		Box::pin(async move {
			o.lock().unwrap().push(3);
			prompt
		})
	}));

	hooks.run_prompt_transform("test".into()).await;
	assert_eq!(*order.lock().unwrap(), vec![1, 2, 3]);
}

// ===========================================================================
// Clear removes all hooks
// ===========================================================================

#[tokio::test]
async fn clear_removes_all_hooks() {
	let hooks = HookSystem::new();

	let _unsub1 = hooks.register_before(Arc::new(|_req| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "blocked".into(),
			})
		})
	}));

	let _unsub2 = hooks.register_after(Arc::new(|ctx| {
		Box::pin(async move {
			let mut r = ctx.result;
			r.output = "modified".into();
			r
		})
	}));

	let _unsub3 = hooks.register_validate(Arc::new(|_ctx| {
		Box::pin(async move { vec!["error".into()] })
	}));

	let _unsub4 = hooks.register_prompt_transform(Arc::new(|_p| {
		Box::pin(async move { "replaced".into() })
	}));

	let _unsub5 = hooks.register_messages_transform(Arc::new(|_m| {
		Box::pin(async move { vec![] })
	}));

	let _unsub6 = hooks.register_compacting(Arc::new(|_ctx| {
		Box::pin(async move { "replaced".into() })
	}));

	hooks.clear();

	// All should now be no-ops
	let req = make_request("tool", "args");
	assert!(matches!(
		hooks.run_before(req.clone()).await,
		BeforeHookResult::Continue(_)
	));

	let res = make_result("original");
	assert_eq!(hooks.run_after(req.clone(), res).await.output, "original");

	let res = make_result("output");
	assert!(hooks.run_validate(req, res).await.is_empty());

	assert_eq!(
		hooks.run_prompt_transform("original".into()).await,
		"original"
	);

	let msgs = vec![make_message(Role::User, "hello")];
	assert_eq!(hooks.run_messages_transform(msgs).await.len(), 1);

	let msgs = vec![make_message(Role::User, "hello")];
	assert_eq!(
		hooks.run_compacting(msgs, "original".into()).await,
		"original"
	);
}

// ===========================================================================
// Clone shares state
// ===========================================================================

#[tokio::test]
async fn clone_shares_state() {
	let hooks = HookSystem::new();
	let hooks2 = hooks.clone();

	let _unsub = hooks.register_before(Arc::new(|_req| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "from original".into(),
			})
		})
	}));

	// Cloned hook system should see the handler registered on the original
	let req = make_request("tool", "args");
	match hooks2.run_before(req).await {
		BeforeHookResult::Blocked(b) => assert_eq!(b.reason, "from original"),
		BeforeHookResult::Continue(_) => panic!("expected Blocked"),
	}
}
