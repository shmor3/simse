//! VfsExec — command execution passthrough.
//!
//! Ports `src/ai/vfs/exec.ts` to Rust. Provides a pluggable `ExecBackend` trait
//! and a `VfsExec` wrapper that delegates execution to the backend.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::SimseError;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Options for command execution.
#[derive(Debug, Clone, Default)]
pub struct ExecOptions {
	/// Working directory for the command.
	pub cwd: Option<String>,
	/// Additional environment variables.
	pub env: Option<HashMap<String, String>>,
	/// Timeout in milliseconds.
	pub timeout_ms: Option<u64>,
	/// Standard input to feed to the command.
	pub stdin: Option<String>,
}

/// Result of a command execution.
#[derive(Debug, Clone)]
pub struct ExecResult {
	/// Standard output.
	pub stdout: String,
	/// Standard error.
	pub stderr: String,
	/// Process exit code.
	pub exit_code: i32,
	/// Files that were changed by the command (if detectable).
	pub files_changed: Vec<String>,
}

// ---------------------------------------------------------------------------
// ExecBackend trait
// ---------------------------------------------------------------------------

/// Backend trait for executing commands. Implementations may run commands
/// locally, in Docker containers, or in other sandboxed environments.
#[async_trait]
pub trait ExecBackend: Send + Sync {
	/// Run a command with the given arguments and options.
	async fn run(
		&self,
		command: &str,
		args: &[String],
		options: Option<&ExecOptions>,
	) -> Result<ExecResult, SimseError>;

	/// Clean up any resources held by the backend.
	async fn dispose(&self) -> Result<(), SimseError>;
}

// ---------------------------------------------------------------------------
// VfsExec
// ---------------------------------------------------------------------------

/// VfsExec wraps an `ExecBackend`, delegating command execution to it.
pub struct VfsExec {
	backend: Arc<dyn ExecBackend>,
}

impl VfsExec {
	/// Create a new VfsExec with the given backend.
	pub fn new(backend: Arc<dyn ExecBackend>) -> Self {
		Self { backend }
	}

	/// Run a command with the given arguments and options.
	pub async fn run(
		&self,
		command: &str,
		args: &[String],
		options: Option<&ExecOptions>,
	) -> Result<ExecResult, SimseError> {
		self.backend.run(command, args, options).await
	}

	/// Clean up the backend resources.
	pub async fn dispose(&self) -> Result<(), SimseError> {
		self.backend.dispose().await
	}
}
