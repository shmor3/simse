//! Subagent tool registration — spawn nested agentic loops and delegate
//! single-shot tasks to ACP agents.
//!
//! Ports `src/ai/tools/subagent-tools.ts` (~396 lines of TS) to Rust.
//!
//! Defines trait abstractions (`SubagentLoopRunner`, `DelegateRunner`) so that
//! the actual agentic loop and ACP client implementations can be plugged in
//! later. This follows the same pattern used in `builtin.rs` with
//! `LibraryStore` trait.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::error::SimseError;
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{ToolCategory, ToolDefinition, ToolHandler, ToolParameter};

// ---------------------------------------------------------------------------
// ID generation
// ---------------------------------------------------------------------------

/// Generate a unique subagent ID using UUID v4 (e.g. "sub_a1b2c3d4").
fn next_subagent_id() -> String {
	let short = &Uuid::new_v4().to_string()[..8];
	format!("sub_{}", short)
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The mode in which a subagent was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentMode {
	/// Full agentic loop with tool access.
	Spawn,
	/// Single-shot ACP delegation.
	Delegate,
}

/// Information about a subagent that was started.
#[derive(Debug, Clone)]
pub struct SubagentInfo {
	/// Unique identifier for this subagent run.
	pub id: String,
	/// Human-readable description of the subagent's task.
	pub description: String,
	/// Whether the subagent was spawned or delegated.
	pub mode: SubagentMode,
}

/// Result from a completed subagent run.
#[derive(Debug, Clone)]
pub struct SubagentResult {
	/// The final text output from the subagent.
	pub text: String,
	/// Number of conversation turns taken.
	pub turns: u32,
	/// Wall-clock duration in milliseconds.
	pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Callback type aliases
// ---------------------------------------------------------------------------

/// Callback fired when a subagent starts.
pub type SubagentStartFn = Box<dyn Fn(&SubagentInfo) + Send + Sync>;

/// Callback fired when a subagent completes.
pub type SubagentCompleteFn = Box<dyn Fn(&str, &SubagentResult) + Send + Sync>;

/// Callback fired when a subagent encounters an error.
pub type SubagentErrorFn = Box<dyn Fn(&str, &SimseError) + Send + Sync>;

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

/// Callbacks for subagent lifecycle events.
pub struct SubagentCallbacks {
	/// Fired when a subagent starts.
	pub on_start: Option<SubagentStartFn>,
	/// Fired when a subagent completes successfully.
	pub on_complete: Option<SubagentCompleteFn>,
	/// Fired when a subagent encounters an error.
	pub on_error: Option<SubagentErrorFn>,
}

impl std::fmt::Debug for SubagentCallbacks {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SubagentCallbacks")
			.field("on_start", &self.on_start.is_some())
			.field("on_complete", &self.on_complete.is_some())
			.field("on_error", &self.on_error.is_some())
			.finish()
	}
}

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// Trait for running a subagent's agentic loop.
///
/// The actual implementation will be provided later when the agentic loop
/// module is built. For now, tests can provide mock implementations.
#[async_trait]
pub trait SubagentLoopRunner: Send + Sync {
	/// Run a subagent loop with the given task prompt.
	///
	/// # Arguments
	/// * `task` - The task/prompt for the subagent
	/// * `max_turns` - Maximum conversation turns
	/// * `system_prompt` - Optional system prompt override
	/// * `depth` - Current recursion depth
	async fn run_subagent(
		&self,
		task: &str,
		max_turns: u32,
		system_prompt: Option<&str>,
		depth: u32,
	) -> Result<SubagentResult, SimseError>;
}

/// Trait for single-shot ACP delegation.
///
/// The actual implementation will use the ACP client to generate a
/// single response.
#[async_trait]
pub trait DelegateRunner: Send + Sync {
	/// Delegate a task for a single-shot response.
	///
	/// # Arguments
	/// * `task` - The task/prompt to delegate
	/// * `server_name` - Optional target ACP server name
	/// * `agent_id` - Optional target agent ID
	async fn delegate(
		&self,
		task: &str,
		server_name: Option<&str>,
		agent_id: Option<&str>,
	) -> Result<String, SimseError>;
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options for subagent tool registration.
pub struct SubagentToolsOptions {
	/// Runner for spawned subagent loops.
	pub loop_runner: Arc<dyn SubagentLoopRunner>,
	/// Runner for single-shot delegation.
	pub delegate_runner: Arc<dyn DelegateRunner>,
	/// Optional lifecycle callbacks.
	pub callbacks: Option<Arc<SubagentCallbacks>>,
	/// Default max turns for subagent_spawn (default: 10).
	pub default_max_turns: u32,
	/// Maximum recursion depth (default: 2). Tools are not registered
	/// if `depth >= max_depth`.
	pub max_depth: u32,
	/// Optional system prompt for subagent spawns.
	pub system_prompt: Option<String>,
}

impl std::fmt::Debug for SubagentToolsOptions {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SubagentToolsOptions")
			.field("default_max_turns", &self.default_max_turns)
			.field("max_depth", &self.max_depth)
			.field("system_prompt", &self.system_prompt)
			.field("callbacks", &self.callbacks.is_some())
			.finish()
	}
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn param(param_type: &str, description: &str, required: bool) -> ToolParameter {
	ToolParameter {
		param_type: param_type.to_string(),
		description: description.to_string(),
		required,
	}
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register `subagent_spawn` and `subagent_delegate` tools on the registry.
///
/// If `depth >= options.max_depth`, no tools are registered (prevents
/// unbounded recursion).
pub fn register_subagent_tools(
	registry: &mut ToolRegistry,
	options: &SubagentToolsOptions,
	depth: u32,
) {
	// Depth check: if at or beyond max depth, skip registration entirely
	if depth >= options.max_depth {
		return;
	}

	let default_max_turns = options.default_max_turns;
	let system_prompt = options.system_prompt.clone();

	// 1. subagent_spawn
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"task".to_string(),
			param("string", "The task/prompt for the subagent to work on", true),
		);
		parameters.insert(
			"description".to_string(),
			param(
				"string",
				"Short label describing what the subagent will do (e.g. \"Researching API endpoints\")",
				true,
			),
		);
		parameters.insert(
			"maxTurns".to_string(),
			param(
				"number",
				&format!(
					"Maximum turns the subagent can take (default: {})",
					default_max_turns
				),
				false,
			),
		);
		parameters.insert(
			"systemPrompt".to_string(),
			param(
				"string",
				"Optional system prompt override for the subagent",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "subagent_spawn".to_string(),
			description: "Spawn a subagent to handle a complex, multi-step task autonomously. The subagent runs in its own conversation context with access to all tools and returns its final result.".to_string(),
			parameters,
			category: ToolCategory::Subagent,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let loop_runner = Arc::clone(&options.loop_runner);
		let callbacks = options.callbacks.clone();
		let system_prompt = system_prompt.clone();

		let handler: ToolHandler = Arc::new(move |args: Value| {
			let loop_runner = Arc::clone(&loop_runner);
			let callbacks = callbacks.clone();
			let system_prompt = system_prompt.clone();

			Box::pin(async move {
				let id = next_subagent_id();
				let task = args
					.get("task")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let desc = args
					.get("description")
					.and_then(|v| v.as_str())
					.unwrap_or("Subagent task");
				let turns = args
					.get("maxTurns")
					.and_then(|v| v.as_u64())
					.map(|n| n as u32)
					.unwrap_or(default_max_turns);
				let child_system_prompt = args
					.get("systemPrompt")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string())
					.or(system_prompt);

				let info = SubagentInfo {
					id: id.clone(),
					description: desc.to_string(),
					mode: SubagentMode::Spawn,
				};

				// Fire on_start callback
				if let Some(ref cbs) = callbacks {
					if let Some(ref on_start) = cbs.on_start {
						on_start(&info);
					}
				}

				match loop_runner
					.run_subagent(
						task,
						turns,
						child_system_prompt.as_deref(),
						depth + 1,
					)
					.await
				{
					Ok(result) => {
						// Fire on_complete callback
						if let Some(ref cbs) = callbacks {
							if let Some(ref on_complete) = cbs.on_complete {
								on_complete(&id, &result);
							}
						}
						Ok(result.text)
					}
					Err(err) => {
						// Fire on_error callback
						if let Some(ref cbs) = callbacks {
							if let Some(ref on_error) = cbs.on_error {
								on_error(&id, &err);
							}
						}
						Err(err)
					}
				}
			})
		});

		registry.register(definition, handler);
	}

	// 2. subagent_delegate
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"task".to_string(),
			param("string", "The task/prompt to delegate", true),
		);
		parameters.insert(
			"description".to_string(),
			param(
				"string",
				"Short label describing the delegation (e.g. \"Summarizing document\")",
				true,
			),
		);
		parameters.insert(
			"serverName".to_string(),
			param("string", "Target ACP server name (optional)", false),
		);
		parameters.insert(
			"agentId".to_string(),
			param("string", "Target agent ID (optional)", false),
		);

		let definition = ToolDefinition {
			name: "subagent_delegate".to_string(),
			description: "Delegate a simple task to an ACP agent for a single-shot response. Use for tasks that do not require multi-step tool use.".to_string(),
			parameters,
			category: ToolCategory::Subagent,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let delegate_runner = Arc::clone(&options.delegate_runner);
		let callbacks = options.callbacks.clone();

		let handler: ToolHandler = Arc::new(move |args: Value| {
			let delegate_runner = Arc::clone(&delegate_runner);
			let callbacks = callbacks.clone();

			Box::pin(async move {
				let id = next_subagent_id();
				let task = args
					.get("task")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let desc = args
					.get("description")
					.and_then(|v| v.as_str())
					.unwrap_or("Delegated task");
				let server_name = args
					.get("serverName")
					.and_then(|v| v.as_str());
				let agent_id = args
					.get("agentId")
					.and_then(|v| v.as_str());

				let info = SubagentInfo {
					id: id.clone(),
					description: desc.to_string(),
					mode: SubagentMode::Delegate,
				};

				// Fire on_start callback
				if let Some(ref cbs) = callbacks {
					if let Some(ref on_start) = cbs.on_start {
						on_start(&info);
					}
				}

				match delegate_runner
					.delegate(task, server_name, agent_id)
					.await
				{
					Ok(text) => {
						let result = SubagentResult {
							text: text.clone(),
							turns: 1,
							duration_ms: 0,
						};

						// Fire on_complete callback
						if let Some(ref cbs) = callbacks {
							if let Some(ref on_complete) = cbs.on_complete {
								on_complete(&id, &result);
							}
						}

						Ok(text)
					}
					Err(err) => {
						// Fire on_error callback
						if let Some(ref cbs) = callbacks {
							if let Some(ref on_error) = cbs.on_error {
								on_error(&id, &err);
							}
						}
						Err(err)
					}
				}
			})
		});

		registry.register(definition, handler);
	}
}
