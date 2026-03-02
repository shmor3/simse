use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use crate::error::VshError;

/// Default shell for command execution.
pub fn default_shell() -> String {
	if cfg!(windows) {
		"bash".to_string()
	} else {
		"/bin/sh".to_string()
	}
}

/// Result of a command execution.
#[derive(Debug, Clone)]
pub struct ExecResult {
	pub stdout: String,
	pub stderr: String,
	pub exit_code: i32,
	pub duration_ms: u64,
}

/// Execute a shell command with the given parameters.
///
/// Spawns a real OS process using tokio, applies env/cwd, enforces timeout,
/// and truncates output if it exceeds max_output_bytes.
pub async fn execute_command(
	command: &str,
	cwd: &Path,
	env: &HashMap<String, String>,
	shell: &str,
	timeout_ms: u64,
	max_output_bytes: usize,
	stdin_input: Option<&str>,
) -> Result<ExecResult, VshError> {
	let start = Instant::now();
	let duration = std::time::Duration::from_millis(timeout_ms);

	let process_future = async {
		let mut cmd = tokio::process::Command::new(shell);
		cmd.arg("-c").arg(command).current_dir(cwd);

		// Apply environment variables (overlay on inherited env)
		for (key, value) in env {
			cmd.env(key, value);
		}

		// If stdin is provided, pipe it
		if stdin_input.is_some() {
			cmd.stdin(std::process::Stdio::piped());
		}

		cmd.stdout(std::process::Stdio::piped());
		cmd.stderr(std::process::Stdio::piped());

		let mut child = cmd.spawn().map_err(|e| {
			VshError::ExecutionFailed(format!("Failed to spawn shell: {}", e))
		})?;

		// Write stdin if provided
		if let Some(input) = stdin_input {
			use tokio::io::AsyncWriteExt;
			if let Some(mut stdin) = child.stdin.take() {
				stdin.write_all(input.as_bytes()).await.map_err(|e| {
					VshError::ExecutionFailed(format!("Failed to write stdin: {}", e))
				})?;
				// Drop stdin to signal EOF
			}
		}

		let output = child.wait_with_output().await.map_err(|e| {
			VshError::ExecutionFailed(format!("Failed to wait for process: {}", e))
		})?;

		let stdout = String::from_utf8_lossy(&output.stdout).to_string();
		let stderr = String::from_utf8_lossy(&output.stderr).to_string();
		let exit_code = output.status.code().unwrap_or(-1);

		Ok::<(String, String, i32), VshError>((stdout, stderr, exit_code))
	};

	match tokio::time::timeout(duration, process_future).await {
		Ok(result) => {
			let (mut stdout, mut stderr, exit_code) = result?;
			let elapsed = start.elapsed().as_millis() as u64;

			// Truncate stdout if needed
			if stdout.len() > max_output_bytes {
				let total = stdout.len();
				stdout.truncate(max_output_bytes);
				stdout.push_str(&format!(
					"\n[truncated: {} bytes total, showing first {}]",
					total, max_output_bytes
				));
			}

			// Truncate stderr if needed
			if stderr.len() > max_output_bytes {
				let total = stderr.len();
				stderr.truncate(max_output_bytes);
				stderr.push_str(&format!(
					"\n[truncated: {} bytes total, showing first {}]",
					total, max_output_bytes
				));
			}

			Ok(ExecResult {
				stdout,
				stderr,
				exit_code,
				duration_ms: elapsed,
			})
		}
		Err(_) => {
			let elapsed = start.elapsed().as_millis() as u64;
			Err(VshError::Timeout(format!(
				"Command timed out after {}ms",
				elapsed
			)))
		}
	}
}

/// Execute a git command with the given arguments.
///
/// Convenience wrapper that runs `git <args>` directly (no shell wrapping).
pub async fn execute_git(
	args: &[String],
	cwd: &Path,
	env: &HashMap<String, String>,
	timeout_ms: u64,
) -> Result<ExecResult, VshError> {
	let start = Instant::now();
	let duration = std::time::Duration::from_millis(timeout_ms);

	let process_future = async {
		let mut cmd = tokio::process::Command::new("git");
		cmd.args(args).current_dir(cwd);

		// Apply environment variables
		for (key, value) in env {
			cmd.env(key, value);
		}

		let output = cmd.output().await.map_err(|e| {
			VshError::ExecutionFailed(format!("Failed to spawn git: {}", e))
		})?;

		let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
		let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
		let exit_code = output.status.code().unwrap_or(-1);

		Ok::<(String, String, i32), VshError>((stdout, stderr, exit_code))
	};

	match tokio::time::timeout(duration, process_future).await {
		Ok(result) => {
			let (stdout, stderr, exit_code) = result?;
			let elapsed = start.elapsed().as_millis() as u64;

			Ok(ExecResult {
				stdout,
				stderr,
				exit_code,
				duration_ms: elapsed,
			})
		}
		Err(_) => {
			let elapsed = start.elapsed().as_millis() as u64;
			Err(VshError::Timeout(format!(
				"Git command timed out after {}ms",
				elapsed
			)))
		}
	}
}
