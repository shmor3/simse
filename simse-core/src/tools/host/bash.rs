//! Bash Tool — shell command execution with timeout and output truncation.
//!
//! Ports `src/ai/tools/host/bash.ts` to Rust.
//! Uses `tokio::process::Command` with `tokio::time::timeout`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::error::{SimseError, ToolErrorCode};
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{
	ToolAnnotations, ToolCategory, ToolDefinition, ToolHandler, ToolParameter,
};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Configuration for bash tool registration.
pub struct BashToolOptions {
	/// The default working directory for commands.
	pub working_directory: PathBuf,
	/// Default timeout in milliseconds (default: 120,000).
	pub default_timeout_ms: Option<u64>,
	/// Maximum output bytes before truncation (default: 50,000).
	pub max_output_bytes: Option<usize>,
	/// Shell to use (default: "bash" on Windows, "/bin/sh" on Unix).
	pub shell: Option<String>,
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 50_000;

fn default_shell() -> String {
	if cfg!(windows) {
		"bash".to_string()
	} else {
		"/bin/sh".to_string()
	}
}

// ---------------------------------------------------------------------------
// Helper: build a ToolParameter
// ---------------------------------------------------------------------------

fn param(param_type: &str, description: &str, required: bool) -> ToolParameter {
	ToolParameter {
		param_type: param_type.to_string(),
		description: description.to_string(),
		required,
	}
}

// ---------------------------------------------------------------------------
// Public registration
// ---------------------------------------------------------------------------

/// Register the `bash` tool on the given registry.
pub fn register_bash_tool(registry: &mut ToolRegistry, options: BashToolOptions) {
	let default_timeout = options.default_timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
	let max_output = options
		.max_output_bytes
		.unwrap_or(DEFAULT_MAX_OUTPUT_BYTES);
	let shell = options.shell.unwrap_or_else(default_shell);
	let wd = Arc::new(options.working_directory);

	let mut parameters = HashMap::new();
	parameters.insert(
		"command".to_string(),
		param("string", "The shell command to execute", true),
	);
	parameters.insert(
		"timeout".to_string(),
		param(
			"number",
			&format!("Timeout in milliseconds (default: {})", default_timeout),
			false,
		),
	);
	parameters.insert(
		"cwd".to_string(),
		param(
			"string",
			&format!(
				"Working directory (default: {})",
				wd.display()
			),
			false,
		),
	);

	let definition = ToolDefinition {
		name: "bash".to_string(),
		description:
			"Execute a shell command. Returns stdout/stderr combined output with the exit code."
				.to_string(),
		parameters,
		category: ToolCategory::Execute,
		annotations: Some(ToolAnnotations {
			destructive: Some(true),
			..Default::default()
		}),
		timeout_ms: None,
		max_output_chars: None,
	};

	let handler: ToolHandler = Arc::new(move |args: Value| {
		let wd = Arc::clone(&wd);
		let shell = shell.clone();
		Box::pin(async move {
			let command = args
				.get("command")
				.and_then(|v| v.as_str())
				.unwrap_or("");

			if command.is_empty() {
				return Err(SimseError::tool(
					ToolErrorCode::ExecutionFailed,
					"Command is required",
				));
			}

			let timeout_ms = args
				.get("timeout")
				.and_then(|v| v.as_u64())
				.unwrap_or(default_timeout);
			// Use working directory — ignore user-supplied cwd to prevent sandbox escape
			let cwd = (*wd).clone();

			let duration = std::time::Duration::from_millis(timeout_ms);

			let process_future = async {
				let output = tokio::process::Command::new(&shell)
					.arg("-c")
					.arg(command)
					.current_dir(&cwd)
					.output()
					.await
					.map_err(|e| {
						SimseError::tool(
							ToolErrorCode::ExecutionFailed,
							format!("Failed to spawn shell: {}", e),
						)
					})?;

				let stdout = String::from_utf8_lossy(&output.stdout).to_string();
				let stderr = String::from_utf8_lossy(&output.stderr).to_string();
				let exit_code = output.status.code().unwrap_or(-1);

				Ok::<(String, String, i32), SimseError>((stdout, stderr, exit_code))
			};

			match tokio::time::timeout(duration, process_future).await {
				Ok(result) => {
					let (stdout, stderr, exit_code) = result?;
					let mut output = format!("{}{}", stdout, stderr);

					// Truncate if output exceeds limit
					if output.len() > max_output {
						let total = output.len();
						output.truncate(max_output);
						output.push_str(&format!(
							"\n[truncated: {} bytes total, showing first {}]",
							total, max_output
						));
					}

					if exit_code != 0 {
						Ok(format!("[exit code {}]\n{}", exit_code, output))
					} else {
						Ok(output)
					}
				}
				Err(_) => {
					// Timeout — process was dropped (killed automatically on drop)
					Ok(format!("[timeout after {}ms]", timeout_ms))
				}
			}
		})
	});

	registry.register_mut(definition, handler);
}
