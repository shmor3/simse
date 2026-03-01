//! JSON-RPC client that communicates with the TS core subprocess.

use std::process::Stdio;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

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
}
