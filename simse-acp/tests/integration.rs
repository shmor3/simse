// ---------------------------------------------------------------------------
// Integration tests for simse-acp-engine
// ---------------------------------------------------------------------------
//
// These tests spawn the compiled `simse-acp-engine` binary and communicate
// with it over stdin/stdout JSON-RPC 2.0 / NDJSON — same pattern used by
// the TypeScript client layer.
//
// Since we cannot spawn real ACP agent processes in tests, these focus on
// protocol-level verification: JSON-RPC dispatch, error handling, lifecycle
// methods, and robustness under malformed input.
// ---------------------------------------------------------------------------

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Test helper
// ---------------------------------------------------------------------------

struct TestEngine {
	child: Child,
	stdin: Option<ChildStdin>,
	reader: BufReader<ChildStdout>,
	next_id: u64,
}

impl TestEngine {
	fn new() -> Self {
		let binary = env!("CARGO_BIN_EXE_simse-acp-engine");
		let mut child = Command::new(binary)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()
			.expect("Failed to spawn simse-acp-engine");

		let stdin = child.stdin.take().expect("Failed to open stdin");
		let stdout = child.stdout.take().expect("Failed to open stdout");
		let reader = BufReader::new(stdout);

		Self {
			child,
			stdin: Some(stdin),
			reader,
			next_id: 1,
		}
	}

	/// Send a JSON-RPC request and read the response.
	fn request(&mut self, method: &str, params: Value) -> Value {
		let id = self.next_id;
		self.next_id += 1;
		let request = json!({
			"id": id,
			"method": method,
			"params": params,
		});
		let line = serde_json::to_string(&request).unwrap();
		let stdin = self.stdin.as_mut().expect("stdin already closed");
		writeln!(stdin, "{}", line).expect("Failed to write to stdin");
		stdin.flush().expect("Failed to flush stdin");

		let mut response_line = String::new();
		self.reader
			.read_line(&mut response_line)
			.expect("Failed to read response");
		serde_json::from_str(&response_line)
			.unwrap_or_else(|e| panic!("Failed to parse response: {e}\nRaw: {response_line}"))
	}

	/// Send raw text to stdin (for invalid JSON tests).
	fn send_raw(&mut self, text: &str) {
		let stdin = self.stdin.as_mut().expect("stdin already closed");
		writeln!(stdin, "{}", text).expect("Failed to write raw text to stdin");
		stdin.flush().expect("Failed to flush stdin");
	}

	/// Close stdin to signal EOF to the engine.
	fn close_stdin(&mut self) {
		self.stdin.take();
	}

	/// Check that the child process is still running.
	fn is_alive(&mut self) -> bool {
		self.child.try_wait().ok().flatten().is_none()
	}
}

impl Drop for TestEngine {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

fn assert_is_error(resp: &Value, expected_code: i32) {
	assert!(
		resp.get("error").is_some(),
		"Expected error response, got: {resp}"
	);
	let error = &resp["error"];
	assert_eq!(
		error["code"].as_i64().unwrap(),
		expected_code as i64,
		"Expected error code {expected_code}, got: {error}"
	);
}

fn assert_is_success(resp: &Value) {
	assert!(
		resp.get("result").is_some(),
		"Expected success response, got: {resp}"
	);
	assert!(
		resp.get("error").is_none(),
		"Expected no error, but got: {}",
		resp["error"]
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The engine binary starts, stays alive, and can be communicated with.
#[test]
fn test_engine_starts() {
	let mut engine = TestEngine::new();

	// Give a brief moment for the process to initialize its async runtime.
	std::thread::sleep(Duration::from_millis(200));

	assert!(engine.is_alive(), "Engine process should be alive");
}

/// An unknown method should produce a METHOD_NOT_FOUND (-32601) error.
#[test]
fn test_unknown_method() {
	let mut engine = TestEngine::new();
	let resp = engine.request("totally/unknown", json!({}));

	assert_is_error(&resp, -32601);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(
		msg.contains("Unknown method"),
		"Error message should mention unknown method, got: {msg}"
	);
}

/// Calling `acp/generate` before `acp/initialize` should return an
/// ACP error (-32000) indicating the client is not initialized.
#[test]
fn test_generate_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/generate",
		json!({
			"prompt": "Hello",
		}),
	);

	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(
		msg.contains("Not initialized"),
		"Error should mention not initialized, got: {msg}"
	);
}

/// Calling `acp/chat` before `acp/initialize` should return a not-initialized error.
#[test]
fn test_chat_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/chat",
		json!({
			"messages": [
				{ "role": "user", "content": [{ "type": "text", "text": "hi" }] }
			],
		}),
	);

	assert_is_error(&resp, -32000);
	assert!(resp["error"]["message"]
		.as_str()
		.unwrap()
		.contains("Not initialized"));
}

/// Calling `acp/serverHealth` before init returns a not-initialized error.
#[test]
fn test_server_health_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request("acp/serverHealth", json!({}));

	assert_is_error(&resp, -32000);
	assert!(resp["error"]["message"]
		.as_str()
		.unwrap()
		.contains("Not initialized"));
}

/// Calling `acp/listAgents` before init returns a not-initialized error.
#[test]
fn test_list_agents_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request("acp/listAgents", json!({}));

	assert_is_error(&resp, -32000);
	assert!(resp["error"]["message"]
		.as_str()
		.unwrap()
		.contains("Not initialized"));
}

/// Calling `acp/listSessions` before init returns a not-initialized error.
#[test]
fn test_list_sessions_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request("acp/listSessions", json!({}));

	assert_is_error(&resp, -32000);
}

/// Calling `acp/setPermissionPolicy` before init returns a not-initialized error.
#[test]
fn test_set_permission_policy_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/setPermissionPolicy",
		json!({ "policy": "auto-approve" }),
	);

	assert_is_error(&resp, -32000);
	assert!(resp["error"]["message"]
		.as_str()
		.unwrap()
		.contains("Not initialized"));
}

/// Calling `acp/streamStart` before init returns a not-initialized error.
#[test]
fn test_stream_start_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/streamStart",
		json!({ "prompt": "test" }),
	);

	assert_is_error(&resp, -32000);
}

/// Calling `acp/embed` before init returns a not-initialized error.
#[test]
fn test_embed_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/embed",
		json!({ "input": ["hello"] }),
	);

	assert_is_error(&resp, -32000);
}

/// `acp/initialize` with an empty servers list should return an ACP error
/// because AcpClient::new fails when no servers can connect.
#[test]
fn test_initialize_no_servers() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/initialize",
		json!({
			"servers": [],
		}),
	);

	// AcpClient::new returns Err(ServerUnavailable) when servers is empty.
	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(
		msg.contains("No ACP servers configured") || msg.contains("unavailable"),
		"Expected server unavailable error, got: {msg}"
	);
}

/// `acp/dispose` without prior initialization should still succeed.
/// The server clears state and returns an empty object.
#[test]
fn test_dispose_without_init() {
	let mut engine = TestEngine::new();
	let resp = engine.request("acp/dispose", json!({}));

	assert_is_success(&resp);
}

/// `acp/dispose` should be idempotent — calling it twice without
/// re-initializing should both succeed.
#[test]
fn test_dispose_idempotent() {
	let mut engine = TestEngine::new();

	let resp1 = engine.request("acp/dispose", json!({}));
	assert_is_success(&resp1);

	let resp2 = engine.request("acp/dispose", json!({}));
	assert_is_success(&resp2);
}

/// `acp/initialize` with invalid params (missing required `servers` field)
/// should return INVALID_PARAMS (-32602).
#[test]
fn test_initialize_invalid_params() {
	let mut engine = TestEngine::new();
	let resp = engine.request("acp/initialize", json!({}));

	assert_is_error(&resp, -32602);
}

/// `acp/generate` with invalid params (missing `prompt`) should return
/// an error. Since the client check happens first (not initialized),
/// we get -32000 rather than -32602.
#[test]
fn test_generate_invalid_params_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request("acp/generate", json!({}));

	// The "not initialized" check happens before param parsing, so
	// we get -32000 (ACP_ERROR) instead of -32602 (INVALID_PARAMS).
	assert_is_error(&resp, -32000);
}

/// Sending invalid JSON should not crash the engine. The engine logs
/// a warning and discards the invalid line. A subsequent valid request
/// should still be processed correctly.
#[test]
fn test_invalid_json_does_not_crash() {
	let mut engine = TestEngine::new();

	// Send garbage that is not valid JSON.
	engine.send_raw("this is not JSON at all!!!");

	// Send a few more invalid lines to be thorough.
	engine.send_raw("{broken json");
	engine.send_raw("");

	// Give a small moment for the server to process the bad lines.
	std::thread::sleep(Duration::from_millis(100));

	// The engine should still be alive.
	assert!(engine.is_alive(), "Engine should survive invalid JSON");

	// Send a valid request — it should still work.
	let resp = engine.request("totally/unknown", json!({}));
	assert_is_error(&resp, -32601);
}

/// The engine handles multiple sequential requests correctly, with
/// incrementing request IDs.
#[test]
fn test_multiple_requests() {
	let mut engine = TestEngine::new();

	// Request 1: unknown method
	let resp1 = engine.request("unknown/method1", json!({}));
	assert_is_error(&resp1, -32601);
	assert_eq!(resp1["id"].as_u64().unwrap(), 1);

	// Request 2: another unknown method
	let resp2 = engine.request("unknown/method2", json!({}));
	assert_is_error(&resp2, -32601);
	assert_eq!(resp2["id"].as_u64().unwrap(), 2);

	// Request 3: dispose (valid, no init required)
	let resp3 = engine.request("acp/dispose", json!({}));
	assert_is_success(&resp3);
	assert_eq!(resp3["id"].as_u64().unwrap(), 3);

	// Request 4: generate (requires init)
	let resp4 = engine.request("acp/generate", json!({ "prompt": "hi" }));
	assert_is_error(&resp4, -32000);
	assert_eq!(resp4["id"].as_u64().unwrap(), 4);

	// Request 5: unknown method again
	let resp5 = engine.request("nope", json!({}));
	assert_is_error(&resp5, -32601);
	assert_eq!(resp5["id"].as_u64().unwrap(), 5);
}

/// All responses should contain the `"jsonrpc": "2.0"` field.
#[test]
fn test_jsonrpc_version_in_responses() {
	let mut engine = TestEngine::new();

	let resp = engine.request("acp/dispose", json!({}));
	assert_eq!(
		resp["jsonrpc"].as_str().unwrap(),
		"2.0",
		"Response must include jsonrpc: 2.0"
	);

	let resp = engine.request("unknown/method", json!({}));
	assert_eq!(resp["jsonrpc"].as_str().unwrap(), "2.0");

	let resp = engine.request("acp/generate", json!({ "prompt": "test" }));
	assert_eq!(resp["jsonrpc"].as_str().unwrap(), "2.0");
}

/// After dispose, calling methods that require initialization should
/// return not-initialized errors (since dispose clears the client).
#[test]
fn test_dispose_clears_client_state() {
	let mut engine = TestEngine::new();

	// Dispose first (no-op since not initialized, but sets the pattern).
	let resp = engine.request("acp/dispose", json!({}));
	assert_is_success(&resp);

	// Now all client-requiring methods should error.
	let resp = engine.request("acp/generate", json!({ "prompt": "test" }));
	assert_is_error(&resp, -32000);

	let resp = engine.request("acp/listAgents", json!({}));
	assert_is_error(&resp, -32000);

	let resp = engine.request("acp/serverHealth", json!({}));
	assert_is_error(&resp, -32000);
}

/// `acp/permissionResponse` before init should return not-initialized.
#[test]
fn test_permission_response_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/permissionResponse",
		json!({
			"requestId": 1,
			"optionId": "allow_once",
		}),
	);

	assert_is_error(&resp, -32000);
}

/// `acp/loadSession` before init should return not-initialized.
#[test]
fn test_load_session_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/loadSession",
		json!({ "sessionId": "test-session" }),
	);

	assert_is_error(&resp, -32000);
}

/// `acp/deleteSession` before init should return not-initialized.
#[test]
fn test_delete_session_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/deleteSession",
		json!({ "sessionId": "test-session" }),
	);

	assert_is_error(&resp, -32000);
}

/// `acp/setSessionMode` before init should return not-initialized.
#[test]
fn test_set_session_mode_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/setSessionMode",
		json!({
			"sessionId": "test-session",
			"value": "fast",
		}),
	);

	assert_is_error(&resp, -32000);
}

/// `acp/setSessionModel` before init should return not-initialized.
#[test]
fn test_set_session_model_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"acp/setSessionModel",
		json!({
			"sessionId": "test-session",
			"value": "claude-opus-4-6",
		}),
	);

	assert_is_error(&resp, -32000);
}

/// The engine should survive interleaved invalid and valid requests.
#[test]
fn test_robustness_interleaved_invalid_valid() {
	let mut engine = TestEngine::new();

	// Valid request.
	let resp = engine.request("acp/dispose", json!({}));
	assert_is_success(&resp);

	// Invalid JSON.
	engine.send_raw("{{{not valid json at all");

	// Give the server a moment to process.
	std::thread::sleep(Duration::from_millis(50));

	// Valid request should still work.
	let resp = engine.request("acp/dispose", json!({}));
	assert_is_success(&resp);

	// Another burst of garbage.
	engine.send_raw("null");
	engine.send_raw("42");
	engine.send_raw("[1,2,3]");

	std::thread::sleep(Duration::from_millis(50));

	// Still works.
	let resp = engine.request("unknown/test", json!({}));
	assert_is_error(&resp, -32601);

	assert!(engine.is_alive());
}

/// Closing stdin should cause the engine to exit gracefully.
#[test]
fn test_engine_exits_on_stdin_close() {
	let mut engine = TestEngine::new();

	// Send a valid request first to verify it's working.
	let resp = engine.request("acp/dispose", json!({}));
	assert_is_success(&resp);

	// Close stdin to signal EOF.
	engine.close_stdin();

	// Wait for the process to exit (with timeout).
	let mut exited = false;
	for _ in 0..20 {
		std::thread::sleep(Duration::from_millis(100));
		if let Ok(Some(_status)) = engine.child.try_wait() {
			exited = true;
			break;
		}
	}

	assert!(exited, "Engine should exit gracefully when stdin is closed");
}
