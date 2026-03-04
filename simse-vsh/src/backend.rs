use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;

use crate::error::VshError;
use crate::executor::ExecResult;

// -- ShellBackend trait -------------------------------------------------------

/// Async shell execution backend trait.
///
/// Abstracts raw command execution so it can be routed through either
/// local OS processes or a remote backend (e.g. SSH).
///
/// Session state (env, cwd, aliases, history) is managed by `VirtualShell`;
/// the backend only handles raw execution.
#[async_trait]
pub trait ShellBackend: Send + Sync {
	/// Execute a shell command with the given parameters.
	///
	/// Spawns a process using the specified shell, applies env/cwd,
	/// enforces timeout, and truncates output if it exceeds max_output_bytes.
	async fn execute_command(
		&self,
		command: &str,
		cwd: &Path,
		env: &HashMap<String, String>,
		shell: &str,
		timeout_ms: u64,
		max_output_bytes: usize,
		stdin_input: Option<&str>,
	) -> Result<ExecResult, VshError>;

	/// Execute a git command with the given arguments.
	///
	/// Runs `git <args>` directly (no shell wrapping), applying env/cwd,
	/// enforcing timeout, and truncating output.
	async fn execute_git(
		&self,
		args: &[String],
		cwd: &Path,
		env: &HashMap<String, String>,
		timeout_ms: u64,
		max_output_bytes: usize,
	) -> Result<ExecResult, VshError>;
}
