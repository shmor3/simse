//! JSON-RPC client that communicates with the TS core subprocess.

use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;

use crate::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RpcMessage, parse_message};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum BridgeError {
	#[error("Failed to spawn bridge process: {0}")]
	SpawnFailed(String),
	#[error("Bridge process exited unexpectedly")]
	ProcessExited,
	#[error("JSON-RPC error {code}: {message}")]
	RpcError { code: i64, message: String },
	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Request timed out")]
	Timeout,
}

/// Bridge client configuration.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
	pub command: String,   // e.g. "bun"
	pub args: Vec<String>, // e.g. ["run", "bridge-server.ts"]
	pub data_dir: String,
	pub timeout_ms: u64,
}

impl Default for BridgeConfig {
	fn default() -> Self {
		Self {
			command: "bun".into(),
			args: vec!["run".into(), "bridge-server.ts".into()],
			data_dir: String::new(),
			timeout_ms: 60_000,
		}
	}
}

/// A running bridge subprocess with stdin/stdout pipes.
pub struct BridgeProcess {
	child: Child,
	stdin: ChildStdin,
	stdout: BufReader<ChildStdout>,
	timeout_ms: u64,
}

/// Spawn a bridge subprocess with piped stdin/stdout and null stderr.
pub async fn spawn_bridge(config: &BridgeConfig) -> Result<BridgeProcess, BridgeError> {
	let mut child = Command::new(&config.command)
		.args(&config.args)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()
		.map_err(|e| BridgeError::SpawnFailed(e.to_string()))?;

	let stdin = child
		.stdin
		.take()
		.ok_or_else(|| BridgeError::SpawnFailed("failed to capture stdin".into()))?;
	let stdout = child
		.stdout
		.take()
		.ok_or_else(|| BridgeError::SpawnFailed("failed to capture stdout".into()))?;

	Ok(BridgeProcess {
		child,
		stdin,
		stdout: BufReader::new(stdout),
		timeout_ms: config.timeout_ms,
	})
}

/// Check if the child process is still running (process ID is present and
/// has not exited).
pub fn is_healthy(bridge: &BridgeProcess) -> bool {
	bridge.child.id().is_some()
}

/// Write a line (with trailing newline) to the child's stdin and flush.
pub async fn send_line(bridge: &mut BridgeProcess, line: &str) -> Result<(), BridgeError> {
	bridge.stdin.write_all(line.as_bytes()).await?;
	bridge.stdin.write_all(b"\n").await?;
	bridge.stdin.flush().await?;
	Ok(())
}

/// Read one line from the child's stdout, with a timeout.
///
/// Returns `BridgeError::ProcessExited` on EOF (empty read) and
/// `BridgeError::Timeout` when the deadline is exceeded.
pub async fn read_line(
	bridge: &mut BridgeProcess,
	timeout: Duration,
) -> Result<String, BridgeError> {
	let mut buf = String::new();
	match tokio::time::timeout(timeout, bridge.stdout.read_line(&mut buf)).await {
		Ok(Ok(0)) => Err(BridgeError::ProcessExited),
		Ok(Ok(_)) => Ok(buf),
		Ok(Err(e)) => Err(BridgeError::Io(e)),
		Err(_) => Err(BridgeError::Timeout),
	}
}

/// Kill the child process. Ignores errors (best-effort cleanup).
pub async fn kill_bridge(mut bridge: BridgeProcess) {
	let _ = bridge.child.kill().await;
}

/// Send a JSON-RPC request and wait for the matching response.
///
/// Reads lines from stdout until a response with the matching ID arrives.
/// Notifications received while waiting are discarded.
/// Returns `BridgeError::Timeout` if no matching response within config timeout.
pub async fn request(
	bridge: &mut BridgeProcess,
	method: &str,
	params: Option<serde_json::Value>,
) -> Result<JsonRpcResponse, BridgeError> {
	let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
	let req = JsonRpcRequest::new(id, method, params);
	let json = serde_json::to_string(&req)?;

	send_line(bridge, &json).await?;

	let timeout = Duration::from_millis(bridge.timeout_ms);
	let deadline = tokio::time::Instant::now() + timeout;

	loop {
		let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
		if remaining.is_zero() {
			return Err(BridgeError::Timeout);
		}

		let line = read_line(bridge, remaining).await?;
		let msg = parse_message(line.trim())?;
		match msg {
			RpcMessage::Response(resp) if resp.id == Some(id) => return Ok(resp),
			_ => continue, // skip non-matching responses and notifications
		}
	}
}

/// Send a JSON-RPC request and collect notifications until the response arrives.
///
/// Notifications received while waiting are forwarded to the provided channel.
/// The final response is returned when a matching ID is found.
pub async fn request_streaming(
	bridge: &mut BridgeProcess,
	method: &str,
	params: Option<serde_json::Value>,
	notification_tx: mpsc::UnboundedSender<JsonRpcNotification>,
) -> Result<JsonRpcResponse, BridgeError> {
	let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
	let req = JsonRpcRequest::new(id, method, params);
	let json = serde_json::to_string(&req)?;

	send_line(bridge, &json).await?;

	let timeout = Duration::from_millis(bridge.timeout_ms);
	let deadline = tokio::time::Instant::now() + timeout;

	loop {
		let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
		if remaining.is_zero() {
			return Err(BridgeError::Timeout);
		}

		let line = read_line(bridge, remaining).await?;
		let msg = parse_message(line.trim())?;
		match msg {
			RpcMessage::Response(resp) if resp.id == Some(id) => return Ok(resp),
			RpcMessage::Response(_) => continue,
			RpcMessage::Notification(notif) => {
				let _ = notification_tx.send(notif);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn bridge_config_defaults() {
		let config = BridgeConfig::default();
		assert_eq!(config.command, "bun");
		assert_eq!(config.timeout_ms, 60_000);
	}

	#[tokio::test]
	async fn spawn_bridge_with_cat_echoes() {
		let config = BridgeConfig {
			command: "cat".into(),
			args: vec![],
			data_dir: String::new(),
			timeout_ms: 5_000,
		};
		let mut bridge = spawn_bridge(&config).await.unwrap();
		assert!(is_healthy(&bridge));

		send_line(&mut bridge, "hello").await.unwrap();
		let response = read_line(&mut bridge, Duration::from_secs(2))
			.await
			.unwrap();
		assert_eq!(response.trim(), "hello");

		kill_bridge(bridge).await;
	}

	#[tokio::test]
	async fn spawn_bridge_detects_exit() {
		let config = BridgeConfig {
			command: "true".into(),
			args: vec![],
			data_dir: String::new(),
			timeout_ms: 1_000,
		};
		let mut bridge = spawn_bridge(&config).await.unwrap();
		// Wait for the process to exit, then reap it so the ID is cleared.
		let _ = bridge.child.wait().await;
		assert!(!is_healthy(&bridge));
	}

	#[tokio::test]
	async fn read_line_timeout() {
		// `sleep 10` produces no output, so read should timeout
		let config = BridgeConfig {
			command: "sleep".into(),
			args: vec!["10".into()],
			data_dir: String::new(),
			timeout_ms: 5_000,
		};
		let mut bridge = spawn_bridge(&config).await.unwrap();
		let result = read_line(&mut bridge, Duration::from_millis(100)).await;
		assert!(matches!(result, Err(BridgeError::Timeout)));
		kill_bridge(bridge).await;
	}

	#[tokio::test]
	async fn request_initialize_roundtrip() {
		let config = BridgeConfig {
			command: "bun".into(),
			args: vec!["run".into(), "../simse-code/bridge-server.ts".into()],
			data_dir: String::new(),
			timeout_ms: 10_000,
		};
		let mut bridge = spawn_bridge(&config).await.unwrap();

		let resp = request(&mut bridge, "initialize", None).await.unwrap();
		let result = resp.result.unwrap();
		assert_eq!(result["protocolVersion"], 1);
		assert_eq!(result["name"], "simse-bridge");

		kill_bridge(bridge).await;
	}

	#[tokio::test]
	async fn request_unknown_method_returns_error() {
		let config = BridgeConfig {
			command: "bun".into(),
			args: vec!["run".into(), "../simse-code/bridge-server.ts".into()],
			data_dir: String::new(),
			timeout_ms: 10_000,
		};
		let mut bridge = spawn_bridge(&config).await.unwrap();

		let resp = request(&mut bridge, "nonexistent", None).await.unwrap();
		assert!(resp.error.is_some());
		assert_eq!(resp.error.unwrap().code, -32601);

		kill_bridge(bridge).await;
	}

	#[tokio::test]
	async fn request_multiple_sequential() {
		let config = BridgeConfig {
			command: "bun".into(),
			args: vec!["run".into(), "../simse-code/bridge-server.ts".into()],
			data_dir: String::new(),
			timeout_ms: 10_000,
		};
		let mut bridge = spawn_bridge(&config).await.unwrap();

		for _ in 0..3 {
			let resp = request(&mut bridge, "initialize", None).await.unwrap();
			assert!(resp.result.is_some());
		}

		kill_bridge(bridge).await;
	}

	#[tokio::test]
	async fn request_streaming_collects_notifications() {
		use crate::protocol::JsonRpcNotification;

		// Use the notification infrastructure (doesn't need bridge server)
		let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<JsonRpcNotification>();
		tx.send(JsonRpcNotification {
			jsonrpc: "2.0".into(),
			method: "stream.delta".into(),
			params: Some(serde_json::json!({"text": "hello"})),
		})
		.unwrap();
		drop(tx);

		let notif = rx.recv().await.unwrap();
		assert_eq!(notif.method, "stream.delta");
		assert_eq!(notif.params.unwrap()["text"].as_str().unwrap(), "hello");
	}
}
