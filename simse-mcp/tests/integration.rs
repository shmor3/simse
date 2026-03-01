// ---------------------------------------------------------------------------
// Integration tests for simse-mcp-engine
// ---------------------------------------------------------------------------
//
// These tests spawn the compiled `simse-mcp-engine` binary and communicate
// with it over stdin/stdout JSON-RPC 2.0 / NDJSON -- same pattern used by
// the TypeScript client layer.
//
// Since we cannot spawn real MCP servers in tests, these focus on
// protocol-level verification: JSON-RPC dispatch, error handling, lifecycle
// methods, initialization modes, and robustness under malformed input.
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
		let binary = env!("CARGO_BIN_EXE_simse-mcp-engine");
		let mut child = Command::new(binary)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()
			.expect("Failed to spawn simse-mcp-engine");

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

// -- 1. Engine starts -------------------------------------------------------

/// The engine binary starts, stays alive, and can be communicated with.
#[test]
fn test_engine_starts() {
	let mut engine = TestEngine::new();

	// Give a brief moment for the process to initialize its async runtime.
	std::thread::sleep(Duration::from_millis(200));

	assert!(engine.is_alive(), "Engine process should be alive");
}

// -- 2. Unknown method ------------------------------------------------------

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

// -- 3. Not initialized error -----------------------------------------------

/// Calling `mcp/connect` before `mcp/initialize` should return an
/// MCP error (-32000) indicating the client is not initialized.
#[test]
fn test_not_initialized_error() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"mcp/connect",
		json!({
			"server": "test-server",
		}),
	);

	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(
		msg.contains("Not initialized"),
		"Error should mention not initialized, got: {msg}"
	);
}

// -- 4. Initialize with client config only ----------------------------------

/// Send `mcp/initialize` with only client config -> success.
#[test]
fn test_initialize_client_only() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": {
				"servers": [],
				"clientName": "test-client",
				"clientVersion": "1.0.0"
			}
		}),
	);

	assert_is_success(&resp);
	let result = &resp["result"];
	assert_eq!(result["clientInitialized"], true);
	assert_eq!(result["serverInitialized"], false);
}

// -- 5. Initialize with server config only ----------------------------------

/// Send `mcp/initialize` with only server config -> success.
#[test]
fn test_initialize_server_only() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);

	assert_is_success(&resp);
	let result = &resp["result"];
	assert_eq!(result["clientInitialized"], false);
	assert_eq!(result["serverInitialized"], true);
}

// -- 6. Initialize with both ------------------------------------------------

/// Send `mcp/initialize` with both client and server configs -> success.
#[test]
fn test_initialize_both() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": {
				"servers": [],
				"clientName": "test-client",
				"clientVersion": "1.0.0"
			},
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);

	assert_is_success(&resp);
	let result = &resp["result"];
	assert_eq!(result["clientInitialized"], true);
	assert_eq!(result["serverInitialized"], true);
}

// -- 7. Initialize with invalid params --------------------------------------

/// `mcp/initialize` with a malformed serverConfig (missing required `name`
/// field) should return INVALID_PARAMS (-32602).
#[test]
fn test_initialize_invalid_params() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"serverConfig": {
				"version": "0.1.0"
			}
		}),
	);

	// serverConfig is missing the required `name` field, which causes
	// deserialization of ServerConfigParams to fail -> INVALID_PARAMS.
	assert_is_error(&resp, -32602);
}

// -- 8. List tools not initialized ------------------------------------------

/// Calling `mcp/listTools` before initialization should return an error.
#[test]
fn test_list_tools_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request("mcp/listTools", json!({}));

	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(
		msg.contains("Not initialized"),
		"Expected not-initialized error, got: {msg}"
	);
}

// -- 9. Call tool not initialized -------------------------------------------

/// Calling `mcp/callTool` before initialization should return an error.
#[test]
fn test_call_tool_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request(
		"mcp/callTool",
		json!({
			"server": "test-server",
			"name": "some-tool",
			"arguments": {}
		}),
	);

	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(msg.contains("Not initialized"));
}

// -- 10. List resources not initialized -------------------------------------

/// Calling `mcp/listResources` before initialization should return an error.
#[test]
fn test_list_resources_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request("mcp/listResources", json!({}));

	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(msg.contains("Not initialized"));
}

// -- 11. List prompts not initialized ---------------------------------------

/// Calling `mcp/listPrompts` before initialization should return an error.
#[test]
fn test_list_prompts_not_initialized() {
	let mut engine = TestEngine::new();
	let resp = engine.request("mcp/listPrompts", json!({}));

	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(msg.contains("Not initialized"));
}

// -- 12. Server register tool -----------------------------------------------

/// Initialize with server config, then register a tool via `server/registerTool`.
#[test]
fn test_server_register_tool() {
	let mut engine = TestEngine::new();

	// Initialize with server config.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);
	assert_is_success(&resp);

	// Register a tool.
	let resp = engine.request(
		"server/registerTool",
		json!({
			"name": "my-tool",
			"description": "A test tool",
			"inputSchema": {
				"type": "object",
				"properties": {
					"query": { "type": "string" }
				}
			}
		}),
	);
	assert_is_success(&resp);
}

// -- 13. Server unregister tool ---------------------------------------------

/// Register then unregister a tool, verify success.
#[test]
fn test_server_unregister_tool() {
	let mut engine = TestEngine::new();

	// Initialize with server config.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);
	assert_is_success(&resp);

	// Register a tool.
	let resp = engine.request(
		"server/registerTool",
		json!({
			"name": "ephemeral-tool",
			"description": "Tool to be removed"
		}),
	);
	assert_is_success(&resp);

	// Unregister the tool.
	let resp = engine.request(
		"server/unregisterTool",
		json!({
			"name": "ephemeral-tool"
		}),
	);
	assert_is_success(&resp);
	assert_eq!(resp["result"]["removed"], true);
}

// -- 14. Server start/stop --------------------------------------------------

/// Start and stop the server, verify responses.
#[test]
fn test_server_start_stop() {
	let mut engine = TestEngine::new();

	// Initialize with server config.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);
	assert_is_success(&resp);

	// Start the server.
	let resp = engine.request("server/start", json!({}));
	assert_is_success(&resp);

	// Stop the server.
	let resp = engine.request("server/stop", json!({}));
	assert_is_success(&resp);
}

// -- 15. Set roots ----------------------------------------------------------

/// Send `mcp/setRoots` with roots, verify success.
#[test]
fn test_set_roots() {
	let mut engine = TestEngine::new();

	// Initialize with client config.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": {
				"servers": []
			}
		}),
	);
	assert_is_success(&resp);

	// Set roots.
	let resp = engine.request(
		"mcp/setRoots",
		json!({
			"roots": [
				{ "uri": "file:///workspace", "name": "workspace" },
				{ "uri": "file:///home" }
			]
		}),
	);
	assert_is_success(&resp);
}

// -- 16. Dispose ------------------------------------------------------------

/// Initialize then dispose, verify clean response.
#[test]
fn test_dispose() {
	let mut engine = TestEngine::new();

	// Initialize with both configs.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": {
				"servers": []
			},
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);
	assert_is_success(&resp);

	// Dispose.
	let resp = engine.request("mcp/dispose", json!({}));
	assert_is_success(&resp);
}

// -- 17. Dispose idempotent -------------------------------------------------

/// Double dispose succeeds.
#[test]
fn test_dispose_idempotent() {
	let mut engine = TestEngine::new();

	let resp1 = engine.request("mcp/dispose", json!({}));
	assert_is_success(&resp1);

	let resp2 = engine.request("mcp/dispose", json!({}));
	assert_is_success(&resp2);
}

// -- 18. Dispose clears state -----------------------------------------------

/// After dispose, client-requiring methods should error.
#[test]
fn test_dispose_clears_state() {
	let mut engine = TestEngine::new();

	// Initialize with client config.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": {
				"servers": []
			},
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);
	assert_is_success(&resp);

	// Dispose.
	let resp = engine.request("mcp/dispose", json!({}));
	assert_is_success(&resp);

	// Now all client-requiring methods should error.
	let resp = engine.request("mcp/listTools", json!({}));
	assert_is_error(&resp, -32000);

	let resp = engine.request("mcp/listResources", json!({}));
	assert_is_error(&resp, -32000);

	let resp = engine.request("mcp/listPrompts", json!({}));
	assert_is_error(&resp, -32000);

	let resp = engine.request(
		"mcp/connect",
		json!({ "server": "test" }),
	);
	assert_is_error(&resp, -32000);

	// Server-requiring methods should also error.
	let resp = engine.request("server/start", json!({}));
	assert_is_error(&resp, -32000);

	let resp = engine.request("server/stop", json!({}));
	assert_is_error(&resp, -32000);
}

// -- 19. Multiple requests --------------------------------------------------

/// Send several requests in sequence, all work correctly with incrementing IDs.
#[test]
fn test_multiple_requests() {
	let mut engine = TestEngine::new();

	// Request 1: unknown method.
	let resp1 = engine.request("unknown/method1", json!({}));
	assert_is_error(&resp1, -32601);
	assert_eq!(resp1["id"].as_u64().unwrap(), 1);

	// Request 2: another unknown method.
	let resp2 = engine.request("unknown/method2", json!({}));
	assert_is_error(&resp2, -32601);
	assert_eq!(resp2["id"].as_u64().unwrap(), 2);

	// Request 3: dispose (valid, no init required).
	let resp3 = engine.request("mcp/dispose", json!({}));
	assert_is_success(&resp3);
	assert_eq!(resp3["id"].as_u64().unwrap(), 3);

	// Request 4: listTools (requires init).
	let resp4 = engine.request("mcp/listTools", json!({}));
	assert_is_error(&resp4, -32000);
	assert_eq!(resp4["id"].as_u64().unwrap(), 4);

	// Request 5: initialize, then a method that works.
	let resp5 = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": { "servers": [] }
		}),
	);
	assert_is_success(&resp5);
	assert_eq!(resp5["id"].as_u64().unwrap(), 5);
}

// -- 20. Invalid JSON does not crash ----------------------------------------

/// Sending invalid JSON should not crash the engine. The engine logs a
/// warning and discards the invalid line. A subsequent valid request should
/// still be processed correctly.
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

	// Send a valid request -- it should still work.
	let resp = engine.request("totally/unknown", json!({}));
	assert_is_error(&resp, -32601);
}

// -- 21. Engine exits on stdin close ----------------------------------------

/// Closing stdin should cause the engine to exit gracefully.
#[test]
fn test_engine_exits_on_stdin_close() {
	let mut engine = TestEngine::new();

	// Send a valid request first to verify it's working.
	let resp = engine.request("mcp/dispose", json!({}));
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

// -- 22. JSON-RPC version in responses --------------------------------------

/// All responses should contain the `"jsonrpc": "2.0"` field.
#[test]
fn test_jsonrpc_version_in_responses() {
	let mut engine = TestEngine::new();

	// Success response: dispose.
	let resp = engine.request("mcp/dispose", json!({}));
	assert_eq!(
		resp["jsonrpc"].as_str().unwrap(),
		"2.0",
		"Response must include jsonrpc: 2.0"
	);

	// Error response: unknown method.
	let resp = engine.request("unknown/method", json!({}));
	assert_eq!(resp["jsonrpc"].as_str().unwrap(), "2.0");

	// Error response: not initialized.
	let resp = engine.request("mcp/listTools", json!({}));
	assert_eq!(resp["jsonrpc"].as_str().unwrap(), "2.0");
}

// ---------------------------------------------------------------------------
// Additional robustness tests
// ---------------------------------------------------------------------------

/// The engine should survive interleaved invalid and valid requests.
#[test]
fn test_robustness_interleaved_invalid_valid() {
	let mut engine = TestEngine::new();

	// Valid request.
	let resp = engine.request("mcp/dispose", json!({}));
	assert_is_success(&resp);

	// Invalid JSON.
	engine.send_raw("{{{not valid json at all");

	// Give the server a moment to process.
	std::thread::sleep(Duration::from_millis(50));

	// Valid request should still work.
	let resp = engine.request("mcp/dispose", json!({}));
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

/// Server methods without server initialization should error.
#[test]
fn test_server_start_not_initialized() {
	let mut engine = TestEngine::new();

	// Initialize with only client config (no server config).
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": {
				"servers": []
			}
		}),
	);
	assert_is_success(&resp);

	// Server methods should error since no server was initialized.
	let resp = engine.request("server/start", json!({}));
	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(
		msg.contains("Server not initialized"),
		"Expected server-not-initialized error, got: {msg}"
	);
}

/// Register tool without server initialization should error.
#[test]
fn test_register_tool_not_initialized() {
	let mut engine = TestEngine::new();

	// Initialize with only client config.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": {
				"servers": []
			}
		}),
	);
	assert_is_success(&resp);

	// Register tool should fail -- no server.
	let resp = engine.request(
		"server/registerTool",
		json!({
			"name": "my-tool",
			"description": "test"
		}),
	);
	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(msg.contains("Server not initialized"));
}

/// Unregister tool that does not exist returns removed: false.
#[test]
fn test_unregister_nonexistent_tool() {
	let mut engine = TestEngine::new();

	// Initialize with server config.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"serverConfig": {
				"name": "test-server",
				"version": "0.1.0"
			}
		}),
	);
	assert_is_success(&resp);

	// Unregister a tool that was never registered.
	let resp = engine.request(
		"server/unregisterTool",
		json!({ "name": "nonexistent" }),
	);
	assert_is_success(&resp);
	assert_eq!(resp["result"]["removed"], false);
}

/// `mcp/setRoots` before client initialization should error.
#[test]
fn test_set_roots_not_initialized() {
	let mut engine = TestEngine::new();

	let resp = engine.request(
		"mcp/setRoots",
		json!({
			"roots": [{ "uri": "file:///workspace" }]
		}),
	);
	assert_is_error(&resp, -32000);
}

/// `mcp/connectAll` before initialization should error.
#[test]
fn test_connect_all_not_initialized() {
	let mut engine = TestEngine::new();

	let resp = engine.request("mcp/connectAll", json!({}));
	assert_is_error(&resp, -32000);
	let msg = resp["error"]["message"].as_str().unwrap();
	assert!(msg.contains("Not initialized"));
}

/// `mcp/disconnect` before initialization should error.
#[test]
fn test_disconnect_not_initialized() {
	let mut engine = TestEngine::new();

	let resp = engine.request(
		"mcp/disconnect",
		json!({ "server": "test" }),
	);
	assert_is_error(&resp, -32000);
}

/// `mcp/readResource` before initialization should error.
#[test]
fn test_read_resource_not_initialized() {
	let mut engine = TestEngine::new();

	let resp = engine.request(
		"mcp/readResource",
		json!({
			"server": "test",
			"uri": "file:///test.txt"
		}),
	);
	assert_is_error(&resp, -32000);
}

/// `mcp/listResourceTemplates` before initialization should error.
#[test]
fn test_list_resource_templates_not_initialized() {
	let mut engine = TestEngine::new();

	let resp = engine.request("mcp/listResourceTemplates", json!({}));
	assert_is_error(&resp, -32000);
}

/// `mcp/getPrompt` before initialization should error.
#[test]
fn test_get_prompt_not_initialized() {
	let mut engine = TestEngine::new();

	let resp = engine.request(
		"mcp/getPrompt",
		json!({
			"server": "test",
			"name": "summarize"
		}),
	);
	assert_is_error(&resp, -32000);
}

/// `mcp/setLoggingLevel` before initialization should error.
#[test]
fn test_set_logging_level_not_initialized() {
	let mut engine = TestEngine::new();

	let resp = engine.request(
		"mcp/setLoggingLevel",
		json!({
			"server": "test",
			"level": "debug"
		}),
	);
	assert_is_error(&resp, -32000);
}

/// `mcp/complete` before initialization should error.
#[test]
fn test_complete_not_initialized() {
	let mut engine = TestEngine::new();

	let resp = engine.request(
		"mcp/complete",
		json!({
			"server": "test",
			"reference": { "type": "ref/prompt", "name": "summarize" },
			"argument": { "name": "style", "value": "bri" }
		}),
	);
	assert_is_error(&resp, -32000);
}

/// Initialize with empty params (no client, no server) should succeed
/// but both fields should be false.
#[test]
fn test_initialize_empty_params() {
	let mut engine = TestEngine::new();

	let resp = engine.request("mcp/initialize", json!({}));
	assert_is_success(&resp);
	let result = &resp["result"];
	assert_eq!(result["clientInitialized"], false);
	assert_eq!(result["serverInitialized"], false);
}

/// Re-initialization should work (overwrite previous state).
#[test]
fn test_reinitialize() {
	let mut engine = TestEngine::new();

	// First init with server only.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"serverConfig": {
				"name": "server-v1",
				"version": "1.0.0"
			}
		}),
	);
	assert_is_success(&resp);
	assert_eq!(resp["result"]["serverInitialized"], true);
	assert_eq!(resp["result"]["clientInitialized"], false);

	// Re-init with client only.
	let resp = engine.request(
		"mcp/initialize",
		json!({
			"clientConfig": {
				"servers": []
			}
		}),
	);
	assert_is_success(&resp);
	assert_eq!(resp["result"]["clientInitialized"], true);
}
