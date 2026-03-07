use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use tokio::io::AsyncReadExt;

use crate::error::SandboxError;

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

/// Apply output truncation to stdout and stderr if they exceed the limit.
fn truncate_output(output: &mut String, max_bytes: usize) {
	if output.len() > max_bytes {
		let total = output.len();
		output.truncate(max_bytes);
		output.push_str(&format!(
			"\n[truncated: {} bytes total, showing first {}]",
			total, max_bytes
		));
	}
}

/// Execute a shell command with the given parameters.
///
/// Spawns a real OS process using tokio, applies env/cwd, enforces timeout,
/// and truncates output if it exceeds max_output_bytes.
/// On timeout, the child process is explicitly killed to prevent orphans.
pub async fn execute_command(
	command: &str,
	cwd: &Path,
	env: &HashMap<String, String>,
	shell: &str,
	timeout_ms: u64,
	max_output_bytes: usize,
	stdin_input: Option<&str>,
) -> Result<ExecResult, SandboxError> {
	let start = Instant::now();
	let duration = std::time::Duration::from_millis(timeout_ms);

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
		SandboxError::VshExecutionFailed(format!("Failed to spawn shell: {}", e))
	})?;

	// Write stdin if provided, then drop to signal EOF
	if let Some(input) = stdin_input {
		use tokio::io::AsyncWriteExt;
		if let Some(mut stdin) = child.stdin.take() {
			stdin.write_all(input.as_bytes()).await.map_err(|e| {
				SandboxError::VshExecutionFailed(format!("Failed to write stdin: {}", e))
			})?;
		}
	}

	// Take stdout/stderr handles so we can read them while retaining &mut child
	let mut child_stdout = child.stdout.take();
	let mut child_stderr = child.stderr.take();

	let read_and_wait = async {
		let mut stdout_buf = Vec::new();
		let mut stderr_buf = Vec::new();

		if let Some(ref mut out) = child_stdout {
			out.read_to_end(&mut stdout_buf).await.ok();
		}
		if let Some(ref mut err) = child_stderr {
			err.read_to_end(&mut stderr_buf).await.ok();
		}

		let status = child.wait().await.map_err(|e| {
			SandboxError::VshExecutionFailed(format!("Failed to wait for process: {}", e))
		})?;

		Ok::<_, SandboxError>((stdout_buf, stderr_buf, status))
	};

	match tokio::time::timeout(duration, read_and_wait).await {
		Ok(Ok((stdout_buf, stderr_buf, status))) => {
			let mut stdout = String::from_utf8_lossy(&stdout_buf).to_string();
			let mut stderr = String::from_utf8_lossy(&stderr_buf).to_string();
			let exit_code = status.code().unwrap_or(-1);
			let elapsed = start.elapsed().as_millis() as u64;

			truncate_output(&mut stdout, max_output_bytes);
			truncate_output(&mut stderr, max_output_bytes);

			Ok(ExecResult {
				stdout,
				stderr,
				exit_code,
				duration_ms: elapsed,
			})
		}
		Ok(Err(e)) => Err(e),
		Err(_) => {
			// Timeout: explicitly kill the child to prevent orphan processes.
			// tokio::process::Child::Drop does NOT kill the process.
			let _ = child.kill().await;
			let elapsed = start.elapsed().as_millis() as u64;
			Err(SandboxError::VshTimeout(format!(
				"Command timed out after {}ms",
				elapsed
			)))
		}
	}
}

/// Execute a git command with the given arguments.
///
/// Convenience wrapper that runs `git <args>` directly (no shell wrapping).
/// On timeout, the child process is explicitly killed to prevent orphans.
pub async fn execute_git(
	args: &[String],
	cwd: &Path,
	env: &HashMap<String, String>,
	timeout_ms: u64,
	max_output_bytes: usize,
) -> Result<ExecResult, SandboxError> {
	let start = Instant::now();
	let duration = std::time::Duration::from_millis(timeout_ms);

	let mut cmd = tokio::process::Command::new("git");
	cmd.args(args).current_dir(cwd);

	cmd.stdout(std::process::Stdio::piped());
	cmd.stderr(std::process::Stdio::piped());

	// Apply environment variables
	for (key, value) in env {
		cmd.env(key, value);
	}

	let mut child = cmd.spawn().map_err(|e| {
		SandboxError::VshExecutionFailed(format!("Failed to spawn git: {}", e))
	})?;

	// Take stdout/stderr handles so we can read them while retaining &mut child
	let mut child_stdout = child.stdout.take();
	let mut child_stderr = child.stderr.take();

	let read_and_wait = async {
		let mut stdout_buf = Vec::new();
		let mut stderr_buf = Vec::new();

		if let Some(ref mut out) = child_stdout {
			out.read_to_end(&mut stdout_buf).await.ok();
		}
		if let Some(ref mut err) = child_stderr {
			err.read_to_end(&mut stderr_buf).await.ok();
		}

		let status = child.wait().await.map_err(|e| {
			SandboxError::VshExecutionFailed(format!("Failed to wait for git process: {}", e))
		})?;

		Ok::<_, SandboxError>((stdout_buf, stderr_buf, status))
	};

	match tokio::time::timeout(duration, read_and_wait).await {
		Ok(Ok((stdout_buf, stderr_buf, status))) => {
			let mut stdout = String::from_utf8_lossy(&stdout_buf).trim().to_string();
			let mut stderr = String::from_utf8_lossy(&stderr_buf).trim().to_string();
			let exit_code = status.code().unwrap_or(-1);
			let elapsed = start.elapsed().as_millis() as u64;

			truncate_output(&mut stdout, max_output_bytes);
			truncate_output(&mut stderr, max_output_bytes);

			Ok(ExecResult {
				stdout,
				stderr,
				exit_code,
				duration_ms: elapsed,
			})
		}
		Ok(Err(e)) => Err(e),
		Err(_) => {
			// Timeout: explicitly kill the child to prevent orphan processes.
			let _ = child.kill().await;
			let elapsed = start.elapsed().as_millis() as u64;
			Err(SandboxError::VshTimeout(format!(
				"Git command timed out after {}ms",
				elapsed
			)))
		}
	}
}
