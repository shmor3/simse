//! Environment context for system prompts.
//!
//! Provides `EnvironmentInfo` struct and `format_environment()` which
//! renders runtime context (platform, shell, cwd, date, git state)
//! as a formatted string suitable for system prompt injection.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// EnvironmentInfo
// ---------------------------------------------------------------------------

/// Runtime environment context for system prompt construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentInfo {
	/// Operating system platform (e.g., "linux", "darwin", "win32").
	pub platform: String,
	/// Shell being used (e.g., "bash", "zsh", "powershell").
	pub shell: String,
	/// Current working directory.
	pub cwd: String,
	/// Current date in ISO format (YYYY-MM-DD).
	pub date: String,
	/// Current git branch, if in a git repository.
	pub git_branch: Option<String>,
	/// Git status: "clean" or porcelain output.
	pub git_status: Option<String>,
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Format environment info as a markdown section for system prompts.
///
/// Produces a `# Environment` header followed by bullet points for each field.
/// Optional git fields are omitted when `None`.
/// Git status of "clean" is rendered inline; dirty status is rendered as a block.
pub fn format_environment(info: &EnvironmentInfo) -> String {
	let mut lines = vec![
		"# Environment".to_string(),
		format!("- Platform: {}", info.platform),
		format!("- Shell: {}", info.shell),
		format!("- Working directory: {}", info.cwd),
		format!("- Date: {}", info.date),
	];

	if let Some(ref branch) = info.git_branch {
		lines.push(format!("- Git branch: {branch}"));
	}

	if let Some(ref status) = info.git_status {
		if status == "clean" {
			lines.push("- Git status: clean".to_string());
		} else {
			lines.push(format!("- Git status:\n{status}"));
		}
	}

	lines.join("\n")
}
