use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;

use simse_vsh_engine::backend::ShellBackend;
use simse_vsh_engine::error::VshError;
use simse_vsh_engine::executor::ExecResult;

use super::channel::read_channel_output;
use super::pool::SshPool;

/// SSH-backed shell execution backend.
///
/// Routes all command execution through an SSH connection, building
/// a remote command string that sets the working directory, exports
/// environment variables, and runs the command through the specified
/// shell.
pub struct SshShellBackend {
	pool: Arc<SshPool>,
}

impl SshShellBackend {
	/// Create a new SSH shell backend wrapping the given pool.
	pub fn new(pool: Arc<SshPool>) -> Self {
		Self { pool }
	}
}

/// Escape a string for safe inclusion in a single-quoted shell argument.
///
/// Wraps the value in single quotes and escapes any embedded single
/// quotes using the `'\''` idiom (end quote, escaped quote, resume quote).
fn shell_escape(s: &str) -> String {
	let mut escaped = String::with_capacity(s.len() + 2);
	escaped.push('\'');
	for ch in s.chars() {
		if ch == '\'' {
			escaped.push_str("'\\''");
		} else {
			escaped.push(ch);
		}
	}
	escaped.push('\'');
	escaped
}

/// Build the full remote command string.
///
/// Produces a command that:
/// 1. Changes to the working directory
/// 2. Exports all environment variables
/// 3. Runs the command through the specified shell
fn build_remote_command(
	command: &str,
	cwd: &Path,
	env: &HashMap<String, String>,
	shell: &str,
) -> String {
	let mut parts = Vec::new();

	// cd to working directory
	let cwd_str = cwd.to_string_lossy();
	parts.push(format!("cd {}", shell_escape(&cwd_str)));

	// Export environment variables
	for (key, value) in env {
		parts.push(format!("export {}={}", key, shell_escape(value)));
	}

	// Run the command through the specified shell
	parts.push(format!("{} -c {}", shell, shell_escape(command)));

	parts.join(" && ")
}

/// Convert a `SandboxError` (from channel helpers) into a `VshError`.
fn sandbox_to_vsh(e: crate::error::SandboxError) -> VshError {
	match e {
		crate::error::SandboxError::Timeout(msg) => VshError::Timeout(msg),
		other => VshError::ExecutionFailed(other.to_string()),
	}
}

#[async_trait]
impl ShellBackend for SshShellBackend {
	async fn execute_command(
		&self,
		command: &str,
		cwd: &Path,
		env: &HashMap<String, String>,
		shell: &str,
		timeout_ms: u64,
		max_output_bytes: usize,
		stdin_input: Option<&str>,
	) -> Result<ExecResult, VshError> {
		let start = Instant::now();

		// 1. Open exec channel
		let mut channel = self
			.pool
			.get_exec_channel()
			.await
			.map_err(|e| VshError::ExecutionFailed(format!("open SSH channel: {e}")))?;

		// 2. Build remote command string
		let full_command = build_remote_command(command, cwd, env, shell);

		// 3. Execute the command on the remote host
		channel
			.exec(true, full_command.as_bytes())
			.await
			.map_err(|e| VshError::ExecutionFailed(format!("SSH exec: {e}")))?;

		// 4. If stdin_input is provided, write it to the channel then signal EOF
		if let Some(input) = stdin_input {
			channel
				.data(input.as_bytes())
				.await
				.map_err(|e| VshError::ExecutionFailed(format!("write stdin: {e}")))?;
			channel
				.eof()
				.await
				.map_err(|e| VshError::ExecutionFailed(format!("send eof: {e}")))?;
		}

		// 5. Read output
		let output = read_channel_output(&mut channel, timeout_ms, max_output_bytes)
			.await
			.map_err(sandbox_to_vsh)?;

		let duration_ms = start.elapsed().as_millis() as u64;

		// 6. Convert ExecOutput to ExecResult
		Ok(ExecResult {
			stdout: output.stdout,
			stderr: output.stderr,
			exit_code: output.exit_code.map(|c| c as i32).unwrap_or(-1),
			duration_ms,
		})
	}

	async fn execute_git(
		&self,
		args: &[String],
		cwd: &Path,
		env: &HashMap<String, String>,
		timeout_ms: u64,
		max_output_bytes: usize,
	) -> Result<ExecResult, VshError> {
		let git_command = format!("git {}", args.join(" "));
		self.execute_command(&git_command, cwd, env, "sh", timeout_ms, max_output_bytes, None)
			.await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_shell_escape_simple() {
		assert_eq!(shell_escape("hello"), "'hello'");
	}

	#[test]
	fn test_shell_escape_with_single_quotes() {
		assert_eq!(shell_escape("it's"), "'it'\\''s'");
	}

	#[test]
	fn test_shell_escape_empty() {
		assert_eq!(shell_escape(""), "''");
	}

	#[test]
	fn test_shell_escape_special_chars() {
		assert_eq!(shell_escape("a b$c"), "'a b$c'");
	}

	#[test]
	fn test_shell_escape_multiple_quotes() {
		assert_eq!(shell_escape("a'b'c"), "'a'\\''b'\\''c'");
	}

	#[test]
	fn test_build_remote_command_no_env() {
		let env = HashMap::new();
		let cmd = build_remote_command("ls -la", Path::new("/tmp"), &env, "bash");
		assert_eq!(cmd, "cd '/tmp' && bash -c 'ls -la'");
	}

	#[test]
	fn test_build_remote_command_with_env() {
		let mut env = HashMap::new();
		env.insert("FOO".to_string(), "bar".to_string());
		let cmd = build_remote_command("echo $FOO", Path::new("/home"), &env, "sh");
		assert!(cmd.starts_with("cd '/home'"));
		assert!(cmd.contains("export FOO='bar'"));
		assert!(cmd.ends_with("sh -c 'echo $FOO'"));
	}

	#[test]
	fn test_build_remote_command_quotes_in_command() {
		let env = HashMap::new();
		let cmd = build_remote_command("echo 'hello world'", Path::new("/"), &env, "sh");
		assert_eq!(cmd, "cd '/' && sh -c 'echo '\\''hello world'\\'''");
	}
}
