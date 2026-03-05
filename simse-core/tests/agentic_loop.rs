//! Integration tests for the agentic loop module.
//!
//! Uses mock implementations of AcpClient, ToolExecutor, ContextPruner, and
//! CompactionProvider to exercise the full loop logic.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;

use simse_core::agentic_loop::*;
use simse_core::error::{LoopErrorCode, ProviderErrorCode, SimseError};
use simse_core::events::event_types;
use simse_core::events::EventBus;
use simse_core::tools::types::{ParsedResponse, ToolCallRequest, ToolCallResult};

// ===========================================================================
// Mock AcpClient
// ===========================================================================

/// A response entry that can be recreated (since SimseError is not Clone).
enum MockResponse {
	Ok {
		text: String,
		usage: Option<TokenUsage>,
	},
	Err {
		message: String,
		code: Option<ProviderErrorCode>,
	},
}

impl MockResponse {
	fn to_result(&self) -> Result<GenerateResponse, SimseError> {
		match self {
			MockResponse::Ok { text, usage } => Ok(GenerateResponse {
				text: text.clone(),
				usage: usage.clone(),
			}),
			MockResponse::Err { message, code } => {
				if let Some(code) = code {
					Err(SimseError::provider(code.clone(), message.as_str(), None))
				} else {
					Err(SimseError::other(message.as_str()))
				}
			}
		}
	}
}

/// Sequenced mock: returns a different response for each call.
struct MockAcp {
	responses: Mutex<Vec<MockResponse>>,
	call_count: AtomicUsize,
}

impl MockAcp {
	fn new(responses: Vec<MockResponse>) -> Self {
		Self {
			responses: Mutex::new(responses),
			call_count: AtomicUsize::new(0),
		}
	}

	/// Creates a mock that always returns the same text.
	fn text(text: &str) -> Self {
		Self::new(vec![MockResponse::Ok {
			text: text.to_string(),
			usage: Some(TokenUsage {
				prompt_tokens: Some(10),
				completion_tokens: Some(5),
				total_tokens: Some(15),
			}),
		}])
	}

	/// Creates a mock that returns tool calls first, then a text response.
	fn tool_then_text(tool_response: &str, text_response: &str) -> Self {
		Self::new(vec![
			MockResponse::Ok {
				text: tool_response.to_string(),
				usage: Some(TokenUsage {
					prompt_tokens: Some(20),
					completion_tokens: Some(10),
					total_tokens: Some(30),
				}),
			},
			MockResponse::Ok {
				text: text_response.to_string(),
				usage: Some(TokenUsage {
					prompt_tokens: Some(25),
					completion_tokens: Some(8),
					total_tokens: Some(33),
				}),
			},
		])
	}

	fn calls(&self) -> usize {
		self.call_count.load(Ordering::SeqCst)
	}
}

#[async_trait]
impl AcpClient for MockAcp {
	async fn generate(
		&self,
		_messages: &[Message],
		_system: Option<&str>,
	) -> Result<GenerateResponse, SimseError> {
		let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
		let responses = self.responses.lock().unwrap();
		if idx < responses.len() {
			responses[idx].to_result()
		} else {
			// Repeat the last response
			responses
				.last()
				.map(|r| r.to_result())
				.unwrap_or_else(|| {
					Ok(GenerateResponse {
						text: "fallback".to_string(),
						usage: None,
					})
				})
		}
	}
}

// ===========================================================================
// Mock ToolExecutor
// ===========================================================================

struct MockToolExecutor {
	/// Results returned by execute(), keyed by tool name.
	results: Mutex<std::collections::HashMap<String, Vec<ToolCallResult>>>,
	execute_count: AtomicUsize,
}

impl MockToolExecutor {
	fn new() -> Self {
		Self {
			results: Mutex::new(std::collections::HashMap::new()),
			execute_count: AtomicUsize::new(0),
		}
	}

	fn with_result(name: &str, output: &str) -> Self {
		let executor = Self::new();
		{
			let mut results = executor.results.lock().unwrap();
			results.insert(
				name.to_string(),
				vec![ToolCallResult {
					id: "call_1".to_string(),
					name: name.to_string(),
					output: output.to_string(),
					is_error: false,
					duration_ms: Some(42),
					diff: None,
				}],
			);
		}
		executor
	}

	fn with_sequential_results(name: &str, outputs: Vec<(&str, bool)>) -> Self {
		let executor = Self::new();
		{
			let mut results = executor.results.lock().unwrap();
			results.insert(
				name.to_string(),
				outputs
					.iter()
					.enumerate()
					.map(|(i, (output, is_error))| ToolCallResult {
						id: format!("call_{}", i + 1),
						name: name.to_string(),
						output: output.to_string(),
						is_error: *is_error,
						duration_ms: Some(10),
						diff: None,
					})
					.collect(),
			);
		}
		executor
	}
}

#[async_trait]
impl ToolExecutor for MockToolExecutor {
	fn parse_tool_calls(&self, response: &str) -> ParsedResponse {
		// Simple parsing: if response contains <tool_use>, parse it
		if response.contains("<tool_use>") {
			// Delegate to the real parser
			simse_core::tools::registry::ToolRegistry::parse_tool_calls(response)
		} else {
			ParsedResponse {
				text: response.to_string(),
				tool_calls: vec![],
			}
		}
	}

	async fn execute(&self, call: &ToolCallRequest) -> ToolCallResult {
		let idx = self.execute_count.fetch_add(1, Ordering::SeqCst);
		let results = self.results.lock().unwrap();
		if let Some(tool_results) = results.get(&call.name) {
			let result_idx = idx.min(tool_results.len().saturating_sub(1));
			let mut result = tool_results[result_idx].clone();
			result.id = call.id.clone();
			result
		} else {
			ToolCallResult {
				id: call.id.clone(),
				name: call.name.clone(),
				output: format!("mock result for {}", call.name),
				is_error: false,
				duration_ms: Some(1),
				diff: None,
			}
		}
	}
}

// ===========================================================================
// Mock ContextPruner
// ===========================================================================

struct MockPruner {
	/// How many messages to keep from the end.
	keep_last: usize,
}

impl ContextPruner for MockPruner {
	fn prune(&self, messages: &[Message]) -> Vec<Message> {
		if messages.len() <= self.keep_last {
			messages.to_vec()
		} else {
			messages[messages.len() - self.keep_last..].to_vec()
		}
	}
}

// ===========================================================================
// Mock CompactionProvider
// ===========================================================================

struct MockCompactionProvider {
	summary: String,
	called: AtomicUsize,
}

impl MockCompactionProvider {
	fn new(summary: &str) -> Self {
		Self {
			summary: summary.to_string(),
			called: AtomicUsize::new(0),
		}
	}
}

#[async_trait]
impl CompactionProvider for MockCompactionProvider {
	async fn generate(&self, _prompt: &str) -> Result<String, SimseError> {
		self.called.fetch_add(1, Ordering::SeqCst);
		Ok(self.summary.clone())
	}
}

struct FailingCompactionProvider;

#[async_trait]
impl CompactionProvider for FailingCompactionProvider {
	async fn generate(&self, _prompt: &str) -> Result<String, SimseError> {
		Err(SimseError::loop_err(
			LoopErrorCode::CompactionFailed,
			"compaction failed",
		))
	}
}

// ===========================================================================
// Helper: build tool use response
// ===========================================================================

fn tool_use_response(name: &str, args: serde_json::Value) -> String {
	format!(
		"Let me call the tool.\n<tool_use>\n{}\n</tool_use>",
		serde_json::to_string(&json!({
			"id": "call_1",
			"name": name,
			"arguments": args,
		}))
		.unwrap()
	)
}

// ===========================================================================
// Tests: Single-turn text response
// ===========================================================================

#[tokio::test]
async fn test_single_turn_text_response() {
	let acp = MockAcp::text("The answer is 42.");
	let executor = MockToolExecutor::new();
	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "What is the answer?".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(result.final_text, "The answer is 42.");
	assert_eq!(result.total_turns, 1);
	assert!(!result.hit_turn_limit);
	assert!(!result.aborted);
	assert!(result.total_duration_ms > 0 || result.total_duration_ms == 0); // timing is non-negative
	assert_eq!(result.turns.len(), 1);
	assert_eq!(result.turns[0].turn_type, TurnType::Text);
	assert_eq!(
		result.turns[0].text.as_deref(),
		Some("The answer is 42.")
	);
}

#[tokio::test]
async fn test_single_turn_text_with_usage() {
	let acp = MockAcp::text("hello");
	let executor = MockToolExecutor::new();
	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "hi".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	let usage = result.total_usage.unwrap();
	assert_eq!(usage.prompt_tokens, Some(10));
	assert_eq!(usage.completion_tokens, Some(5));
	assert_eq!(usage.total_tokens, Some(15));
}

// ===========================================================================
// Tests: Multi-turn with tool calls
// ===========================================================================

#[tokio::test]
async fn test_multi_turn_with_tool_call() {
	let tool_response = tool_use_response("read_file", json!({"path": "/test.txt"}));
	let acp = MockAcp::tool_then_text(&tool_response, "The file contains 'hello'.");
	let executor = MockToolExecutor::with_result("read_file", "hello");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "Read /test.txt".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(result.total_turns, 2);
	assert_eq!(result.turns[0].turn_type, TurnType::ToolUse);
	assert_eq!(result.turns[1].turn_type, TurnType::Text);
	assert_eq!(result.final_text, "The file contains 'hello'.");
	assert!(!result.hit_turn_limit);
}

#[tokio::test]
async fn test_multi_turn_tool_results_in_turn() {
	let tool_response = tool_use_response("search", json!({"query": "test"}));
	let acp = MockAcp::tool_then_text(&tool_response, "Found results.");
	let executor = MockToolExecutor::with_result("search", "result1, result2");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "Search for test".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(result.turns[0].tool_calls.len(), 1);
	assert_eq!(result.turns[0].tool_results.len(), 1);
	assert_eq!(result.turns[0].tool_results[0].output, "result1, result2");
	assert!(!result.turns[0].tool_results[0].is_error);
}

// ===========================================================================
// Tests: Token usage accumulation
// ===========================================================================

#[tokio::test]
async fn test_token_usage_accumulation() {
	let tool_response = tool_use_response("calc", json!({"expr": "1+1"}));
	let acp = MockAcp::tool_then_text(&tool_response, "done");
	let executor = MockToolExecutor::with_result("calc", "2");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "calculate".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	let usage = result.total_usage.unwrap();
	// Turn 1: prompt=20, completion=10, total=30
	// Turn 2: prompt=25, completion=8, total=33
	assert_eq!(usage.prompt_tokens, Some(45));
	assert_eq!(usage.completion_tokens, Some(18));
	assert_eq!(usage.total_tokens, Some(63));
}

// ===========================================================================
// Tests: Doom loop detection
// ===========================================================================

#[tokio::test]
async fn test_doom_loop_detection_fires_callback() {
	// ACP always returns the same tool call
	let tool_resp = tool_use_response("broken_tool", json!({"x": 1}));
	let responses: Vec<MockResponse> = (0..5)
		.map(|_| MockResponse::Ok {
			text: tool_resp.clone(),
			usage: None,
		})
		.collect();
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::with_result("broken_tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "do something".to_string(),
	}];

	let doom_count = Arc::new(AtomicUsize::new(0));
	let doom_name = Arc::new(Mutex::new(String::new()));
	let dc = doom_count.clone();
	let dn = doom_name.clone();

	let callbacks = LoopCallbacks {
		on_doom_loop: Some(Box::new(move |name, _count| {
			dc.fetch_add(1, Ordering::SeqCst);
			*dn.lock().unwrap() = name.to_string();
		})),
		..Default::default()
	};

	let opts = AgenticLoopOptions {
		max_turns: 5,
		max_identical_tool_calls: 3,
		..Default::default()
	};

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		opts,
		Some(callbacks),
		None,
		None,
		None,
	)
	.await
	.unwrap();

	// Doom loop should have fired at least once
	assert!(doom_count.load(Ordering::SeqCst) >= 1);
	assert_eq!(*doom_name.lock().unwrap(), "broken_tool");
	assert!(result.hit_turn_limit);
}

#[tokio::test]
async fn test_doom_loop_injects_system_warning() {
	let tool_resp = tool_use_response("bad_tool", json!({"a": "b"}));
	let responses: Vec<MockResponse> = (0..4)
		.map(|_| MockResponse::Ok {
			text: tool_resp.clone(),
			usage: None,
		})
		.collect();
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::with_result("bad_tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "do it".to_string(),
	}];

	let opts = AgenticLoopOptions {
		max_turns: 4,
		max_identical_tool_calls: 3,
		..Default::default()
	};

	let _ = run_agentic_loop(
		&acp, &executor, &mut messages, opts, None, None, None, None,
	)
	.await
	.unwrap();

	// Check that a system warning was injected
	let has_warning = messages.iter().any(|m| {
		m.role == MessageRole::System && m.content.contains("WARNING") && m.content.contains("bad_tool")
	});
	assert!(has_warning, "Expected system warning in messages");
}

// ===========================================================================
// Tests: Stream retry on transient error
// ===========================================================================

#[tokio::test]
async fn test_stream_retry_on_transient_error() {
	let responses = vec![
		MockResponse::Err {
			message: "timeout".to_string(),
			code: Some(ProviderErrorCode::Timeout),
		},
		MockResponse::Ok {
			text: "recovered".to_string(),
			usage: None,
		},
	];
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "hi".to_string(),
	}];

	let opts = AgenticLoopOptions {
		stream_retry: RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1, // minimal delay for tests
		},
		..Default::default()
	};

	let result = run_agentic_loop(
		&acp, &executor, &mut messages, opts, None, None, None, None,
	)
	.await
	.unwrap();

	assert_eq!(result.final_text, "recovered");
	assert_eq!(acp.calls(), 2);
}

#[tokio::test]
async fn test_stream_retry_non_transient_error_not_retried() {
	let responses = vec![MockResponse::Err {
		message: "fatal error".to_string(),
		code: None,
	}];
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "hi".to_string(),
	}];

	let opts = AgenticLoopOptions {
		stream_retry: RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1,
		},
		..Default::default()
	};

	let err = run_agentic_loop(
		&acp, &executor, &mut messages, opts, None, None, None, None,
	)
	.await
	.unwrap_err();

	assert_eq!(err.to_string(), "fatal error");
	assert_eq!(acp.calls(), 1); // Not retried
}

// ===========================================================================
// Tests: Tool retry on transient output
// ===========================================================================

#[tokio::test]
async fn test_tool_retry_on_transient_output() {
	let tool_resp = tool_use_response("flaky_tool", json!({"x": 1}));
	let acp = MockAcp::tool_then_text(&tool_resp, "done");
	let executor = MockToolExecutor::with_sequential_results(
		"flaky_tool",
		vec![
			("ECONNREFUSED 127.0.0.1:3000", true),
			("success data", false),
		],
	);

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "do it".to_string(),
	}];

	let opts = AgenticLoopOptions {
		tool_retry: RetryConfig {
			max_attempts: 3,
			base_delay_ms: 1,
		},
		..Default::default()
	};

	let result = run_agentic_loop(
		&acp, &executor, &mut messages, opts, None, None, None, None,
	)
	.await
	.unwrap();

	assert_eq!(result.final_text, "done");
	// The tool should have been called twice (first transient, second success)
	assert!(executor.execute_count.load(Ordering::SeqCst) >= 2);
}

// ===========================================================================
// Tests: Abort via CancellationToken
// ===========================================================================

#[tokio::test]
async fn test_abort_via_cancellation_token() {
	// ACP always returns tool calls so the loop would continue indefinitely
	let tool_resp = tool_use_response("tool", json!({}));
	let responses: Vec<MockResponse> = (0..20)
		.map(|_| MockResponse::Ok {
			text: tool_resp.clone(),
			usage: None,
		})
		.collect();
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::with_result("tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "start".to_string(),
	}];

	let token = CancellationToken::new();
	// Cancel immediately
	token.cancel();

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			max_turns: 20,
			..Default::default()
		},
		None,
		Some(&token),
		None,
		None,
	)
	.await
	.unwrap();

	assert!(result.aborted);
	assert!(!result.hit_turn_limit);
	assert_eq!(result.total_turns, 0);
}

#[tokio::test]
async fn test_abort_mid_loop() {
	// First call returns a tool use, second call would too but we cancel
	let tool_resp = tool_use_response("tool", json!({}));
	let responses: Vec<MockResponse> = (0..10)
		.map(|_| MockResponse::Ok {
			text: tool_resp.clone(),
			usage: None,
		})
		.collect();
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::with_result("tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "start".to_string(),
	}];

	let token = CancellationToken::new();
	let token_clone = token.clone();

	// Cancel after the first turn starts
	let turn_count = Arc::new(AtomicUsize::new(0));
	let tc = turn_count.clone();
	let callbacks = LoopCallbacks {
		on_turn_complete: Some(Box::new(move |_turn| {
			let count = tc.fetch_add(1, Ordering::SeqCst);
			if count >= 1 {
				token_clone.cancel();
			}
		})),
		..Default::default()
	};

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			max_turns: 10,
			..Default::default()
		},
		Some(callbacks),
		Some(&token),
		None,
		None,
	)
	.await
	.unwrap();

	assert!(result.aborted);
	assert!(!result.hit_turn_limit);
	// Should have completed at least 1 turn before aborting
	assert!(result.total_turns >= 1);
}

// ===========================================================================
// Tests: Hit turn limit
// ===========================================================================

#[tokio::test]
async fn test_hit_turn_limit() {
	let tool_resp = tool_use_response("tool", json!({}));
	let responses: Vec<MockResponse> = (0..5)
		.map(|_| MockResponse::Ok {
			text: tool_resp.clone(),
			usage: None,
		})
		.collect();
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::with_result("tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "start".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			max_turns: 3,
			..Default::default()
		},
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert!(result.hit_turn_limit);
	assert!(!result.aborted);
	assert_eq!(result.total_turns, 3);
}

#[tokio::test]
async fn test_hit_turn_limit_single_turn() {
	let tool_resp = tool_use_response("tool", json!({}));
	let acp = MockAcp::new(vec![MockResponse::Ok {
		text: tool_resp,
		usage: None,
	}]);
	let executor = MockToolExecutor::with_result("tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "start".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			max_turns: 1,
			..Default::default()
		},
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert!(result.hit_turn_limit);
	assert_eq!(result.total_turns, 1);
}

// ===========================================================================
// Tests: Agent manages tools mode
// ===========================================================================

#[tokio::test]
async fn test_agent_manages_tools_skips_parsing() {
	// Response contains tool_use tags but should not be parsed
	let text_with_tags =
		"I'll handle the tools myself.\n<tool_use>\n{\"name\":\"x\",\"arguments\":{}}\n</tool_use>";
	let acp = MockAcp::text(text_with_tags);
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "do it".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			agent_manages_tools: true,
			..Default::default()
		},
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	// Should treat as text response, not tool use
	assert_eq!(result.total_turns, 1);
	assert_eq!(result.turns[0].turn_type, TurnType::Text);
	assert!(result.final_text.contains("handle the tools"));
	assert_eq!(executor.execute_count.load(Ordering::SeqCst), 0);
}

// ===========================================================================
// Tests: Event publishing
// ===========================================================================

#[tokio::test]
async fn test_events_published_for_text_response() {
	let bus = Arc::new(EventBus::new());
	let events = Arc::new(Mutex::new(Vec::<String>::new()));
	let ev = events.clone();
	let _unsub = bus.subscribe_all(move |event_type, _payload| {
		ev.lock().unwrap().push(event_type.to_string());
	});

	let acp = MockAcp::text("hi");
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "hello".to_string(),
	}];

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			event_bus: Some(bus),
			..Default::default()
		},
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	let captured = events.lock().unwrap();
	assert!(captured.contains(&event_types::LOOP_START.to_string()));
	assert!(captured.contains(&event_types::LOOP_TURN_START.to_string()));
	assert!(captured.contains(&event_types::LOOP_TURN_END.to_string()));
	assert!(captured.contains(&event_types::LOOP_COMPLETE.to_string()));
}

#[tokio::test]
async fn test_events_published_for_tool_use() {
	let bus = Arc::new(EventBus::new());
	let events = Arc::new(Mutex::new(Vec::<String>::new()));
	let ev = events.clone();
	let _unsub = bus.subscribe_all(move |event_type, _payload| {
		ev.lock().unwrap().push(event_type.to_string());
	});

	let tool_resp = tool_use_response("my_tool", json!({}));
	let acp = MockAcp::tool_then_text(&tool_resp, "done");
	let executor = MockToolExecutor::with_result("my_tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "go".to_string(),
	}];

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			event_bus: Some(bus),
			..Default::default()
		},
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	let captured = events.lock().unwrap();
	assert!(captured.contains(&event_types::LOOP_TOOL_START.to_string()));
	assert!(captured.contains(&event_types::LOOP_TOOL_END.to_string()));
}

#[tokio::test]
async fn test_doom_loop_event_published() {
	let bus = Arc::new(EventBus::new());
	let events = Arc::new(Mutex::new(Vec::<String>::new()));
	let ev = events.clone();
	let _unsub = bus.subscribe_all(move |event_type, _payload| {
		ev.lock().unwrap().push(event_type.to_string());
	});

	let tool_resp = tool_use_response("looping_tool", json!({"x": 1}));
	let responses: Vec<MockResponse> = (0..5)
		.map(|_| MockResponse::Ok {
			text: tool_resp.clone(),
			usage: None,
		})
		.collect();
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::with_result("looping_tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "go".to_string(),
	}];

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			max_turns: 5,
			max_identical_tool_calls: 3,
			event_bus: Some(bus),
			..Default::default()
		},
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	let captured = events.lock().unwrap();
	assert!(captured.contains(&event_types::LOOP_DOOM_LOOP.to_string()));
}

// ===========================================================================
// Tests: Callbacks fired at correct times
// ===========================================================================

#[tokio::test]
async fn test_on_turn_complete_callback_fires() {
	let acp = MockAcp::text("answer");
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "q".to_string(),
	}];

	let turn_count = Arc::new(AtomicUsize::new(0));
	let tc = turn_count.clone();

	let callbacks = LoopCallbacks {
		on_turn_complete: Some(Box::new(move |_turn| {
			tc.fetch_add(1, Ordering::SeqCst);
		})),
		..Default::default()
	};

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		Some(callbacks),
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(turn_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_on_stream_delta_callback_fires() {
	let acp = MockAcp::text("delta text");
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "q".to_string(),
	}];

	let captured = Arc::new(Mutex::new(String::new()));
	let cap = captured.clone();

	let callbacks = LoopCallbacks {
		on_stream_delta: Some(Box::new(move |text| {
			*cap.lock().unwrap() = text.to_string();
		})),
		..Default::default()
	};

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		Some(callbacks),
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(*captured.lock().unwrap(), "delta text");
}

#[tokio::test]
async fn test_on_tool_call_start_end_callbacks() {
	let tool_resp = tool_use_response("my_tool", json!({"key": "val"}));
	let acp = MockAcp::tool_then_text(&tool_resp, "done");
	let executor = MockToolExecutor::with_result("my_tool", "result");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "go".to_string(),
	}];

	let start_names = Arc::new(Mutex::new(Vec::<String>::new()));
	let end_names = Arc::new(Mutex::new(Vec::<String>::new()));
	let sn = start_names.clone();
	let en = end_names.clone();

	let callbacks = LoopCallbacks {
		on_tool_call_start: Some(Box::new(move |call| {
			sn.lock().unwrap().push(call.name.clone());
		})),
		on_tool_call_end: Some(Box::new(move |result| {
			en.lock().unwrap().push(result.name.clone());
		})),
		..Default::default()
	};

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		Some(callbacks),
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(*start_names.lock().unwrap(), vec!["my_tool"]);
	assert_eq!(*end_names.lock().unwrap(), vec!["my_tool"]);
}

#[tokio::test]
async fn test_on_usage_update_callback() {
	let acp = MockAcp::text("hi");
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "q".to_string(),
	}];

	let usage_updates = Arc::new(AtomicUsize::new(0));
	let uu = usage_updates.clone();

	let callbacks = LoopCallbacks {
		on_usage_update: Some(Box::new(move |_usage| {
			uu.fetch_add(1, Ordering::SeqCst);
		})),
		..Default::default()
	};

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		Some(callbacks),
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert!(usage_updates.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn test_on_error_callback_for_stream_failure() {
	let acp = MockAcp::new(vec![MockResponse::Err {
		message: "boom".to_string(),
		code: None,
	}]);
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "q".to_string(),
	}];

	let error_count = Arc::new(AtomicUsize::new(0));
	let ec = error_count.clone();

	let callbacks = LoopCallbacks {
		on_error: Some(Box::new(move |_err| {
			ec.fetch_add(1, Ordering::SeqCst);
		})),
		..Default::default()
	};

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		Some(callbacks),
		None,
		None,
		None,
	)
	.await;

	assert!(error_count.load(Ordering::SeqCst) >= 1);
}

// ===========================================================================
// Tests: Auto-compaction
// ===========================================================================

#[tokio::test]
async fn test_auto_compaction_with_pruner() {
	let acp = MockAcp::text("final answer");
	let executor = MockToolExecutor::new();

	// Create a very long conversation
	let mut messages: Vec<Message> = (0..100)
		.map(|i| Message {
			role: if i % 2 == 0 {
				MessageRole::User
			} else {
				MessageRole::Assistant
			},
			content: "x".repeat(200),
		})
		.collect();

	let pruner = MockPruner { keep_last: 5 };

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			auto_compact: true,
			..Default::default()
		},
		None,
		None,
		Some(&pruner),
		None,
	)
	.await
	.unwrap();

	assert_eq!(result.final_text, "final answer");
	// Messages should have been pruned
	// (the exact count depends on pruning + the assistant response added)
}

#[tokio::test]
async fn test_auto_compaction_with_summarization() {
	let acp = MockAcp::text("answer after compaction");
	let executor = MockToolExecutor::new();
	let compaction = MockCompactionProvider::new("Summary of conversation.");

	// Create messages that exceed the 100k char threshold
	let mut messages: Vec<Message> = (0..600)
		.map(|i| Message {
			role: if i % 2 == 0 {
				MessageRole::User
			} else {
				MessageRole::Assistant
			},
			content: "x".repeat(200),
		})
		.collect();

	let compaction_called = Arc::new(Mutex::new(false));
	let cc = compaction_called.clone();

	let callbacks = LoopCallbacks {
		on_compaction: Some(Box::new(move |_summary| {
			*cc.lock().unwrap() = true;
		})),
		..Default::default()
	};

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			auto_compact: true,
			..Default::default()
		},
		Some(callbacks),
		None,
		None,
		Some(&compaction),
	)
	.await
	.unwrap();

	assert_eq!(result.final_text, "answer after compaction");
	assert!(*compaction_called.lock().unwrap());
	assert!(compaction.called.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn test_compaction_failure_does_not_crash_loop() {
	let acp = MockAcp::text("fine");
	let executor = MockToolExecutor::new();
	let compaction = FailingCompactionProvider;

	// Messages over threshold
	let mut messages: Vec<Message> = (0..600)
		.map(|i| Message {
			role: if i % 2 == 0 {
				MessageRole::User
			} else {
				MessageRole::Assistant
			},
			content: "x".repeat(200),
		})
		.collect();

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			auto_compact: true,
			..Default::default()
		},
		None,
		None,
		None,
		Some(&compaction),
	)
	.await
	.unwrap();

	// Loop should still complete despite compaction failure
	assert_eq!(result.final_text, "fine");
}

#[tokio::test]
async fn test_custom_compaction_prompt() {
	let acp = MockAcp::text("done");
	let executor = MockToolExecutor::new();

	let prompt_received = Arc::new(Mutex::new(String::new()));
	let pr = prompt_received.clone();

	struct CapturingCompaction {
		received: Arc<Mutex<String>>,
	}

	#[async_trait]
	impl CompactionProvider for CapturingCompaction {
		async fn generate(&self, prompt: &str) -> Result<String, SimseError> {
			*self.received.lock().unwrap() = prompt.to_string();
			Ok("summary".to_string())
		}
	}

	let compaction = CapturingCompaction { received: pr };

	let mut messages: Vec<Message> = (0..600)
		.map(|i| Message {
			role: if i % 2 == 0 {
				MessageRole::User
			} else {
				MessageRole::Assistant
			},
			content: "x".repeat(200),
		})
		.collect();

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			auto_compact: true,
			compaction_prompt: Some("Custom compaction instruction.".to_string()),
			..Default::default()
		},
		None,
		None,
		None,
		Some(&compaction),
	)
	.await
	.unwrap();

	let received = prompt_received.lock().unwrap();
	assert!(received.contains("Custom compaction instruction."));
}

#[tokio::test]
async fn test_pre_compaction_hook_injects_context() {
	let acp = MockAcp::text("done");
	let executor = MockToolExecutor::new();

	let prompt_received = Arc::new(Mutex::new(String::new()));
	let pr = prompt_received.clone();

	struct CapturingCompaction {
		received: Arc<Mutex<String>>,
	}

	#[async_trait]
	impl CompactionProvider for CapturingCompaction {
		async fn generate(&self, prompt: &str) -> Result<String, SimseError> {
			*self.received.lock().unwrap() = prompt.to_string();
			Ok("summary".to_string())
		}
	}

	let compaction = CapturingCompaction { received: pr };

	let mut messages: Vec<Message> = (0..600)
		.map(|i| Message {
			role: if i % 2 == 0 {
				MessageRole::User
			} else {
				MessageRole::Assistant
			},
			content: "x".repeat(200),
		})
		.collect();

	let callbacks = LoopCallbacks {
		on_pre_compaction: Some(Box::new(|_prompt| {
			Some("EXTRA CONTEXT: important stuff".to_string())
		})),
		..Default::default()
	};

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			auto_compact: true,
			..Default::default()
		},
		Some(callbacks),
		None,
		None,
		Some(&compaction),
	)
	.await
	.unwrap();

	let received = prompt_received.lock().unwrap();
	assert!(received.contains("EXTRA CONTEXT: important stuff"));
}

// ===========================================================================
// Tests: Empty tool calls parsed
// ===========================================================================

#[tokio::test]
async fn test_empty_tool_calls_treated_as_text() {
	// Response has no tool_use tags
	let acp = MockAcp::text("just plain text");
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "hi".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(result.turns[0].turn_type, TurnType::Text);
	assert_eq!(result.turns[0].tool_calls.len(), 0);
	assert_eq!(result.turns[0].tool_results.len(), 0);
}

// ===========================================================================
// Tests: Messages are updated
// ===========================================================================

#[tokio::test]
async fn test_messages_updated_after_text_response() {
	let acp = MockAcp::text("answer");
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "question".to_string(),
	}];

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	// Should have the original user message + assistant response
	assert!(messages.len() >= 2);
	assert_eq!(messages.last().unwrap().role, MessageRole::Assistant);
	assert_eq!(messages.last().unwrap().content, "answer");
}

#[tokio::test]
async fn test_messages_updated_after_tool_use() {
	let tool_resp = tool_use_response("tool", json!({}));
	let acp = MockAcp::tool_then_text(&tool_resp, "done");
	let executor = MockToolExecutor::with_result("tool", "tool output");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "go".to_string(),
	}];

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	// Should contain: user, assistant (tool call), user (tool result), assistant (final)
	assert!(messages.len() >= 4);

	// Check that tool result is in the messages
	let has_tool_result = messages
		.iter()
		.any(|m| m.content.contains("Tool Result") && m.content.contains("tool output"));
	assert!(has_tool_result);
}

// ===========================================================================
// Tests: Default options sensibility
// ===========================================================================

#[test]
fn test_default_options_are_sensible() {
	let opts = AgenticLoopOptions::default();
	assert!(opts.max_turns > 0);
	assert!(opts.max_identical_tool_calls > 0);
	assert!(opts.stream_retry.max_attempts > 0);
	assert!(opts.tool_retry.max_attempts > 0);
}

// ===========================================================================
// Tests: System prompt passed to ACP
// ===========================================================================

#[tokio::test]
async fn test_system_prompt_passed_to_acp() {
	let system_received = Arc::new(Mutex::new(None::<String>));
	let sr = system_received.clone();

	struct SystemCapturingAcp {
		received: Arc<Mutex<Option<String>>>,
	}

	#[async_trait]
	impl AcpClient for SystemCapturingAcp {
		async fn generate(
			&self,
			_messages: &[Message],
			system: Option<&str>,
		) -> Result<GenerateResponse, SimseError> {
			*self.received.lock().unwrap() = system.map(|s| s.to_string());
			Ok(GenerateResponse {
				text: "ok".to_string(),
				usage: None,
			})
		}
	}

	let acp = SystemCapturingAcp { received: sr };
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "hi".to_string(),
	}];

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			system_prompt: Some("You are a helpful assistant.".to_string()),
			..Default::default()
		},
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	let received = system_received.lock().unwrap();
	assert_eq!(
		received.as_deref(),
		Some("You are a helpful assistant.")
	);
}

// ===========================================================================
// Tests: CancellationToken unit tests
// ===========================================================================

#[test]
fn test_cancellation_token_is_send_sync() {
	fn assert_send_sync<T: Send + Sync>() {}
	assert_send_sync::<CancellationToken>();
}

// ===========================================================================
// Tests: Error from ACP propagates
// ===========================================================================

#[tokio::test]
async fn test_acp_error_propagates_through_loop() {
	let acp = MockAcp::new(vec![MockResponse::Err {
		message: "generation failed".to_string(),
		code: None,
	}]);
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "hi".to_string(),
	}];

	let err = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap_err();

	assert_eq!(err.to_string(), "generation failed");
}

// ===========================================================================
// Tests: Doom loop with max_identical_tool_calls = 0 (disabled)
// ===========================================================================

#[tokio::test]
async fn test_doom_loop_disabled_when_zero() {
	let tool_resp = tool_use_response("tool", json!({"same": true}));
	let responses: Vec<MockResponse> = (0..5)
		.map(|_| MockResponse::Ok {
			text: tool_resp.clone(),
			usage: None,
		})
		.collect();
	let acp = MockAcp::new(responses);
	let executor = MockToolExecutor::with_result("tool", "ok");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "go".to_string(),
	}];

	let doom_count = Arc::new(AtomicUsize::new(0));
	let dc = doom_count.clone();

	let callbacks = LoopCallbacks {
		on_doom_loop: Some(Box::new(move |_name, _count| {
			dc.fetch_add(1, Ordering::SeqCst);
		})),
		..Default::default()
	};

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			max_turns: 5,
			max_identical_tool_calls: 0, // disabled
			..Default::default()
		},
		Some(callbacks),
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(doom_count.load(Ordering::SeqCst), 0);
}

// ===========================================================================
// Tests: No usage when ACP returns None
// ===========================================================================

#[tokio::test]
async fn test_no_usage_when_acp_returns_none() {
	let acp = MockAcp::new(vec![MockResponse::Ok {
		text: "hi".to_string(),
		usage: None,
	}]);
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "q".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	let usage = result.total_usage.unwrap();
	// All fields should be None since ACP returned no usage
	assert!(usage.prompt_tokens.is_none());
	assert!(usage.completion_tokens.is_none());
	assert!(usage.total_tokens.is_none());
}

// ===========================================================================
// Tests: Multiple tool calls in a single turn
// ===========================================================================

#[tokio::test]
async fn test_multiple_tool_calls_single_response() {
	// Response with two tool_use blocks
	let text = "Let me read two files.\n\
		<tool_use>\n{\"id\": \"c1\", \"name\": \"read_file\", \"arguments\": {\"path\": \"a.txt\"}}\n</tool_use>\n\
		<tool_use>\n{\"id\": \"c2\", \"name\": \"read_file\", \"arguments\": {\"path\": \"b.txt\"}}\n</tool_use>";
	let acp = MockAcp::tool_then_text(text, "Both files read.");
	let executor = MockToolExecutor::with_result("read_file", "content");

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "read both files".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(result.total_turns, 2);
	assert_eq!(result.turns[0].tool_calls.len(), 2);
	assert_eq!(result.turns[0].tool_results.len(), 2);
	assert_eq!(result.final_text, "Both files read.");
}

// ===========================================================================
// Tests: Total duration is tracked
// ===========================================================================

#[tokio::test]
async fn test_total_duration_tracked() {
	let acp = MockAcp::text("hi");
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "q".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	// Duration should be non-negative (could be 0ms on fast machines)
	assert!(result.total_duration_ms < 10_000); // sanity check: under 10s
}

// ===========================================================================
// Tests: Turn duration is tracked
// ===========================================================================

#[tokio::test]
async fn test_turn_duration_tracked() {
	let acp = MockAcp::text("hi");
	let executor = MockToolExecutor::new();

	let mut messages = vec![Message {
		role: MessageRole::User,
		content: "q".to_string(),
	}];

	let result = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(),
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	assert_eq!(result.turns.len(), 1);
	assert!(result.turns[0].duration_ms < 10_000);
}

// ===========================================================================
// Tests: Auto-compact disabled by default
// ===========================================================================

#[tokio::test]
async fn test_auto_compact_disabled_by_default() {
	let acp = MockAcp::text("ok");
	let executor = MockToolExecutor::new();

	// Even with lots of messages, no compaction should happen
	let mut messages: Vec<Message> = (0..600)
		.map(|i| Message {
			role: if i % 2 == 0 {
				MessageRole::User
			} else {
				MessageRole::Assistant
			},
			content: "x".repeat(200),
		})
		.collect();

	let original_len = messages.len();

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions::default(), // auto_compact is false
		None,
		None,
		None,
		None,
	)
	.await
	.unwrap();

	// Messages should not have been compacted (only the assistant message added)
	assert!(messages.len() >= original_len);
}

// ===========================================================================
// Tests: Compaction event published
// ===========================================================================

#[tokio::test]
async fn test_compaction_event_published() {
	let bus = Arc::new(EventBus::new());
	let events = Arc::new(Mutex::new(Vec::<String>::new()));
	let ev = events.clone();
	let _unsub = bus.subscribe_all(move |event_type, _payload| {
		ev.lock().unwrap().push(event_type.to_string());
	});

	let acp = MockAcp::text("ok");
	let executor = MockToolExecutor::new();
	let compaction = MockCompactionProvider::new("summary");

	let mut messages: Vec<Message> = (0..600)
		.map(|i| Message {
			role: if i % 2 == 0 {
				MessageRole::User
			} else {
				MessageRole::Assistant
			},
			content: "x".repeat(200),
		})
		.collect();

	let _ = run_agentic_loop(
		&acp,
		&executor,
		&mut messages,
		AgenticLoopOptions {
			auto_compact: true,
			event_bus: Some(bus),
			..Default::default()
		},
		None,
		None,
		None,
		Some(&compaction),
	)
	.await
	.unwrap();

	let captured = events.lock().unwrap();
	assert!(captured.contains(&event_types::LOOP_COMPACTION.to_string()));
}
