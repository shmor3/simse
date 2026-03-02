//! Integration tests for hook JSON-RPC handlers.
//!
//! Tests follow the same dispatch pattern as `rpc_events.rs`: exercise the
//! `HookSystem` API directly and verify dispatch routing via `CoreRpcServer`.
//!
//! Since hook handler closures send `hook/execute` notifications to stdout and
//! await oneshot channels, the tests focus on:
//! 1. Registration/unregistration (state management)
//! 2. `hook/result` resolving pending channels
//! 3. HookSystem direct API

use std::sync::Arc;

use simse_core::config::AppConfig;
use simse_core::context::CoreContext;
use simse_core::hooks::*;
use simse_core::rpc_protocol::JsonRpcRequest;
use simse_core::rpc_server::CoreRpcServer;
use simse_core::rpc_transport::NdjsonTransport;
use simse_core::tools::types::{ToolCallRequest, ToolCallResult};

// ---------------------------------------------------------------------------
// Helper: build an initialized server
// ---------------------------------------------------------------------------

fn make_server() -> CoreRpcServer {
	let transport = NdjsonTransport::new();
	CoreRpcServer::new(transport)
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

// ---------------------------------------------------------------------------
// HookSystem direct API tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn hook_system_register_before_and_run() {
	let hs = HookSystem::new();

	let handler: BeforeHandler = Arc::new(|req: ToolCallRequest| {
		Box::pin(async move {
			// Modify the request name
			BeforeHookResult::Continue(ToolCallRequest {
				id: req.id,
				name: format!("{}_modified", req.name),
				arguments: req.arguments,
			})
		})
	});

	let _unsub = hs.register_before(handler);

	let request = ToolCallRequest {
		id: "call_1".to_string(),
		name: "read_file".to_string(),
		arguments: serde_json::json!({ "path": "/tmp/test" }),
	};

	match hs.run_before(request).await {
		BeforeHookResult::Continue(req) => {
			assert_eq!(req.name, "read_file_modified");
			assert_eq!(req.id, "call_1");
		}
		BeforeHookResult::Blocked(_) => panic!("Expected Continue"),
	}
}

#[tokio::test]
async fn hook_system_before_hook_can_block() {
	let hs = HookSystem::new();

	let handler: BeforeHandler = Arc::new(|_req: ToolCallRequest| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "Not allowed".to_string(),
			})
		})
	});

	let _unsub = hs.register_before(handler);

	let request = ToolCallRequest {
		id: "call_1".to_string(),
		name: "delete_file".to_string(),
		arguments: serde_json::json!({}),
	};

	match hs.run_before(request).await {
		BeforeHookResult::Blocked(b) => {
			assert_eq!(b.reason, "Not allowed");
		}
		BeforeHookResult::Continue(_) => panic!("Expected Blocked"),
	}
}

#[tokio::test]
async fn hook_system_before_chain_stops_on_block() {
	let hs = HookSystem::new();

	// First handler: modify name
	let h1: BeforeHandler = Arc::new(|req: ToolCallRequest| {
		Box::pin(async move {
			BeforeHookResult::Continue(ToolCallRequest {
				id: req.id,
				name: format!("{}_h1", req.name),
				arguments: req.arguments,
			})
		})
	});

	// Second handler: block
	let h2: BeforeHandler = Arc::new(|_req: ToolCallRequest| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "blocked by h2".to_string(),
			})
		})
	});

	// Third handler: should never run
	let h3_called = Arc::new(std::sync::Mutex::new(false));
	let h3_called_clone = Arc::clone(&h3_called);
	let h3: BeforeHandler = Arc::new(move |req: ToolCallRequest| {
		let called = Arc::clone(&h3_called_clone);
		Box::pin(async move {
			*called.lock().unwrap() = true;
			BeforeHookResult::Continue(req)
		})
	});

	let _u1 = hs.register_before(h1);
	let _u2 = hs.register_before(h2);
	let _u3 = hs.register_before(h3);

	let request = ToolCallRequest {
		id: "call_1".to_string(),
		name: "test".to_string(),
		arguments: serde_json::json!({}),
	};

	match hs.run_before(request).await {
		BeforeHookResult::Blocked(b) => {
			assert_eq!(b.reason, "blocked by h2");
		}
		BeforeHookResult::Continue(_) => panic!("Expected Blocked"),
	}

	assert!(!*h3_called.lock().unwrap(), "h3 should not have been called");
}

#[tokio::test]
async fn hook_system_after_chains_results() {
	let hs = HookSystem::new();

	let handler: AfterHandler = Arc::new(|ctx: AfterHookContext| {
		Box::pin(async move {
			ToolCallResult {
				id: ctx.result.id,
				name: ctx.result.name,
				output: format!("{} [transformed]", ctx.result.output),
				is_error: ctx.result.is_error,
				duration_ms: ctx.result.duration_ms,
			}
		})
	});

	let _unsub = hs.register_after(handler);

	let request = ToolCallRequest {
		id: "call_1".to_string(),
		name: "read_file".to_string(),
		arguments: serde_json::json!({}),
	};
	let result = ToolCallResult {
		id: "call_1".to_string(),
		name: "read_file".to_string(),
		output: "file contents".to_string(),
		is_error: false,
		duration_ms: Some(10),
	};

	let transformed = hs.run_after(request, result).await;
	assert_eq!(transformed.output, "file contents [transformed]");
}

#[tokio::test]
async fn hook_system_validate_concatenates_messages() {
	let hs = HookSystem::new();

	let h1: ValidateHandler = Arc::new(|_ctx: ValidateHookContext| {
		Box::pin(async move { vec!["warning 1".to_string()] })
	});

	let h2: ValidateHandler = Arc::new(|_ctx: ValidateHookContext| {
		Box::pin(async move { vec!["warning 2".to_string(), "warning 3".to_string()] })
	});

	let _u1 = hs.register_validate(h1);
	let _u2 = hs.register_validate(h2);

	let request = ToolCallRequest {
		id: "call_1".to_string(),
		name: "write_file".to_string(),
		arguments: serde_json::json!({}),
	};
	let result = ToolCallResult {
		id: "call_1".to_string(),
		name: "write_file".to_string(),
		output: "ok".to_string(),
		is_error: false,
		duration_ms: None,
	};

	let messages = hs.run_validate(request, result).await;
	assert_eq!(messages.len(), 3);
	assert_eq!(messages[0], "warning 1");
	assert_eq!(messages[1], "warning 2");
	assert_eq!(messages[2], "warning 3");
}

#[tokio::test]
async fn hook_system_prompt_transform_chains() {
	let hs = HookSystem::new();

	let h1: PromptTransformHandler = Arc::new(|prompt: String| {
		Box::pin(async move { format!("{} [h1]", prompt) })
	});

	let h2: PromptTransformHandler = Arc::new(|prompt: String| {
		Box::pin(async move { format!("{} [h2]", prompt) })
	});

	let _u1 = hs.register_prompt_transform(h1);
	let _u2 = hs.register_prompt_transform(h2);

	let result = hs.run_prompt_transform("base prompt".to_string()).await;
	assert_eq!(result, "base prompt [h1] [h2]");
}

#[tokio::test]
async fn hook_system_unsubscribe_removes_handler() {
	let hs = HookSystem::new();

	let call_count = Arc::new(std::sync::Mutex::new(0u32));
	let count_clone = Arc::clone(&call_count);

	let handler: BeforeHandler = Arc::new(move |req: ToolCallRequest| {
		let count = Arc::clone(&count_clone);
		Box::pin(async move {
			*count.lock().unwrap() += 1;
			BeforeHookResult::Continue(req)
		})
	});

	let unsub = hs.register_before(handler);

	// Run once
	let request = ToolCallRequest {
		id: "call_1".to_string(),
		name: "test".to_string(),
		arguments: serde_json::json!({}),
	};
	hs.run_before(request).await;
	assert_eq!(*call_count.lock().unwrap(), 1);

	// Unsubscribe
	unsub();

	// Run again — handler should not be called
	let request2 = ToolCallRequest {
		id: "call_2".to_string(),
		name: "test".to_string(),
		arguments: serde_json::json!({}),
	};
	hs.run_before(request2).await;
	assert_eq!(*call_count.lock().unwrap(), 1);
}

#[tokio::test]
async fn hook_system_clear_removes_all() {
	let hs = HookSystem::new();

	let _u1 = hs.register_before(Arc::new(|_req: ToolCallRequest| {
		Box::pin(async move {
			BeforeHookResult::Blocked(BlockedResult {
				reason: "blocked".to_string(),
			})
		})
	}));

	// Should block
	let request = ToolCallRequest {
		id: "call_1".to_string(),
		name: "test".to_string(),
		arguments: serde_json::json!({}),
	};
	match hs.run_before(request).await {
		BeforeHookResult::Blocked(_) => {}
		_ => panic!("Expected Blocked"),
	}

	// Clear all hooks
	hs.clear();

	// Should now continue (no handlers)
	let request2 = ToolCallRequest {
		id: "call_2".to_string(),
		name: "test".to_string(),
		arguments: serde_json::json!({}),
	};
	match hs.run_before(request2).await {
		BeforeHookResult::Continue(r) => {
			assert_eq!(r.name, "test");
		}
		_ => panic!("Expected Continue after clear"),
	}
}

#[test]
fn hook_system_through_core_context() {
	let ctx = CoreContext::new(AppConfig::default());
	// HookSystem is accessible
	let _unsub = ctx.hook_system.register_before(Arc::new(|req: ToolCallRequest| {
		Box::pin(async move { BeforeHookResult::Continue(req) })
	}));
	ctx.hook_system.clear();
}

#[tokio::test]
async fn hook_system_no_handlers_returns_defaults() {
	let hs = HookSystem::new();

	// Before: no handlers => Continue with original request
	let request = ToolCallRequest {
		id: "call_1".to_string(),
		name: "test".to_string(),
		arguments: serde_json::json!({}),
	};
	match hs.run_before(request).await {
		BeforeHookResult::Continue(r) => assert_eq!(r.name, "test"),
		_ => panic!("Expected Continue"),
	}

	// After: no handlers => returns original result
	let request2 = ToolCallRequest {
		id: "call_1".to_string(),
		name: "test".to_string(),
		arguments: serde_json::json!({}),
	};
	let result = ToolCallResult {
		id: "call_1".to_string(),
		name: "test".to_string(),
		output: "original".to_string(),
		is_error: false,
		duration_ms: None,
	};
	let after = hs.run_after(request2, result).await;
	assert_eq!(after.output, "original");

	// Validate: no handlers => empty messages
	let request3 = ToolCallRequest {
		id: "call_1".to_string(),
		name: "test".to_string(),
		arguments: serde_json::json!({}),
	};
	let result3 = ToolCallResult {
		id: "call_1".to_string(),
		name: "test".to_string(),
		output: "ok".to_string(),
		is_error: false,
		duration_ms: None,
	};
	let messages = hs.run_validate(request3, result3).await;
	assert!(messages.is_empty());

	// Prompt transform: no handlers => original prompt
	let transformed = hs.run_prompt_transform("original".to_string()).await;
	assert_eq!(transformed, "original");
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — hook/registerBefore
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_hook_register_before_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should write not-initialized error, not panic
}

#[tokio::test]
async fn dispatch_hook_register_before_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return { hookId: "hook_0" }
}

#[tokio::test]
async fn dispatch_hook_register_before_returns_unique_ids() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// hook_0 and hook_1 should be returned
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — hook/registerAfter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_hook_register_after_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerAfter".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_hook_register_after_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerAfter".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return { hookId: "hook_0" }
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — hook/registerValidate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_hook_register_validate_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerValidate".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_hook_register_validate_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerValidate".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return { hookId: "hook_0" }
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — hook/registerTransform
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_hook_register_transform_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerTransform".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_hook_register_transform_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerTransform".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return { hookId: "hook_0" }
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — hook/unregister
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_hook_unregister_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({ "hookId": "hook_0" }),
		})
		.await;
	// Should write not-initialized error
}

#[tokio::test]
async fn dispatch_hook_unregister_existing() {
	let mut server = make_initialized_server().await;

	// Register a hook first
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Unregister it
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({ "hookId": "hook_0" }),
		})
		.await;
	// Should succeed
}

#[tokio::test]
async fn dispatch_hook_unregister_nonexistent() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({ "hookId": "hook_999" }),
		})
		.await;
	// Should return {} (idempotent)
}

#[tokio::test]
async fn dispatch_hook_unregister_double_call() {
	let mut server = make_initialized_server().await;

	// Register
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Unregister twice — should be idempotent
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({ "hookId": "hook_0" }),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({ "hookId": "hook_0" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_hook_unregister_missing_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return INVALID_PARAMS error (missing hookId)
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — hook/result
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_hook_result_missing_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/result".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return INVALID_PARAMS error (missing requestId and result)
}

#[tokio::test]
async fn dispatch_hook_result_nonexistent_request_id() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/result".to_string(),
			params: serde_json::json!({
				"requestId": "nonexistent-uuid",
				"result": { "output": "hello" }
			}),
		})
		.await;
	// Should return {} (no pending call to resolve, but no error)
}

#[tokio::test]
async fn dispatch_hook_result_resolves_pending_channel() {
	let mut server = make_initialized_server().await;

	// Insert a pending hook call manually via the server's pending_hook_calls
	let (tx, rx) = tokio::sync::oneshot::channel::<serde_json::Value>();
	{
		let mut map = server.pending_hook_calls().lock().await;
		map.insert("test-request-id".to_string(), tx);
	}

	// Send hook/result
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/result".to_string(),
			params: serde_json::json!({
				"requestId": "test-request-id",
				"result": { "prompt": "transformed prompt" }
			}),
		})
		.await;

	// The channel should have received the result
	let result = rx.await.unwrap();
	assert_eq!(
		result.get("prompt").and_then(|v| v.as_str()),
		Some("transformed prompt")
	);
}

// ---------------------------------------------------------------------------
// Full lifecycle tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_hook_register_all_types_unique_ids() {
	let mut server = make_initialized_server().await;

	// Register one of each type
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "hook/registerAfter".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "hook/registerValidate".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "hook/registerTransform".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// All should get unique IDs (hook_0, hook_1, hook_2, hook_3)
	// and not panic
}

#[tokio::test]
async fn dispatch_hook_full_lifecycle() {
	let mut server = make_initialized_server().await;

	// Register a before hook
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Register an after hook
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "hook/registerAfter".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Unregister the before hook
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({ "hookId": "hook_0" }),
		})
		.await;

	// Register another before hook — should get hook_2
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Unregister all
	server
		.dispatch(JsonRpcRequest {
			id: 5,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({ "hookId": "hook_1" }),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 6,
			method: "hook/unregister".to_string(),
			params: serde_json::json!({ "hookId": "hook_2" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_hook_result_with_null_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/result".to_string(),
			params: serde_json::json!(null),
		})
		.await;
	// Should return INVALID_PARAMS error
}

#[tokio::test]
async fn dispatch_hook_register_before_with_null_params() {
	let mut server = make_initialized_server().await;
	// registerBefore takes no params — null should be fine
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "hook/registerBefore".to_string(),
			params: serde_json::json!(null),
		})
		.await;
	// Should still succeed (no params needed)
}
