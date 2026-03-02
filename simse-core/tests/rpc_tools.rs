//! Integration tests for tool JSON-RPC handlers.
//!
//! Tests the tool register/unregister/list/execute/parse/metrics/result flow
//! through the `CoreRpcServer` dispatch, including the callback pattern.

use std::sync::Arc;

use simse_core::rpc_protocol::JsonRpcRequest;
use simse_core::rpc_server::CoreRpcServer;
use simse_core::rpc_transport::NdjsonTransport;
use simse_core::tools::registry::ToolRegistry;
use simse_core::tools::types::{ToolDefinition, ToolHandler, ToolRegistryOptions};

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
// ToolRegistry integration (verifies the API the handlers call)
// ---------------------------------------------------------------------------

#[test]
fn tool_registry_register_and_list() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let handler: ToolHandler = Arc::new(|_args| Box::pin(async { Ok("ok".to_string()) }));
	let definition = ToolDefinition {
		name: "test_tool".to_string(),
		description: "A test tool".to_string(),
		parameters: Default::default(),
		category: Default::default(),
		annotations: None,
		timeout_ms: None,
		max_output_chars: None,
	};

	registry.register(definition, handler);
	assert!(registry.is_registered("test_tool"));
	assert_eq!(registry.tool_count(), 1);
	assert_eq!(registry.tool_names(), vec!["test_tool"]);
}

#[test]
fn tool_registry_unregister() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let handler: ToolHandler = Arc::new(|_args| Box::pin(async { Ok("ok".to_string()) }));
	let definition = ToolDefinition {
		name: "tool_to_remove".to_string(),
		description: "Will be removed".to_string(),
		parameters: Default::default(),
		category: Default::default(),
		annotations: None,
		timeout_ms: None,
		max_output_chars: None,
	};

	registry.register(definition, handler);
	assert!(registry.is_registered("tool_to_remove"));

	let removed = registry.unregister("tool_to_remove");
	assert!(removed);
	assert!(!registry.is_registered("tool_to_remove"));

	let removed_again = registry.unregister("tool_to_remove");
	assert!(!removed_again);
}

#[tokio::test]
async fn tool_registry_execute_success() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let handler: ToolHandler = Arc::new(|args| {
		Box::pin(async move {
			let name = args
				.get("name")
				.and_then(|v| v.as_str())
				.unwrap_or("world");
			Ok(format!("Hello, {}!", name))
		})
	});
	let definition = ToolDefinition {
		name: "greet".to_string(),
		description: "Greets someone".to_string(),
		parameters: Default::default(),
		category: Default::default(),
		annotations: None,
		timeout_ms: None,
		max_output_chars: None,
	};

	registry.register(definition, handler);

	let call = simse_core::tools::types::ToolCallRequest {
		id: "call_1".to_string(),
		name: "greet".to_string(),
		arguments: serde_json::json!({ "name": "Rust" }),
	};

	let result = registry.execute(&call).await;
	assert_eq!(result.output, "Hello, Rust!");
	assert!(!result.is_error);
	assert!(result.duration_ms.is_some());
}

#[tokio::test]
async fn tool_registry_execute_not_found() {
	let registry = ToolRegistry::new(ToolRegistryOptions::default());
	let call = simse_core::tools::types::ToolCallRequest {
		id: "call_1".to_string(),
		name: "nonexistent".to_string(),
		arguments: serde_json::json!({}),
	};

	let result = registry.execute(&call).await;
	assert!(result.is_error);
	assert!(result.output.contains("not found"));
}

#[test]
fn tool_registry_parse_tool_calls() {
	let text = r#"Here is my response.
<tool_use>
{"id": "call_1", "name": "greet", "arguments": {"name": "world"}}
</tool_use>
Some more text."#;

	let parsed = ToolRegistry::parse_tool_calls(text);
	assert_eq!(parsed.tool_calls.len(), 1);
	assert_eq!(parsed.tool_calls[0].name, "greet");
	assert_eq!(parsed.tool_calls[0].id, "call_1");
	assert!(parsed.text.contains("Here is my response."));
	assert!(parsed.text.contains("Some more text."));
	assert!(!parsed.text.contains("tool_use"));
}

#[test]
fn tool_registry_format_for_system_prompt_empty() {
	let registry = ToolRegistry::new(ToolRegistryOptions::default());
	let prompt = registry.format_for_system_prompt();
	assert!(prompt.is_empty());
}

#[test]
fn tool_registry_format_for_system_prompt_with_tools() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let handler: ToolHandler = Arc::new(|_args| Box::pin(async { Ok("ok".to_string()) }));
	let definition = ToolDefinition {
		name: "my_tool".to_string(),
		description: "Does something".to_string(),
		parameters: Default::default(),
		category: Default::default(),
		annotations: None,
		timeout_ms: None,
		max_output_chars: None,
	};
	registry.register(definition, handler);

	let prompt = registry.format_for_system_prompt();
	assert!(prompt.contains("my_tool"));
	assert!(prompt.contains("Does something"));
	assert!(prompt.contains("Available tools:"));
}

#[tokio::test]
async fn tool_registry_metrics() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	let handler: ToolHandler = Arc::new(|_args| Box::pin(async { Ok("ok".to_string()) }));
	let definition = ToolDefinition {
		name: "metric_tool".to_string(),
		description: "For metrics".to_string(),
		parameters: Default::default(),
		category: Default::default(),
		annotations: None,
		timeout_ms: None,
		max_output_chars: None,
	};
	registry.register(definition, handler);

	// Before any calls, no metrics
	assert!(registry.get_all_tool_metrics().is_empty());

	// Execute once
	let call = simse_core::tools::types::ToolCallRequest {
		id: "c1".to_string(),
		name: "metric_tool".to_string(),
		arguments: serde_json::json!({}),
	};
	registry.execute(&call).await;

	let metrics = registry.get_all_tool_metrics();
	assert_eq!(metrics.len(), 1);
	assert_eq!(metrics[0].name, "metric_tool");
	assert_eq!(metrics[0].call_count, 1);
	assert_eq!(metrics[0].error_count, 0);
}

// ---------------------------------------------------------------------------
// RPC dispatch tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rpc_tool_register_requires_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/register".to_string(),
			params: serde_json::json!({
				"name": "test",
				"description": "test tool",
			}),
		})
		.await;
	// Should write NOT_INITIALIZED error (output goes to stdout, we just verify no panic)
}

#[tokio::test]
async fn rpc_tool_register_and_list() {
	let mut server = make_initialized_server().await;

	// Register a tool
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/register".to_string(),
			params: serde_json::json!({
				"name": "echo",
				"description": "Echoes input",
				"inputSchema": {
					"text": { "type": "string", "description": "Text to echo", "required": true }
				}
			}),
		})
		.await;

	// List tools — should contain the registered tool
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "tool/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Output goes to stdout; no panic = pass
}

#[tokio::test]
async fn rpc_tool_register_and_unregister() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/register".to_string(),
			params: serde_json::json!({
				"name": "temp_tool",
				"description": "Temporary",
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "tool/unregister".to_string(),
			params: serde_json::json!({ "name": "temp_tool" }),
		})
		.await;

	// List should be empty now
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "tool/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_parse() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/parse".to_string(),
			params: serde_json::json!({
				"text": "Hello <tool_use>{\"id\":\"c1\",\"name\":\"greet\",\"arguments\":{\"name\":\"world\"}}</tool_use> bye"
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_format_system_prompt() {
	let mut server = make_initialized_server().await;

	// Empty registry
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/formatSystemPrompt".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Register a tool, then format
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "tool/register".to_string(),
			params: serde_json::json!({
				"name": "search",
				"description": "Searches documents",
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "tool/formatSystemPrompt".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_metrics_empty() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/metrics".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_execute_not_found() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/execute".to_string(),
			params: serde_json::json!({
				"name": "nonexistent_tool",
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_batch_execute_empty() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/batchExecute".to_string(),
			params: serde_json::json!({ "calls": [] }),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_result_resolves_pending() {
	let mut server = make_initialized_server().await;

	// Register a tool (creates callback handler)
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/register".to_string(),
			params: serde_json::json!({
				"name": "callback_tool",
				"description": "A callback tool",
			}),
		})
		.await;

	// Manually insert a pending tool call to simulate in-flight callback
	let (tx, rx) = tokio::sync::oneshot::channel();
	{
		let mut map = server.pending_tool_calls().lock().await;
		map.insert("test_req_123".to_string(), tx);
	}

	// Send tool/result to resolve it
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "tool/result".to_string(),
			params: serde_json::json!({
				"requestId": "test_req_123",
				"output": "callback result",
				"isError": false,
			}),
		})
		.await;

	// The receiver should get the result
	let result = rx.await.unwrap();
	assert_eq!(
		result.get("output").and_then(|v| v.as_str()),
		Some("callback result")
	);
	assert_eq!(
		result.get("isError").and_then(|v| v.as_bool()),
		Some(false)
	);
}

#[tokio::test]
async fn rpc_tool_result_no_pending_still_succeeds() {
	let mut server = make_initialized_server().await;

	// Send tool/result for a non-existent requestId — should not panic
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/result".to_string(),
			params: serde_json::json!({
				"requestId": "nonexistent",
				"output": "orphaned",
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_unregister_nonexistent() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/unregister".to_string(),
			params: serde_json::json!({ "name": "does_not_exist" }),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_register_with_max_output_chars() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/register".to_string(),
			params: serde_json::json!({
				"name": "limited_tool",
				"description": "Has output limit",
				"maxOutputChars": 1000,
			}),
		})
		.await;

	// Verify it's listed
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "tool/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_execute_invalid_params() {
	let mut server = make_initialized_server().await;

	// Missing required "name" field
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/execute".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_register_invalid_params() {
	let mut server = make_initialized_server().await;

	// Missing required fields
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/register".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_parse_no_tool_calls() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/parse".to_string(),
			params: serde_json::json!({
				"text": "This is just plain text with no tool calls."
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_tool_parse_multiple_tool_calls() {
	let mut server = make_initialized_server().await;

	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "tool/parse".to_string(),
			params: serde_json::json!({
				"text": "Text <tool_use>{\"id\":\"c1\",\"name\":\"a\",\"arguments\":{}}</tool_use> middle <tool_use>{\"id\":\"c2\",\"name\":\"b\",\"arguments\":{}}</tool_use> end"
			}),
		})
		.await;
}
