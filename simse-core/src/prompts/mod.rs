//! System prompt builder with ordered sections.
//!
//! Ports `src/ai/prompts/` (~434 lines).
//!
//! Provides `SystemPromptBuilder` (builder pattern), `EnvironmentInfo`,
//! `PromptMode` enum, `ProviderPromptResolver`, and instruction discovery.

mod builder;
mod environment;
mod provider;

pub use builder::{
	DiscoveredInstruction, PromptMode, SystemPromptBuildContext, SystemPromptBuilder,
};
pub use environment::{format_environment, EnvironmentInfo};
pub use provider::{provider_prompt, ProviderPromptConfig, ProviderPromptResolver};

use std::path::Path;

// ---------------------------------------------------------------------------
// Default patterns for instruction discovery
// ---------------------------------------------------------------------------

const DEFAULT_INSTRUCTION_PATTERNS: &[&str] =
	&["CLAUDE.md", "AGENTS.md", ".simse/instructions.md"];

// ---------------------------------------------------------------------------
// Instruction Discovery
// ---------------------------------------------------------------------------

/// Discover instruction files in a directory using default patterns.
///
/// Scans for `CLAUDE.md`, `AGENTS.md`, and `.simse/instructions.md`.
/// Missing files are silently skipped.
pub async fn discover_instructions(dir: &Path) -> Vec<DiscoveredInstruction> {
	discover_instructions_with(dir, DEFAULT_INSTRUCTION_PATTERNS).await
}

/// Discover instruction files in a directory using custom patterns.
///
/// Each pattern is resolved relative to `dir`. Missing files are silently skipped.
pub async fn discover_instructions_with(
	dir: &Path,
	patterns: &[&str],
) -> Vec<DiscoveredInstruction> {
	let mut results = Vec::new();

	for pattern in patterns {
		let full_path = dir.join(pattern);
		match tokio::fs::read_to_string(&full_path).await {
			Ok(content) => {
				let path_str = full_path.to_string_lossy().into_owned();
				results.push(DiscoveredInstruction {
					path: path_str,
					content,
				});
			}
			Err(_) => {
				// File not found or unreadable — skip silently.
			}
		}
	}

	results
}
