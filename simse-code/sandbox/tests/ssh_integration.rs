// ---------------------------------------------------------------------------
// SSH integration tests for simse-sandbox-engine (feature-gated)
//
// These tests require a running SSH server and the following env vars:
//   SSH_TEST_HOST     — hostname or IP of the SSH server
//   SSH_TEST_PORT     — port number (e.g. 22)
//   SSH_TEST_USER     — SSH username
//   SSH_TEST_KEY      — path to the private key file
//
// Run with: cargo test --features ssh-test
// ---------------------------------------------------------------------------

#![cfg(feature = "ssh-test")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helper (same pattern as integration.rs)
// ---------------------------------------------------------------------------

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

	fn call(&mut self, method: &str, params: Value) -> Value {
		match self.send(method, params) {
			RpcResponse::Ok(v) => v,
			RpcResponse::Error(e) => panic!("expected success, got error: {e}"),
		}
	}

	/// Build the SSH backend JSON from environment variables.
	fn ssh_backend() -> Value {
		let host = std::env::var("SSH_TEST_HOST").expect("SSH_TEST_HOST env var required");
		let port: u16 = std::env::var("SSH_TEST_PORT")
			.expect("SSH_TEST_PORT env var required")
			.parse()
			.expect("SSH_TEST_PORT must be a valid u16");
		let username = std::env::var("SSH_TEST_USER").expect("SSH_TEST_USER env var required");
		let key_path = std::env::var("SSH_TEST_KEY").expect("SSH_TEST_KEY env var required");

		json!({
			"type": "ssh",
			"ssh": {
				"host": host,
				"port": port,
				"username": username,
				"auth": {
					"type": "key",
					"privateKeyPath": key_path,
				}
			}
		})
	}

	/// Initialize with SSH backend and all engines.
	fn initialize_ssh_all(&mut self) -> Value {
		self.call(
			"sandbox/initialize",
			json!({
				"backend": Self::ssh_backend(),
				"vfs": {
					"rootDirectory": "/tmp",
					"maxHistory": 10,
				},
				"vsh": {
					"rootDirectory": "/tmp",
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
		drop(self.child.stdin.take());
		let _ = self.child.wait();
	}
}

// ---------------------------------------------------------------------------
// Test 1: Initialize sandbox with SSH backend, verify health
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_initialize() {
	let mut proc = SandboxProcess::spawn();

	let result = proc.initialize_ssh_all();
	assert_eq!(result["ok"], true);

	let health = proc.call("sandbox/health", json!({}));
	assert_eq!(health["initialized"], true);
	assert_eq!(health["backendType"], "ssh");
	assert_eq!(health["sshHealthy"], true);

	let engines = &health["engines"];
	assert_eq!(engines["vfs"], true);
	assert_eq!(engines["fsBackend"], true);
	assert_eq!(engines["vsh"], true);
	assert_eq!(engines["vnet"], true);
}

// ---------------------------------------------------------------------------
// Test 2: SSH + VSH — create session, exec echo, verify stdout
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_vsh_exec() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_ssh_all();

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
// Test 3: SSH + VFS — write a file, read it back, verify content
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_vfs_write_read() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_ssh_all();

	let unique_name = format!(
		"sandbox-test-{}",
		std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_millis()
	);
	let path = format!("file:///tmp/{unique_name}.txt");
	let content = "Hello from SSH sandbox test!";

	// Write file
	proc.call(
		"sandbox/vfs/writeFile",
		json!({
			"path": path,
			"content": content,
		}),
	);

	// Read file back
	let result = proc.call(
		"sandbox/vfs/readFile",
		json!({
			"path": path,
		}),
	);

	assert_eq!(
		result["text"].as_str().unwrap(),
		content,
		"file content mismatch"
	);
}

// ---------------------------------------------------------------------------
// Test 4: SSH + VNet — resolve "localhost", verify addresses returned
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_net_resolve() {
	let mut proc = SandboxProcess::spawn();
	proc.initialize_ssh_all();

	// Register a DNS mock for localhost (VNet uses mock-based resolution)
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

	let result = proc.call(
		"sandbox/net/resolve",
		json!({
			"hostname": "localhost",
		}),
	);

	let addresses = result["addresses"].as_array().unwrap();
	assert!(
		!addresses.is_empty(),
		"expected at least one address, got empty array"
	);
	assert!(
		addresses.iter().any(|a| a.as_str() == Some("127.0.0.1")),
		"expected 127.0.0.1 in addresses, got: {:?}",
		addresses
	);
}

// ---------------------------------------------------------------------------
// Test 5: Switch from local backend to SSH backend, verify health + ops
// ---------------------------------------------------------------------------

#[test]
fn test_switch_local_to_ssh() {
	let mut proc = SandboxProcess::spawn();

	// Start with local backend
	let result = proc.call(
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
	);
	assert_eq!(result["ok"], true);

	// Verify local backend
	let health = proc.call("sandbox/health", json!({}));
	assert_eq!(health["backendType"], "local");

	// Switch to SSH backend
	let switch_result = proc.call(
		"sandbox/switchBackend",
		json!({
			"backend": SandboxProcess::ssh_backend(),
		}),
	);
	assert_eq!(switch_result["ok"], true);

	// Verify SSH backend
	let health = proc.call("sandbox/health", json!({}));
	assert_eq!(health["initialized"], true);
	assert_eq!(health["backendType"], "ssh");
	assert_eq!(health["sshHealthy"], true);

	// Verify operations work after switch: create session and exec
	let session = proc.call("sandbox/session/create", json!({}));
	let session_id = session["id"].as_str().unwrap();

	let result = proc.call(
		"sandbox/exec/run",
		json!({
			"sessionId": session_id,
			"command": "echo switched",
		}),
	);

	assert_eq!(result["exitCode"], 0);
	assert!(
		result["stdout"].as_str().unwrap().contains("switched"),
		"stdout should contain 'switched', got: {}",
		result["stdout"]
	);
}
