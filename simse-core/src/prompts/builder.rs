//! System prompt builder with ordered sections.
//!
//! Assembles a system prompt from static and dynamic sections in a
//! cache-friendly order: identity -> mode -> tool_guidelines -> environment
//! -> instructions -> custom -> tool_defs -> memory.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::environment::{format_environment, EnvironmentInfo};

// ---------------------------------------------------------------------------
// PromptMode
// ---------------------------------------------------------------------------

/// Agent operating mode.
///
/// - `Build`: Default mode — gathers context, takes actions, verifies results.
/// - `Plan`: Research and planning only — no code modifications.
/// - `Explore`: Fast, read-only codebase exploration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptMode {
	Build,
	Plan,
	Explore,
}

impl PromptMode {
	/// Human-readable description of this mode.
	pub fn description(&self) -> &'static str {
		match self {
			Self::Build => "Default mode — gathers context, takes actions, verifies results.",
			Self::Plan => "Research and planning only — no code modifications.",
			Self::Explore => "Fast, read-only codebase exploration.",
		}
	}

	/// Default mode instructions for the system prompt.
	pub fn default_instructions(&self) -> &'static str {
		match self {
			Self::Build => DEFAULT_BUILD_INSTRUCTIONS,
			Self::Plan => DEFAULT_PLAN_INSTRUCTIONS,
			Self::Explore => DEFAULT_EXPLORE_INSTRUCTIONS,
		}
	}
}

impl fmt::Display for PromptMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Build => write!(f, "build"),
			Self::Plan => write!(f, "plan"),
			Self::Explore => write!(f, "explore"),
		}
	}
}

impl FromStr for PromptMode {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"build" => Ok(Self::Build),
			"plan" => Ok(Self::Plan),
			"explore" => Ok(Self::Explore),
			other => Err(format!("unknown prompt mode: {other}")),
		}
	}
}

// ---------------------------------------------------------------------------
// Default instructions
// ---------------------------------------------------------------------------

const DEFAULT_IDENTITY: &str = "You are a software development assistant.";

const DEFAULT_BUILD_INSTRUCTIONS: &str = "# Operating Mode: Build\n\n\
Follow a gather-action-verify workflow:\n\
1. **Gather**: Read relevant files, understand the codebase, and plan your approach before making changes.\n\
2. **Action**: Make precise, minimal changes. Prefer editing existing files over creating new ones.\n\
3. **Verify**: After changes, run relevant checks (typecheck, lint, tests) to confirm correctness.\n\n\
Guidelines:\n\
- Only modify code you have read and understood.\n\
- Keep changes focused — do not add features, refactoring, or improvements beyond what was requested.\n\
- Use parallel tool calls when operations are independent.\n\
- When uncertain, ask for clarification rather than guessing.";

const DEFAULT_PLAN_INSTRUCTIONS: &str = "# Operating Mode: Plan\n\n\
You are in planning mode. Research the codebase, analyze the task, and produce a structured implementation plan.\n\n\
Constraints:\n\
- Do NOT modify any files — read-only exploration only.\n\
- Do NOT execute commands that change state (no writes, installs, or deletions).\n\
- Output a clear, numbered implementation plan with file paths and descriptions of changes.";

const DEFAULT_EXPLORE_INSTRUCTIONS: &str = "# Operating Mode: Explore\n\n\
You are in exploration mode. Quickly find information in the codebase and return concise answers.\n\n\
Constraints:\n\
- Do NOT modify any files — read-only exploration only.\n\
- Be concise — answer the question directly without unnecessary elaboration.\n\
- Use search tools (grep, glob) efficiently to locate relevant code.";

const DEFAULT_TOOL_GUIDELINES: &str = "# Tool Usage Guidelines\n\n\
- When multiple tool calls are independent, execute them in parallel.\n\
- Use search tools (grep, glob) before reading files to find the right targets.\n\
- Always read a file before editing it.\n\
- Prefer editing existing files over creating new ones.\n\
- After writing code, verify it compiles/passes checks when possible.";

// ---------------------------------------------------------------------------
// DiscoveredInstruction
// ---------------------------------------------------------------------------

/// A discovered instruction file with its path and content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredInstruction {
	/// Absolute or relative path to the instruction file.
	pub path: String,
	/// Content of the instruction file.
	pub content: String,
}

// ---------------------------------------------------------------------------
// SystemPromptBuildContext
// ---------------------------------------------------------------------------

/// Context for building a system prompt from a single struct.
///
/// Mirrors the TS `SystemPromptBuildContext` interface.
#[derive(Debug, Clone, Default)]
pub struct SystemPromptBuildContext {
	/// Current operating mode. Defaults to `Build` if `None`.
	pub mode: Option<PromptMode>,
	/// Environment context (platform, shell, cwd, git).
	pub environment: Option<EnvironmentInfo>,
	/// Discovered instruction files.
	pub instructions: Option<Vec<DiscoveredInstruction>>,
	/// Dynamic memory context injected per turn.
	pub memory_context: Option<String>,
}

// ---------------------------------------------------------------------------
// SystemPromptBuilder
// ---------------------------------------------------------------------------

/// Builder for assembling a system prompt from ordered sections.
///
/// Sections are concatenated in this order (empty sections are skipped):
/// 1. identity
/// 2. mode instructions
/// 3. tool_guidelines
/// 4. environment
/// 5. instructions
/// 6. custom sections
/// 7. tool_defs
/// 8. memory
///
/// Sections are separated by double newlines.
#[derive(Debug, Clone)]
pub struct SystemPromptBuilder {
	identity: Option<String>,
	mode: Option<PromptMode>,
	mode_override: Option<String>,
	tool_guidelines: Option<String>,
	environment: Option<EnvironmentInfo>,
	instructions: Option<Vec<DiscoveredInstruction>>,
	custom_sections: Vec<String>,
	tool_defs: Option<String>,
	memory: Option<String>,
}

impl SystemPromptBuilder {
	/// Create a new empty builder.
	pub fn new() -> Self {
		Self {
			identity: None,
			mode: None,
			mode_override: None,
			tool_guidelines: None,
			environment: None,
			instructions: None,
			custom_sections: Vec::new(),
			tool_defs: None,
			memory: None,
		}
	}

	/// Construct a builder from a `SystemPromptBuildContext`.
	///
	/// Sets mode, environment, instructions, and memory from the context.
	/// Uses default identity. Call `use_default_tool_guidelines()` separately
	/// to add tool guidelines.
	pub fn from_context(ctx: &SystemPromptBuildContext) -> Self {
		let mut builder = Self::new();
		if let Some(mode) = ctx.mode {
			builder = builder.mode(mode);
		}
		if let Some(ref env) = ctx.environment {
			builder = builder.environment(env.clone());
		}
		if let Some(ref instructions) = ctx.instructions {
			builder = builder.instructions(instructions.clone());
		}
		if let Some(ref memory) = ctx.memory_context {
			builder = builder.memory(memory.clone());
		}
		builder
	}

	/// Set the identity section.
	pub fn identity(mut self, identity: impl Into<String>) -> Self {
		self.identity = Some(identity.into());
		self
	}

	/// Set the operating mode. Uses default instructions for that mode.
	pub fn mode(mut self, mode: PromptMode) -> Self {
		self.mode = Some(mode);
		self
	}

	/// Override the mode instructions with custom text.
	pub fn mode_instructions(mut self, instructions: impl Into<String>) -> Self {
		self.mode_override = Some(instructions.into());
		self
	}

	/// Set custom tool usage guidelines.
	pub fn tool_guidelines(mut self, guidelines: impl Into<String>) -> Self {
		self.tool_guidelines = Some(guidelines.into());
		self
	}

	/// Use the default tool guidelines section.
	pub fn use_default_tool_guidelines(mut self) -> Self {
		self.tool_guidelines = Some(DEFAULT_TOOL_GUIDELINES.to_string());
		self
	}

	/// Set the environment context.
	pub fn environment(mut self, env: EnvironmentInfo) -> Self {
		self.environment = Some(env);
		self
	}

	/// Set the discovered instruction files.
	pub fn instructions(mut self, instructions: Vec<DiscoveredInstruction>) -> Self {
		self.instructions = Some(instructions);
		self
	}

	/// Add a custom section. Multiple custom sections are appended in order.
	pub fn custom(mut self, section: impl Into<String>) -> Self {
		self.custom_sections.push(section.into());
		self
	}

	/// Set the tool definitions section.
	pub fn tool_defs(mut self, defs: impl Into<String>) -> Self {
		self.tool_defs = Some(defs.into());
		self
	}

	/// Set the memory context section.
	pub fn memory(mut self, memory: impl Into<String>) -> Self {
		self.memory = Some(memory.into());
		self
	}

	/// Build the system prompt by concatenating non-empty sections
	/// with double newline separators.
	pub fn build(&self) -> String {
		let mut sections: Vec<String> = Vec::new();

		// 1. Identity (static, cacheable)
		let identity = self
			.identity
			.clone()
			.unwrap_or_else(|| DEFAULT_IDENTITY.to_string());
		sections.push(identity);

		// 2. Mode instructions
		if let Some(ref mode) = self.mode {
			let mode_text = self
				.mode_override
				.clone()
				.unwrap_or_else(|| mode.default_instructions().to_string());
			sections.push(mode_text);
		}

		// 3. Tool usage guidelines
		if let Some(ref guidelines) = self.tool_guidelines {
			sections.push(guidelines.clone());
		}

		// 4. Environment context
		if let Some(ref env) = self.environment {
			sections.push(format_environment(env));
		}

		// 5. Instruction files
		if let Some(ref instructions) = self.instructions {
			if !instructions.is_empty() {
				let instr_section = instructions
					.iter()
					.map(|i| format!("## {}\n\n{}", i.path, i.content))
					.collect::<Vec<_>>()
					.join("\n\n");
				sections.push(format!("# Project Instructions\n\n{instr_section}"));
			}
		}

		// 6. Custom sections
		for section in &self.custom_sections {
			if !section.is_empty() {
				sections.push(section.clone());
			}
		}

		// 7. Tool definitions
		if let Some(ref tool_defs) = self.tool_defs {
			if !tool_defs.is_empty() {
				sections.push(tool_defs.clone());
			}
		}

		// 8. Memory context (most dynamic, last for cache efficiency)
		if let Some(ref memory) = self.memory {
			if !memory.is_empty() {
				sections.push(format!("# Memory Context\n\n{memory}"));
			}
		}

		sections.join("\n\n")
	}
}

impl Default for SystemPromptBuilder {
	fn default() -> Self {
		Self::new()
	}
}
