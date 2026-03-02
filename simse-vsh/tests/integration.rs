// ---------------------------------------------------------------------------
// Integration tests for simse-vsh-engine
//
// Each test spawns the binary, communicates over JSON-RPC 2.0 / NDJSON stdio,
// and verifies responses.
// ---------------------------------------------------------------------------

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Manages a running `simse-vsh-engine` child process and provides methods for
/// sending JSON-RPC requests and reading responses.
struct VshProcess {
	child: Child,
	reader: BufReader<std::process::ChildStdout>,
	next_id: AtomicU64,
}

impl VshProcess {
	/// Spawn a new VSH engine process.
	fn spawn() -> Self {
		let bin = env!("CARGO_BIN_EXE_simse-vsh-engine");
		let mut child = Command::new(bin)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.spawn()
			.expect("failed to spawn simse-vsh-engine");

		let stdout = child.stdout.take().expect("no stdout");
		let reader = BufReader::new(stdout);

		Self {
			child,
			reader,
			next_id: AtomicU64::new(1),
		}
	}

	/// Send a JSON-RPC request and return the response `result` or `error` field.
	/// Skips any notification lines (lines without an `id` field).
	fn send(&mut self, method: &str, params: Value) -> RpcResponse {
		let id = self.next_id.fetch_add(1, Ordering::SeqCst);
		let request = json!({
			"jsonrpc": "2.0",
			"id": id,
			"method": method,
			"params": params,
		});

		let stdin = self.child.stdin.as_mut().expect("no stdin");
		let mut line = serde_json::to_string(&request).unwrap();
		line.push('\n');
		stdin.write_all(line.as_bytes()).unwrap();
		stdin.flush().unwrap();

		// Read lines until we find the response matching our id.
		loop {
			let mut buf = String::new();
			let bytes_read = self
				.reader
				.read_line(&mut buf)
				.expect("failed to read from stdout");
			if bytes_read == 0 {
				panic!(
					"unexpected EOF from simse-vsh-engine while waiting for response to id={}",
					id
				);
			}
			let buf = buf.trim();
			if buf.is_empty() {
				continue;
			}
			let parsed: Value = serde_json::from_str(buf)
				.unwrap_or_else(|e| panic!("invalid JSON from engine: {e}\nline: {buf}"));

			// Notifications have no `id` field -- skip them.
			if parsed.get("id").is_none() {
				continue;
			}

			let resp_id = parsed["id"].as_u64().expect("response id is not u64");
			assert_eq!(resp_id, id, "response id mismatch");

			if let Some(error) = parsed.get("error") {
				return RpcResponse::Error(error.clone());
			}
			return RpcResponse::Ok(parsed.get("result").cloned().unwrap_or(Value::Null));
		}
	}

	/// Convenience: send a request, expect success, return the `result` value.
	fn call(&mut self, method: &str, params: Value) -> Value {
		match self.send(method, params) {
			RpcResponse::Ok(v) => v,
			RpcResponse::Error(e) => panic!("expected success, got error: {e}"),
		}
	}

	/// Convenience: send a request, expect an error, return the `error` object.
	fn call_err(&mut self, method: &str, params: Value) -> Value {
		match self.send(method, params) {
			RpcResponse::Error(e) => e,
			RpcResponse::Ok(v) => panic!("expected error, got success: {v}"),
		}
	}

	/// Initialize with a temp directory as root.
	fn initialize(&mut self, root: &str) -> Value {
		self.call(
			"initialize",
			json!({
				"rootDirectory": root,
			}),
		)
	}

	/// Create a session with defaults, return the session info.
	fn create_session(&mut self) -> Value {
		self.call("session/create", json!({}))
	}
}

impl Drop for VshProcess {
	fn drop(&mut self) {
		// Close stdin to let the child exit gracefully.
		drop(self.child.stdin.take());
		let _ = self.child.wait();
	}
}

#[derive(Debug)]
enum RpcResponse {
	Ok(Value),
	Error(Value),
}

/// Create a temp directory and return its path as a string.
fn temp_dir() -> tempfile::TempDir {
	tempfile::tempdir().expect("failed to create temp dir")
}

// ---------------------------------------------------------------------------
// Test 1: initialize and create session
// ---------------------------------------------------------------------------

#[test]
fn initialize_and_create_session() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	assert!(session["id"].is_string());
	assert!(session["createdAt"].is_u64());
	assert!(session["commandCount"].as_u64().unwrap() == 0);
}

// ---------------------------------------------------------------------------
// Test 2: exec/run echo
// ---------------------------------------------------------------------------

#[test]
fn exec_run_echo() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	let result = proc.call(
		"exec/run",
		json!({
			"sessionId": session_id,
			"command": "echo hello",
		}),
	);

	assert_eq!(result["exitCode"], 0);
	assert!(
		result["stdout"].as_str().unwrap().contains("hello"),
		"stdout should contain 'hello', got: {}",
		result["stdout"]
	);
}

// ---------------------------------------------------------------------------
// Test 3: env/set + env/get verify session env state
// ---------------------------------------------------------------------------

#[test]
fn exec_run_with_env() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	// Set an env var
	proc.call(
		"env/set",
		json!({
			"sessionId": session_id,
			"key": "MY_VAR",
			"value": "hello_env",
		}),
	);

	// Verify the env var is stored in the session
	let result = proc.call(
		"env/get",
		json!({
			"sessionId": session_id,
			"key": "MY_VAR",
		}),
	);
	assert_eq!(result["value"], "hello_env");

	// Verify via session/get that env is present
	let session_info = proc.call(
		"session/get",
		json!({
			"sessionId": session_id,
		}),
	);
	assert_eq!(session_info["env"]["MY_VAR"], "hello_env");

	// Verify exec still works in this session
	let result = proc.call(
		"exec/run",
		json!({
			"sessionId": session_id,
			"command": "echo works",
		}),
	);
	assert_eq!(result["exitCode"], 0);
	assert!(result["stdout"].as_str().unwrap().contains("works"));
}

// ---------------------------------------------------------------------------
// Test 4: exec/runRaw stateless
// ---------------------------------------------------------------------------

#[test]
fn exec_run_raw_stateless() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let result = proc.call(
		"exec/runRaw",
		json!({
			"command": "echo raw_output",
		}),
	);

	assert_eq!(result["exitCode"], 0);
	assert!(
		result["stdout"].as_str().unwrap().contains("raw_output"),
		"stdout should contain 'raw_output', got: {}",
		result["stdout"]
	);
}

// ---------------------------------------------------------------------------
// Test 5: session cwd changes
// ---------------------------------------------------------------------------

#[test]
fn session_cwd_changes() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let subdir = dir.path().join("subdir");
	std::fs::create_dir(&subdir).unwrap();

	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	// Change cwd
	proc.call(
		"shell/setCwd",
		json!({
			"sessionId": session_id,
			"cwd": subdir.to_str().unwrap(),
		}),
	);

	// Verify cwd
	let result = proc.call(
		"shell/getCwd",
		json!({
			"sessionId": session_id,
		}),
	);

	let cwd = result["cwd"].as_str().unwrap();
	assert!(
		cwd.contains("subdir"),
		"cwd should contain 'subdir', got: {}",
		cwd
	);
}

// ---------------------------------------------------------------------------
// Test 6: alias resolution
// ---------------------------------------------------------------------------

#[test]
fn alias_resolution() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	// Set alias
	proc.call(
		"shell/setAlias",
		json!({
			"sessionId": session_id,
			"name": "greet",
			"command": "echo hello from alias",
		}),
	);

	// Verify aliases
	let result = proc.call(
		"shell/getAliases",
		json!({
			"sessionId": session_id,
		}),
	);
	assert_eq!(result["aliases"]["greet"], "echo hello from alias");

	// Run aliased command
	let result = proc.call(
		"exec/run",
		json!({
			"sessionId": session_id,
			"command": "greet",
		}),
	);

	assert_eq!(result["exitCode"], 0);
	assert!(
		result["stdout"]
			.as_str()
			.unwrap()
			.contains("hello from alias"),
		"stdout should contain alias expansion, got: {}",
		result["stdout"]
	);
}

// ---------------------------------------------------------------------------
// Test 7: command history tracking
// ---------------------------------------------------------------------------

#[test]
fn command_history_tracking() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	// Run a few commands
	proc.call(
		"exec/run",
		json!({
			"sessionId": session_id,
			"command": "echo one",
		}),
	);
	proc.call(
		"exec/run",
		json!({
			"sessionId": session_id,
			"command": "echo two",
		}),
	);

	// Get history
	let result = proc.call(
		"shell/history",
		json!({
			"sessionId": session_id,
		}),
	);

	let history = result["history"].as_array().unwrap();
	assert_eq!(history.len(), 2);
	assert_eq!(history[0]["command"], "echo one");
	assert_eq!(history[1]["command"], "echo two");
	assert_eq!(history[0]["exitCode"], 0);
}

// ---------------------------------------------------------------------------
// Test 8: session list and delete
// ---------------------------------------------------------------------------

#[test]
fn session_list_and_delete() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	// Create two sessions
	let s1 = proc.create_session();
	let s2 = proc.create_session();
	let s1_id = s1["id"].as_str().unwrap().to_string();
	let s2_id = s2["id"].as_str().unwrap().to_string();

	// List sessions
	let result = proc.call("session/list", json!({}));
	let sessions = result["sessions"].as_array().unwrap();
	assert_eq!(sessions.len(), 2);

	// Delete one
	let deleted = proc.call("session/delete", json!({"sessionId": s1_id}));
	assert_eq!(deleted["deleted"], true);

	// List again
	let result = proc.call("session/list", json!({}));
	let sessions = result["sessions"].as_array().unwrap();
	assert_eq!(sessions.len(), 1);
	assert_eq!(sessions[0]["id"], s2_id);
}

// ---------------------------------------------------------------------------
// Test 9: sandbox violation
// ---------------------------------------------------------------------------

#[test]
fn sandbox_violation() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	// Try to set cwd outside sandbox root
	let error = proc.call_err(
		"shell/setCwd",
		json!({
			"sessionId": session_id,
			"cwd": "/tmp/totally-outside",
		}),
	);

	assert!(error.get("code").is_some());
	let data = error.get("data").unwrap();
	let vsh_code = data["vshCode"].as_str().unwrap();
	assert_eq!(
		vsh_code, "VSH_SANDBOX_VIOLATION",
		"should be VSH_SANDBOX_VIOLATION error"
	);
}

// ---------------------------------------------------------------------------
// Test 10: command timeout
// ---------------------------------------------------------------------------

#[test]
fn command_timeout() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	// Run a long sleep with a very short timeout
	let error = proc.call_err(
		"exec/run",
		json!({
			"sessionId": session_id,
			"command": "sleep 10",
			"timeoutMs": 500,
		}),
	);

	assert!(error.get("code").is_some());
	let data = error.get("data").unwrap();
	let vsh_code = data["vshCode"].as_str().unwrap();
	assert_eq!(vsh_code, "VSH_TIMEOUT", "should be VSH_TIMEOUT error");
}

// ---------------------------------------------------------------------------
// Test 11: error before initialize
// ---------------------------------------------------------------------------

#[test]
fn error_before_initialize() {
	let mut proc = VshProcess::spawn();

	let error = proc.call_err(
		"exec/run",
		json!({
			"sessionId": "fake",
			"command": "echo hello",
		}),
	);

	assert!(error.get("code").is_some());
	let msg = error["message"].as_str().unwrap();
	assert!(
		msg.contains("Not initialized"),
		"error message should mention not initialized, got: {msg}"
	);
}

// ---------------------------------------------------------------------------
// Test 12: unknown method error
// ---------------------------------------------------------------------------

#[test]
fn unknown_method_error() {
	let mut proc = VshProcess::spawn();

	let error = proc.call_err("vsh/nonExistent", json!({}));

	let code = error["code"].as_i64().unwrap();
	assert_eq!(code, -32601, "should be METHOD_NOT_FOUND code");
}

// ---------------------------------------------------------------------------
// Test 13: env operations
// ---------------------------------------------------------------------------

#[test]
fn env_operations() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	// Set env var
	proc.call(
		"env/set",
		json!({
			"sessionId": session_id,
			"key": "TEST_KEY",
			"value": "test_value",
		}),
	);

	// Get env var
	let result = proc.call(
		"env/get",
		json!({
			"sessionId": session_id,
			"key": "TEST_KEY",
		}),
	);
	assert_eq!(result["value"], "test_value");

	// List env
	let result = proc.call(
		"env/list",
		json!({
			"sessionId": session_id,
		}),
	);
	assert_eq!(result["env"]["TEST_KEY"], "test_value");

	// Delete env var
	let result = proc.call(
		"env/delete",
		json!({
			"sessionId": session_id,
			"key": "TEST_KEY",
		}),
	);
	assert_eq!(result["deleted"], true);

	// Verify it's gone
	let result = proc.call(
		"env/get",
		json!({
			"sessionId": session_id,
			"key": "TEST_KEY",
		}),
	);
	assert!(result["value"].is_null());
}

// ---------------------------------------------------------------------------
// Test 14: shell/metrics
// ---------------------------------------------------------------------------

#[test]
fn shell_metrics() {
	let dir = temp_dir();
	let root = dir.path().to_str().unwrap();
	let mut proc = VshProcess::spawn();
	proc.initialize(root);

	let session = proc.create_session();
	let session_id = session["id"].as_str().unwrap();

	// Run a command
	proc.call(
		"exec/run",
		json!({
			"sessionId": session_id,
			"command": "echo metric_test",
		}),
	);

	// Check metrics
	let result = proc.call("shell/metrics", json!({}));
	assert_eq!(result["sessionCount"], 1);
	assert!(result["totalCommands"].as_u64().unwrap() >= 1);
}
