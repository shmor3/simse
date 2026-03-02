use std::path::{Component, Path, PathBuf};

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
	///
	/// Uses `std::fs::canonicalize()` to resolve symlinks and `..` sequences,
	/// preventing sandbox escapes via path traversal.
	pub fn validate_cwd(&self, path: &Path) -> Result<PathBuf, VshError> {
		let joined = if path.is_absolute() {
			path.to_path_buf()
		} else {
			self.root_directory.join(path)
		};

		// Canonicalize the input path to resolve symlinks and ".." sequences.
		// If the path does not exist on disk, canonicalize will fail —
		// fall back to a lexical normalization that still strips "..".
		let canonical = std::fs::canonicalize(&joined).unwrap_or_else(|_| normalize_path(&joined));

		// Canonicalize the root directory for consistent comparison.
		let canonical_root = std::fs::canonicalize(&self.root_directory)
			.unwrap_or_else(|_| normalize_path(&self.root_directory));

		// Check if the path is within root_directory
		if canonical.starts_with(&canonical_root) {
			return Ok(canonical);
		}

		// Check allowed paths (also canonicalized)
		for allowed in &self.allowed_paths {
			let canonical_allowed =
				std::fs::canonicalize(allowed).unwrap_or_else(|_| normalize_path(allowed));
			if canonical.starts_with(&canonical_allowed) {
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

/// Lexically normalize a path by resolving `.` and `..` components without
/// touching the filesystem. Used as a fallback when `std::fs::canonicalize`
/// fails (e.g. path does not exist yet).
fn normalize_path(path: &Path) -> PathBuf {
	let mut out = PathBuf::new();
	for component in path.components() {
		match component {
			Component::ParentDir => {
				out.pop();
			}
			Component::CurDir => {}
			other => out.push(other),
		}
	}
	out
}
