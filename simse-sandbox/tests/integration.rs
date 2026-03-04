// ---------------------------------------------------------------------------
// Integration tests for simse-sandbox-engine (local backend)
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

/// Manages a running `simse-sandbox-engine` child process and provides methods
/// for sending JSON-RPC requests and reading responses.
struct SandboxProcess {
	child: Child,
	reader: BufReader<std::process::ChildStdout>,
	next_id: AtomicU64,
}

#[derive(Debug)]
enum RpcResponse {
	Ok(Value),
	Error(Value),
}

impl SandboxProcess {
	/// Spawn a new sandbox engine process.
	fn spawn() -> Self {
		let bin = env!("CARGO_BIN_EXE_simse-sandbox-engine");
		let mut child = Command::new(bin)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.spawn()
			.expect("failed to spawn simse-sandbox-engine");

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
					"unexpected EOF from simse-sandbox-engine while waiting for response to id={}",
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

	/// Initialize with local backend and all engines (VFS + VSH + VNet).
	fn initialize_all(&mut self) -> Value {
		self.call(
			"sandbox/initialize",
			json!({
				"backend": { "type": "local" },
				"vfs": {
					"rootDirectory": ".",
					"maxHistory": 10,
				},
				"vsh": {
					"rootDirectory": ".",
					"shell": "sh",
					"defaultTimeoutMs": 30000,
					"maxOutputBytes": 50000,
				},
				"vnet": {
					"allowedHosts": ["*"],
					"allowedPorts": [{ "start": 1, "end": 65535 }],
					"allowedProtocols": ["http", "https", "ws", "tcp", "udp"],
				},
			}),
		)
	}
}

impl Drop for SandboxProcess {
	fn drop(&mut self) {
		// Close stdin to let the child exit gracefully.
		drop(self.child.stdin.take());
		let _ = self.child.wait();
	}
}

// ---------------------------------------------------------------------------
// Test 1: initialize local and health check
// ---------------------------------------------------------------------------

#[test]
fn test_initialize_local() {
	let mut proc = SandboxProcess::spawn();

	let result = proc.initialize_all();
	assert_eq!(result["ok"], true);

	let health = proc.call("sandbox/health", json!({}));
	assert_eq!(health["initialized"], true);
	assert_eq!(health["backendType"], "local");

	let engines = &health["engines"];
	assert_eq!(engines["vfs"], true);
	assert_eq!(engines["fsBackend"], true);
	assert_eq!(engines["vsh"], true);
	assert_eq!(engines["vnet"], true);
}

// ---------------------------------------------------------------------------
// Test 2: VFS write and read
// ---------------------------------------------------------------------------

#[test]
fn test_vfs_write_read() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_all();

	// Write a file
	proc.call(
		"sandbox/vfs/writeFile",
		json!({
			"path": "vfs://hello.txt",
			"content": "Hello, sandbox!",
		}),
	);

	// Read it back
	let result = proc.call(
		"sandbox/vfs/readFile",
		json!({
			"path": "vfs://hello.txt",
		}),
	);

	assert_eq!(result["text"], "Hello, sandbox!");
}

// ---------------------------------------------------------------------------
// Test 3: VFS mkdir and readdir
// ---------------------------------------------------------------------------

#[test]
fn test_vfs_mkdir_readdir() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_all();

	// Create directory
	proc.call(
		"sandbox/vfs/mkdir",
		json!({
			"path": "vfs://test-dir",
		}),
	);

	// Read root directory
	let result = proc.call(
		"sandbox/vfs/readdir",
		json!({
			"path": "vfs://",
		}),
	);

	let entries = result["entries"].as_array().unwrap();
	let names: Vec<&str> = entries
		.iter()
		.map(|e| e["name"].as_str().unwrap())
		.collect();
	assert!(
		names.contains(&"test-dir"),
		"expected 'test-dir' in entries, got: {:?}",
		names
	);
}

// ---------------------------------------------------------------------------
// Test 4: VSH session create and exec run
// ---------------------------------------------------------------------------

#[test]
fn test_vsh_session_exec() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_all();

	// Create session
	let session = proc.call("sandbox/session/create", json!({}));
	let session_id = session["id"].as_str().unwrap();

	// Run echo
	let result = proc.call(
		"sandbox/exec/run",
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
// Test 5: VSH exec runRaw (stateless)
// ---------------------------------------------------------------------------

#[test]
fn test_vsh_exec_raw() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_all();

	let result = proc.call(
		"sandbox/exec/runRaw",
		json!({
			"command": "echo test",
		}),
	);

	assert_eq!(result["exitCode"], 0);
	assert!(
		result["stdout"].as_str().unwrap().contains("test"),
		"stdout should contain 'test', got: {}",
		result["stdout"]
	);
}

// ---------------------------------------------------------------------------
// Test 6: VNet mock register and HTTP request
// ---------------------------------------------------------------------------

#[test]
fn test_vnet_mock_register_request() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_all();

	// Register mock
	let reg = proc.call(
		"sandbox/mock/register",
		json!({
			"urlPattern": "mock://api.example.com/data",
			"method": "GET",
			"response": {
				"status": 200,
				"body": "{\"ok\":true}",
				"headers": { "content-type": "application/json" },
				"bodyType": "text",
			},
		}),
	);
	assert!(reg["id"].is_string(), "mock register should return an id");

	// Make request
	let resp = proc.call(
		"sandbox/net/httpRequest",
		json!({
			"url": "mock://api.example.com/data",
			"method": "GET",
		}),
	);

	assert_eq!(resp["status"], 200);
	assert_eq!(resp["body"], "{\"ok\":true}");
}

// ---------------------------------------------------------------------------
// Test 7: VNet resolve localhost via DNS mock
// ---------------------------------------------------------------------------

#[test]
fn test_vnet_resolve_localhost() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_all();

	// Register a DNS mock for localhost
	proc.call(
		"sandbox/mock/register",
		json!({
			"urlPattern": "mock://dns/localhost",
			"method": "dns",
			"response": {
				"status": 200,
				"body": "[\"127.0.0.1\"]",
				"bodyType": "text",
			},
		}),
	);

	// Resolve
	let result = proc.call(
		"sandbox/net/resolve",
		json!({
			"hostname": "localhost",
		}),
	);

	let addresses = result["addresses"].as_array().unwrap();
	assert!(
		addresses.iter().any(|a| a.as_str() == Some("127.0.0.1")),
		"expected 127.0.0.1 in addresses, got: {:?}",
		addresses
	);
}

// ---------------------------------------------------------------------------
// Test 8: switch backend (local -> local)
// ---------------------------------------------------------------------------

#[test]
fn test_switch_backend() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_all();

	// Switch to local again
	let result = proc.call(
		"sandbox/switchBackend",
		json!({
			"backend": { "type": "local" },
		}),
	);
	assert_eq!(result["ok"], true);

	// Verify health still shows initialized
	let health = proc.call("sandbox/health", json!({}));
	assert_eq!(health["initialized"], true);
	assert_eq!(health["backendType"], "local");
}

// ---------------------------------------------------------------------------
// Test 9: dispose
// ---------------------------------------------------------------------------

#[test]
fn test_dispose() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_all();

	// Dispose
	let result = proc.call("sandbox/dispose", json!({}));
	assert_eq!(result["ok"], true);

	// Health should show not initialized
	let health = proc.call("sandbox/health", json!({}));
	assert_eq!(health["initialized"], false);
}

// ---------------------------------------------------------------------------
// Test 10: unknown method
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_method() {
	let mut proc = SandboxProcess::spawn();

	let error = proc.call_err("sandbox/nonExistent", json!({}));
	let code = error["code"].as_i64().unwrap();
	assert_eq!(code, -32601, "should be METHOD_NOT_FOUND code");
}
