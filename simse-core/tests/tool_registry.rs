//! Tests for the tool registry module.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use simse_core::error::SimseError;
use simse_core::agentic_loop::ToolExecutor;
use simse_core::tools::{
	ToolCallRequest, ToolCallResult, ToolCategory, ToolDefinition, ToolHandler, ToolParameter,
	ToolPermissionResolver, ToolRegistry, ToolRegistryOptions,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Creates a simple tool definition with the given name and an optional parameter.
fn make_definition(name: &str) -> ToolDefinition {
	let mut params = HashMap::new();
	params.insert(
		"input".to_string(),
		ToolParameter {
			param_type: "string".to_string(),
			description: "The input value".to_string(),
			required: true,
		},
	);
	ToolDefinition {
		name: name.to_string(),
		description: format!("Test tool: {}", name),
		parameters: params,
		category: ToolCategory::Other,
		annotations: None,
		timeout_ms: None,
		max_output_chars: None,
	}
}

/// Creates a handler that echoes the "input" argument or returns "no input".
fn make_echo_handler() -> ToolHandler {
	Arc::new(|args: serde_json::Value| {
		Box::pin(async move {
			let input = args
				.get("input")
				.and_then(|v| v.as_str())
				.unwrap_or("no input");
			Ok(format!("echo: {}", input))
		})
	})
}

/// Creates a handler that always fails with the given message.
fn make_failing_handler(msg: &str) -> ToolHandler {
	let message = msg.to_string();
	Arc::new(move |_args: serde_json::Value| {
		let m = message.clone();
		Box::pin(async move { Err(SimseError::other(m)) })
	})
}

/// Creates a handler that returns a string of `n` 'x' characters.
fn make_large_output_handler(n: usize) -> ToolHandler {
	Arc::new(move |_args: serde_json::Value| {
		Box::pin(async move { Ok("x".repeat(n)) })
	})
}

/// Creates a handler that sleeps for the given duration.
fn make_slow_handler(ms: u64) -> ToolHandler {
	Arc::new(move |_args: serde_json::Value| {
		Box::pin(async move {
			tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
			Ok("slow result".to_string())
		})
	})
}

/// Creates a default registry with no options.
fn make_registry() -> ToolRegistry {
	ToolRegistry::new(ToolRegistryOptions::default())
}

/// Creates a tool call request.
fn make_call(id: &str, name: &str) -> ToolCallRequest {
	ToolCallRequest {
		id: id.to_string(),
		name: name.to_string(),
		arguments: serde_json::json!({"input": "hello"}),
	}
}

// ---------------------------------------------------------------------------
// Mock permission resolver
// ---------------------------------------------------------------------------

struct AllowAllResolver;

#[async_trait]
impl ToolPermissionResolver for AllowAllResolver {
	async fn check(
		&self,
		_request: &ToolCallRequest,
		_definition: Option<&ToolDefinition>,
	) -> bool {
		true
	}
}

struct DenyAllResolver;

#[async_trait]
impl ToolPermissionResolver for DenyAllResolver {
	async fn check(
		&self,
		_request: &ToolCallRequest,
		_definition: Option<&ToolDefinition>,
	) -> bool {
		false
	}
}

struct DenySpecificResolver {
	denied_tool: String,
}

#[async_trait]
impl ToolPermissionResolver for DenySpecificResolver {
	async fn check(
		&self,
		request: &ToolCallRequest,
		_definition: Option<&ToolDefinition>,
	) -> bool {
		request.name != self.denied_tool
	}
}

// ===========================================================================
// Tests
// ===========================================================================

// ---------------------------------------------------------------------------
// 1. Register and retrieve a tool
// ---------------------------------------------------------------------------

#[test]
fn test_register_and_retrieve_tool() {
	let mut registry = make_registry();
	let def = make_definition("echo");
	registry.register_mut(def, make_echo_handler());

	assert!(registry.is_registered("echo"));
	assert_eq!(registry.tool_count(), 1);

	let retrieved = registry.get_tool_definition("echo").unwrap();
	assert_eq!(retrieved.name, "echo");
	assert_eq!(retrieved.description, "Test tool: echo");
	assert_eq!(retrieved.category, ToolCategory::Other);
	assert!(retrieved.parameters.contains_key("input"));
}

#[test]
fn test_get_tool_definitions_returns_all() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("alpha"), make_echo_handler());
	registry.register_mut(make_definition("beta"), make_echo_handler());

	let defs = registry.get_tool_definitions();
	assert_eq!(defs.len(), 2);
}

#[test]
fn test_get_nonexistent_tool_returns_none() {
	let registry = make_registry();
	assert!(registry.get_tool_definition("nope").is_none());
}

// ---------------------------------------------------------------------------
// 2. Unregister a tool
// ---------------------------------------------------------------------------

#[test]
fn test_unregister_tool() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("to_remove"), make_echo_handler());
	assert!(registry.is_registered("to_remove"));

	let removed = registry.unregister_mut("to_remove");
	assert!(removed);
	assert!(!registry.is_registered("to_remove"));
	assert_eq!(registry.tool_count(), 0);
}

#[test]
fn test_unregister_nonexistent_returns_false() {
	let mut registry = make_registry();
	assert!(!registry.unregister_mut("ghost"));
}

// ---------------------------------------------------------------------------
// 3. Execute a tool successfully
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_tool_success() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("echo"), make_echo_handler());

	let call = make_call("c1", "echo");
	let result = registry.execute(&call).await;

	assert_eq!(result.id, "c1");
	assert_eq!(result.name, "echo");
	assert_eq!(result.output, "echo: hello");
	assert!(!result.is_error);
	assert!(result.duration_ms.is_some());
}

// ---------------------------------------------------------------------------
// 4. Execute with output truncation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_with_output_truncation() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions {
		max_output_chars: Some(50),
		..Default::default()
	});
	// Handler produces 200 chars
	registry.register_mut(make_definition("big"), make_large_output_handler(200));

	let call = ToolCallRequest {
		id: "c1".to_string(),
		name: "big".to_string(),
		arguments: serde_json::json!({}),
	};
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("[OUTPUT TRUNCATED"));
	assert!(result.output.contains("200 chars total"));
	assert!(result.output.contains("showing first 50"));
	// The visible content should start with the first 50 'x' chars
	assert!(result.output.starts_with("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".get(..50).unwrap()));
}

#[tokio::test]
async fn test_execute_with_per_tool_output_limit() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions {
		max_output_chars: Some(1000), // registry default is 1000
		..Default::default()
	});
	let mut def = make_definition("limited");
	def.max_output_chars = Some(20); // per-tool override
	registry.register_mut(def, make_large_output_handler(100));

	let call = ToolCallRequest {
		id: "c1".to_string(),
		name: "limited".to_string(),
		arguments: serde_json::json!({}),
	};
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("[OUTPUT TRUNCATED"));
	assert!(result.output.contains("100 chars total"));
	assert!(result.output.contains("showing first 20"));
}

#[tokio::test]
async fn test_no_truncation_when_within_limit() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions {
		max_output_chars: Some(500),
		..Default::default()
	});
	registry.register_mut(make_definition("small"), make_large_output_handler(100));

	let call = ToolCallRequest {
		id: "c1".to_string(),
		name: "small".to_string(),
		arguments: serde_json::json!({}),
	};
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(!result.output.contains("[OUTPUT TRUNCATED"));
	assert_eq!(result.output.len(), 100);
}

// ---------------------------------------------------------------------------
// 5. Execute non-existent tool returns error result
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_nonexistent_tool() {
	let registry = make_registry();
	let call = make_call("c1", "nonexistent");
	let result = registry.execute(&call).await;

	assert!(result.is_error);
	assert_eq!(result.id, "c1");
	assert_eq!(result.name, "nonexistent");
	assert!(result.output.contains("Tool not found"));
	assert!(result.output.contains("nonexistent"));
}

// ---------------------------------------------------------------------------
// 6. Execute with permission denied
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_permission_denied() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions {
		permission_resolver: Some(Arc::new(DenyAllResolver)),
		..Default::default()
	});
	registry.register_mut(make_definition("secret"), make_echo_handler());

	let call = make_call("c1", "secret");
	let result = registry.execute(&call).await;

	assert!(result.is_error);
	assert!(result.output.contains("Permission denied"));
	assert!(result.output.contains("secret"));
}

#[tokio::test]
async fn test_execute_permission_allowed() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions {
		permission_resolver: Some(Arc::new(AllowAllResolver)),
		..Default::default()
	});
	registry.register_mut(make_definition("allowed"), make_echo_handler());

	let call = make_call("c1", "allowed");
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "echo: hello");
}

#[tokio::test]
async fn test_execute_selective_permission() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions {
		permission_resolver: Some(Arc::new(DenySpecificResolver {
			denied_tool: "restricted".to_string(),
		})),
		..Default::default()
	});
	registry.register_mut(make_definition("restricted"), make_echo_handler());
	registry.register_mut(make_definition("open"), make_echo_handler());

	let denied_result = registry.execute(&make_call("c1", "restricted")).await;
	assert!(denied_result.is_error);
	assert!(denied_result.output.contains("Permission denied"));

	let allowed_result = registry.execute(&make_call("c2", "open")).await;
	assert!(!allowed_result.is_error);
	assert_eq!(allowed_result.output, "echo: hello");
}

// ---------------------------------------------------------------------------
// 7. Execute with timeout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_with_timeout() {
	let mut registry = make_registry();
	let mut def = make_definition("slow");
	def.timeout_ms = Some(50); // 50ms timeout
	registry.register_mut(def, make_slow_handler(5000)); // handler takes 5s

	let call = make_call("c1", "slow");
	let result = registry.execute(&call).await;

	assert!(result.is_error);
	assert!(result.output.contains("timed out"));
}

#[tokio::test]
async fn test_execute_with_default_timeout() {
	let mut registry = ToolRegistry::new(ToolRegistryOptions {
		default_timeout_ms: Some(50),
		..Default::default()
	});
	registry.register_mut(make_definition("slow"), make_slow_handler(5000));

	let call = make_call("c1", "slow");
	let result = registry.execute(&call).await;

	assert!(result.is_error);
	assert!(result.output.contains("timed out"));
}

#[tokio::test]
async fn test_execute_completes_within_timeout() {
	let mut registry = make_registry();
	let mut def = make_definition("quick");
	def.timeout_ms = Some(5000); // generous timeout
	registry.register_mut(def, make_slow_handler(10)); // 10ms handler

	let call = make_call("c1", "quick");
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "slow result");
}

// ---------------------------------------------------------------------------
// 8. Batch execute with concurrency
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_batch_execute() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("echo"), make_echo_handler());

	let calls = vec![
		make_call("c1", "echo"),
		make_call("c2", "echo"),
		make_call("c3", "echo"),
	];

	let results = registry.batch_execute(&calls, None).await;

	assert_eq!(results.len(), 3);
	for (i, result) in results.iter().enumerate() {
		assert_eq!(result.id, format!("c{}", i + 1));
		assert!(!result.is_error);
		assert_eq!(result.output, "echo: hello");
	}
}

#[tokio::test]
async fn test_batch_execute_empty() {
	let registry = make_registry();
	let results = registry.batch_execute(&[], None).await;
	assert!(results.is_empty());
}

#[tokio::test]
async fn test_batch_execute_with_concurrency_limit() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("echo"), make_echo_handler());

	let calls: Vec<ToolCallRequest> = (0..10)
		.map(|i| make_call(&format!("c{}", i), "echo"))
		.collect();

	// Limit concurrency to 2
	let results = registry.batch_execute(&calls, Some(2)).await;

	assert_eq!(results.len(), 10);
	for result in &results {
		assert!(!result.is_error);
	}
}

#[tokio::test]
async fn test_batch_execute_mixed_success_and_failure() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("good"), make_echo_handler());
	registry.register_mut(make_definition("bad"), make_failing_handler("boom"));

	let calls = vec![
		make_call("c1", "good"),
		make_call("c2", "bad"),
		make_call("c3", "good"),
		make_call("c4", "missing"),
	];

	let results = registry.batch_execute(&calls, None).await;

	assert_eq!(results.len(), 4);
	assert!(!results[0].is_error);
	assert!(results[1].is_error);
	assert!(!results[2].is_error);
	assert!(results[3].is_error); // not found
}

// ---------------------------------------------------------------------------
// 9. Parse tool calls from response text
// ---------------------------------------------------------------------------

#[test]
fn test_parse_tool_calls() {
	let response = r#"Here is my response.

<tool_use>
{"id": "call_1", "name": "search", "arguments": {"query": "rust"}}
</tool_use>

More text here.

<tool_use>
{"id": "call_2", "name": "read_file", "arguments": {"path": "/tmp/x.txt"}}
</tool_use>

Final text."#;

	let parsed = ToolRegistry::parse_tool_calls(response);

	assert_eq!(parsed.tool_calls.len(), 2);

	assert_eq!(parsed.tool_calls[0].id, "call_1");
	assert_eq!(parsed.tool_calls[0].name, "search");
	assert_eq!(
		parsed.tool_calls[0].arguments,
		serde_json::json!({"query": "rust"})
	);

	assert_eq!(parsed.tool_calls[1].id, "call_2");
	assert_eq!(parsed.tool_calls[1].name, "read_file");
	assert_eq!(
		parsed.tool_calls[1].arguments,
		serde_json::json!({"path": "/tmp/x.txt"})
	);

	// Text should have tool blocks removed
	assert!(!parsed.text.contains("<tool_use>"));
	assert!(parsed.text.contains("Here is my response."));
	assert!(parsed.text.contains("More text here."));
	assert!(parsed.text.contains("Final text."));
}

#[test]
fn test_parse_tool_calls_auto_id() {
	let response = r#"<tool_use>
{"name": "search", "arguments": {"q": "test"}}
</tool_use>"#;

	let parsed = ToolRegistry::parse_tool_calls(response);

	assert_eq!(parsed.tool_calls.len(), 1);
	assert_eq!(parsed.tool_calls[0].id, "call_1"); // auto-generated
	assert_eq!(parsed.tool_calls[0].name, "search");
}

#[test]
fn test_parse_tool_calls_auto_empty_arguments() {
	let response = r#"<tool_use>
{"id": "c1", "name": "list_files"}
</tool_use>"#;

	let parsed = ToolRegistry::parse_tool_calls(response);

	assert_eq!(parsed.tool_calls.len(), 1);
	assert_eq!(parsed.tool_calls[0].arguments, serde_json::json!({}));
}

// ---------------------------------------------------------------------------
// 10. Parse with malformed JSON (skip gracefully)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_malformed_json_skipped() {
	let response = r#"Text before.

<tool_use>
{this is not valid json}
</tool_use>

<tool_use>
{"id": "c1", "name": "valid_tool", "arguments": {"key": "value"}}
</tool_use>

<tool_use>
{"malformed": true}
</tool_use>"#;

	let parsed = ToolRegistry::parse_tool_calls(response);

	// Only the valid one with a "name" should be captured
	assert_eq!(parsed.tool_calls.len(), 1);
	assert_eq!(parsed.tool_calls[0].name, "valid_tool");
	assert!(parsed.text.contains("Text before."));
}

#[test]
fn test_parse_missing_name_skipped() {
	let response = r#"<tool_use>
{"id": "c1", "arguments": {"key": "value"}}
</tool_use>"#;

	let parsed = ToolRegistry::parse_tool_calls(response);
	assert_eq!(parsed.tool_calls.len(), 0);
}

// ---------------------------------------------------------------------------
// 11. Parse with no tool calls
// ---------------------------------------------------------------------------

#[test]
fn test_parse_no_tool_calls() {
	let response = "Just a regular response with no tool calls.";
	let parsed = ToolRegistry::parse_tool_calls(response);

	assert!(parsed.tool_calls.is_empty());
	assert_eq!(parsed.text, response);
}

#[test]
fn test_parse_empty_string() {
	let parsed = ToolRegistry::parse_tool_calls("");
	assert!(parsed.tool_calls.is_empty());
	assert_eq!(parsed.text, "");
}

// ---------------------------------------------------------------------------
// 12. Format tools for system prompt
// ---------------------------------------------------------------------------

#[test]
fn test_format_for_system_prompt() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("search"), make_echo_handler());
	registry.register_mut(make_definition("read_file"), make_echo_handler());

	let prompt = registry.format_for_system_prompt();

	assert!(prompt.contains("You have access to tools"));
	assert!(prompt.contains("<tool_use>"));
	assert!(prompt.contains("</tool_use>"));
	assert!(prompt.contains("Available tools:"));
	assert!(prompt.contains("search"));
	assert!(prompt.contains("read_file"));
	assert!(prompt.contains("Parameters:"));
	assert!(prompt.contains("input (string, required)"));
}

#[test]
fn test_format_empty_registry() {
	let registry = make_registry();
	let prompt = registry.format_for_system_prompt();
	assert_eq!(prompt, "");
}

#[test]
fn test_format_tool_without_parameters() {
	let mut registry = make_registry();
	let def = ToolDefinition {
		name: "noop".to_string(),
		description: "Does nothing".to_string(),
		parameters: HashMap::new(),
		category: ToolCategory::Other,
		annotations: None,
		timeout_ms: None,
		max_output_chars: None,
	};
	registry.register_mut(def, make_echo_handler());

	let prompt = registry.format_for_system_prompt();
	assert!(prompt.contains("- noop: Does nothing"));
	// Should NOT contain "Parameters:" for this tool since it has none
	// (but other tools might, so just check noop line)
}

// ---------------------------------------------------------------------------
// 13. Metrics tracking
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_metrics_tracking() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("echo"), make_echo_handler());

	// Execute twice
	registry.execute(&make_call("c1", "echo")).await;
	registry.execute(&make_call("c2", "echo")).await;

	let metrics = registry.get_tool_metrics("echo").unwrap();
	assert_eq!(metrics.name, "echo");
	assert_eq!(metrics.call_count, 2);
	assert_eq!(metrics.error_count, 0);
	// total_duration_ms is u64, always >= 0
	let _ = metrics.total_duration_ms;
	assert!(metrics.avg_duration_ms >= 0.0);
	assert!(metrics.last_called_at > 0);
}

#[tokio::test]
async fn test_metrics_error_tracking() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("fail"), make_failing_handler("oops"));

	registry.execute(&make_call("c1", "fail")).await;
	registry.execute(&make_call("c2", "fail")).await;

	let metrics = registry.get_tool_metrics("fail").unwrap();
	assert_eq!(metrics.call_count, 2);
	assert_eq!(metrics.error_count, 2);
}

#[tokio::test]
async fn test_metrics_mixed_success_error() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("echo"), make_echo_handler());
	registry.register_mut(make_definition("fail"), make_failing_handler("oops"));

	registry.execute(&make_call("c1", "echo")).await;
	registry.execute(&make_call("c2", "fail")).await;
	registry.execute(&make_call("c3", "echo")).await;

	let echo_metrics = registry.get_tool_metrics("echo").unwrap();
	assert_eq!(echo_metrics.call_count, 2);
	assert_eq!(echo_metrics.error_count, 0);

	let fail_metrics = registry.get_tool_metrics("fail").unwrap();
	assert_eq!(fail_metrics.call_count, 1);
	assert_eq!(fail_metrics.error_count, 1);
}

#[tokio::test]
async fn test_get_all_tool_metrics() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("alpha"), make_echo_handler());
	registry.register_mut(make_definition("beta"), make_echo_handler());

	registry.execute(&make_call("c1", "alpha")).await;
	registry.execute(&make_call("c2", "beta")).await;
	registry.execute(&make_call("c3", "alpha")).await;

	let all_metrics = registry.get_all_tool_metrics();
	assert_eq!(all_metrics.len(), 2);

	let alpha = all_metrics.iter().find(|m| m.name == "alpha").unwrap();
	assert_eq!(alpha.call_count, 2);

	let beta = all_metrics.iter().find(|m| m.name == "beta").unwrap();
	assert_eq!(beta.call_count, 1);
}

#[test]
fn test_get_metrics_for_uncalled_tool_returns_none() {
	let registry = make_registry();
	assert!(registry.get_tool_metrics("never_called").is_none());
}

// ---------------------------------------------------------------------------
// 14. Clear metrics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_clear_metrics() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("echo"), make_echo_handler());

	registry.execute(&make_call("c1", "echo")).await;
	assert!(registry.get_tool_metrics("echo").is_some());

	registry.clear_metrics();
	assert!(registry.get_tool_metrics("echo").is_none());
	assert!(registry.get_all_tool_metrics().is_empty());
}

// ---------------------------------------------------------------------------
// 15. tool_count and tool_names accessors
// ---------------------------------------------------------------------------

#[test]
fn test_tool_count_and_names() {
	let mut registry = make_registry();
	assert_eq!(registry.tool_count(), 0);
	assert!(registry.tool_names().is_empty());

	registry.register_mut(make_definition("alpha"), make_echo_handler());
	registry.register_mut(make_definition("beta"), make_echo_handler());

	assert_eq!(registry.tool_count(), 2);

	let names = registry.tool_names();
	assert_eq!(names.len(), 2);
	assert!(names.contains(&"alpha".to_string()));
	assert!(names.contains(&"beta".to_string()));
}

#[test]
fn test_is_registered() {
	let mut registry = make_registry();
	assert!(!registry.is_registered("test"));

	registry.register_mut(make_definition("test"), make_echo_handler());
	assert!(registry.is_registered("test"));

	registry.unregister_mut("test");
	assert!(!registry.is_registered("test"));
}

// ---------------------------------------------------------------------------
// Additional edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_handler_error_is_captured() {
	let mut registry = make_registry();
	registry.register_mut(
		make_definition("kaboom"),
		make_failing_handler("something went wrong"),
	);

	let call = make_call("c1", "kaboom");
	let result = registry.execute(&call).await;

	assert!(result.is_error);
	assert!(result.output.contains("something went wrong"));
	assert!(result.duration_ms.is_some());
}

#[test]
fn test_register_overwrites_existing() {
	let mut registry = make_registry();
	let mut def1 = make_definition("tool");
	def1.description = "version 1".to_string();
	registry.register_mut(def1, make_echo_handler());

	let mut def2 = make_definition("tool");
	def2.description = "version 2".to_string();
	registry.register_mut(def2, make_echo_handler());

	assert_eq!(registry.tool_count(), 1);
	let retrieved = registry.get_tool_definition("tool").unwrap();
	assert_eq!(retrieved.description, "version 2");
}

#[test]
fn test_tool_category_display() {
	assert_eq!(ToolCategory::Read.to_string(), "read");
	assert_eq!(ToolCategory::Edit.to_string(), "edit");
	assert_eq!(ToolCategory::Search.to_string(), "search");
	assert_eq!(ToolCategory::Execute.to_string(), "execute");
	assert_eq!(ToolCategory::Library.to_string(), "library");
	assert_eq!(ToolCategory::Task.to_string(), "task");
	assert_eq!(ToolCategory::Subagent.to_string(), "subagent");
	assert_eq!(ToolCategory::Other.to_string(), "other");
}

#[test]
fn test_tool_category_default() {
	assert_eq!(ToolCategory::default(), ToolCategory::Other);
}

#[test]
fn test_tool_definition_serde() {
	let def = make_definition("test");
	let json = serde_json::to_string(&def).unwrap();
	let restored: ToolDefinition = serde_json::from_str(&json).unwrap();
	assert_eq!(restored.name, "test");
	assert_eq!(restored.category, ToolCategory::Other);
	assert!(restored.parameters.contains_key("input"));
}

#[test]
fn test_tool_call_request_serde() {
	let call = make_call("c1", "echo");
	let json = serde_json::to_string(&call).unwrap();
	let restored: ToolCallRequest = serde_json::from_str(&json).unwrap();
	assert_eq!(restored.id, "c1");
	assert_eq!(restored.name, "echo");
}

#[test]
fn test_tool_call_result_serde() {
	let result = ToolCallResult {
		id: "c1".to_string(),
		name: "echo".to_string(),
		output: "hello".to_string(),
		is_error: false,
		duration_ms: Some(42),
		diff: None,
	};
	let json = serde_json::to_string(&result).unwrap();
	let restored: ToolCallResult = serde_json::from_str(&json).unwrap();
	assert_eq!(restored.id, "c1");
	assert_eq!(restored.duration_ms, Some(42));
	assert!(!restored.is_error);
}

#[tokio::test]
async fn test_batch_execute_preserves_order() {
	let mut registry = make_registry();

	// Register tools that return their name
	for name in &["aaa", "bbb", "ccc"] {
		let n = name.to_string();
		let handler: ToolHandler = Arc::new(move |_args: serde_json::Value| {
			let name = n.clone();
			Box::pin(async move { Ok(format!("result_{}", name)) })
		});
		registry.register_mut(make_definition(name), handler);
	}

	let calls = vec![
		make_call("c1", "aaa"),
		make_call("c2", "bbb"),
		make_call("c3", "ccc"),
	];

	let results = registry.batch_execute(&calls, Some(1)).await;

	assert_eq!(results[0].output, "result_aaa");
	assert_eq!(results[1].output, "result_bbb");
	assert_eq!(results[2].output, "result_ccc");
}

#[test]
fn test_parse_tool_calls_preserves_surrounding_whitespace_stripped() {
	let response = "   \n  Some text \n  ";
	let parsed = ToolRegistry::parse_tool_calls(response);
	assert_eq!(parsed.text, "Some text");
}

#[test]
fn test_registry_debug_format() {
	let registry = make_registry();
	let debug = format!("{:?}", registry);
	assert!(debug.contains("ToolRegistry"));
	assert!(debug.contains("tool_count"));
}

// ---------------------------------------------------------------------------
// ToolExecutor trait implementation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tool_registry_implements_tool_executor() {
	let registry = make_registry();
	let executor: &dyn ToolExecutor = &registry;

	// Test parse_tool_calls via trait object
	let parsed = executor.parse_tool_calls("hello world");
	assert!(parsed.tool_calls.is_empty());
	assert_eq!(parsed.text, "hello world");

	// Test parse_tool_calls with actual tool use block
	let response = r#"Some text <tool_use>
{"id": "c1", "name": "test", "arguments": {"key": "value"}}
</tool_use>"#;
	let parsed = executor.parse_tool_calls(response);
	assert_eq!(parsed.tool_calls.len(), 1);
	assert_eq!(parsed.tool_calls[0].name, "test");

	// Test execute via trait object (non-existent tool)
	let call = make_call("c1", "nonexistent");
	let result = executor.execute(&call).await;
	assert!(result.is_error);
	assert!(result.output.contains("Tool not found"));
}

#[tokio::test]
async fn test_tool_executor_execute_success() {
	let mut registry = make_registry();
	registry.register_mut(make_definition("echo"), make_echo_handler());

	let executor: &dyn ToolExecutor = &registry;
	let call = make_call("c1", "echo");
	let result = executor.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "echo: hello");
}
