use std::path::{Path, PathBuf};

use crate::error::VshError;

/// Sandbox configuration for shell execution.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
	pub root_directory: PathBuf,
	pub allowed_paths: Vec<PathBuf>,
	pub blocked_patterns: Vec<String>,
	pub max_sessions: usize,
	pub default_timeout_ms: u64,
	pub max_output_bytes: usize,
}

impl Default for SandboxConfig {
	fn default() -> Self {
		Self {
			root_directory: PathBuf::from("."),
			allowed_paths: Vec::new(),
			blocked_patterns: Vec::new(),
			max_sessions: 32,
			default_timeout_ms: 120_000,
			max_output_bytes: 50_000,
		}
	}
}

impl SandboxConfig {
	/// Validate that a path is within the sandbox root or allowed paths.
	pub fn validate_cwd(&self, path: &Path) -> Result<PathBuf, VshError> {
		let canonical = if path.is_absolute() {
			path.to_path_buf()
		} else {
			self.root_directory.join(path)
		};

		// Check if the path is within root_directory
		if canonical.starts_with(&self.root_directory) {
			return Ok(canonical);
		}

		// Check allowed paths
		for allowed in &self.allowed_paths {
			if canonical.starts_with(allowed) {
				return Ok(canonical);
			}
		}

		Err(VshError::SandboxViolation(format!(
			"Path '{}' is outside the sandbox root '{}'",
			path.display(),
			self.root_directory.display(),
		)))
	}

	/// Check if a command matches any blocked patterns.
	pub fn check_command(&self, command: &str) -> Result<(), VshError> {
		for pattern in &self.blocked_patterns {
			if command.contains(pattern) {
				return Err(VshError::SandboxViolation(format!(
					"Command matches blocked pattern: '{}'",
					pattern,
				)));
			}
		}
		Ok(())
	}
}
