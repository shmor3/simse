use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;

use crate::backend::ShellBackend;
use crate::error::VshError;
use crate::executor::{self, ExecResult};

// -- LocalShellBackend --------------------------------------------------------

/// Shell backend that executes commands on the local OS.
///
/// Stateless unit struct that delegates to the functions in `executor`.
pub struct LocalShellBackend;

#[async_trait]
impl ShellBackend for LocalShellBackend {
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
		executor::execute_command(command, cwd, env, shell, timeout_ms, max_output_bytes, stdin_input)
			.await
	}

	async fn execute_git(
		&self,
		args: &[String],
		cwd: &Path,
		env: &HashMap<String, String>,
		timeout_ms: u64,
		max_output_bytes: usize,
	) -> Result<ExecResult, VshError> {
		executor::execute_git(args, cwd, env, timeout_ms, max_output_bytes).await
	}
}
