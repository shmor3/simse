//! Cross-ACP delegation tool registration.
//!
//! Registers a `delegate_{server_name}` tool for each non-primary ACP server
//! so that one ACP model can invoke another via single-shot generation.
//! Follows the same trait-based pattern as `builtin.rs` and `subagent.rs`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::SimseError;
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{ToolCategory, ToolDefinition, ToolHandler, ToolParameter};

// ---------------------------------------------------------------------------
// ID generation
// ---------------------------------------------------------------------------

/// Global atomic counter for generating unique delegation IDs.
static DELEGATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate the next unique delegation ID (e.g. "del_1", "del_2").
fn next_delegation_id() -> String {
	let n = DELEGATION_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
	format!("del_{}", n)
}

/// Reset the counter. Exposed for tests only.
pub fn reset_delegation_counter() {
	DELEGATION_COUNTER.store(0, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Information about a delegation that was started.
#[derive(Debug, Clone)]
pub struct DelegationInfo {
	/// Unique identifier for this delegation.
	pub id: String,
	/// The target ACP server name.
	pub server_name: String,
	/// The task/prompt that was delegated.
	pub task: String,
}

/// Result from a completed delegation.
#[derive(Debug, Clone)]
pub struct DelegationResult {
	/// The text response from the delegated server.
	pub text: String,
	/// The server that produced the response.
	pub server_name: String,
	/// Wall-clock duration in milliseconds.
	pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Callback type aliases
// ---------------------------------------------------------------------------

/// Callback fired when a delegation starts.
pub type DelegationStartFn = Box<dyn Fn(&DelegationInfo) + Send + Sync>;

/// Callback fired when a delegation completes.
pub type DelegationCompleteFn = Box<dyn Fn(&str, &DelegationResult) + Send + Sync>;

/// Callback fired when a delegation encounters an error.
pub type DelegationErrorFn = Box<dyn Fn(&str, &SimseError) + Send + Sync>;

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

/// Callbacks for delegation lifecycle events.
pub struct DelegationCallbacks {
	/// Fired when a delegation starts.
	pub on_start: Option<DelegationStartFn>,
	/// Fired when a delegation completes successfully.
	pub on_complete: Option<DelegationCompleteFn>,
	/// Fired when a delegation encounters an error.
	pub on_error: Option<DelegationErrorFn>,
}

impl std::fmt::Debug for DelegationCallbacks {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DelegationCallbacks")
			.field("on_start", &self.on_start.is_some())
			.field("on_complete", &self.on_complete.is_some())
			.field("on_error", &self.on_error.is_some())
			.finish()
	}
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Trait for server-specific delegation via ACP.
///
/// The actual implementation will use the ACP client to enumerate servers
/// and generate responses. Tests can provide mock implementations.
#[async_trait]
pub trait ServerDelegator: Send + Sync {
	/// Get all available ACP server names.
	fn server_names(&self) -> Vec<String>;

	/// Generate a single-shot response from a specific server.
	///
	/// # Arguments
	/// * `task` - The task/prompt to send
	/// * `server_name` - The target ACP server
	/// * `system_prompt` - Optional system prompt override
	async fn generate(
		&self,
		task: &str,
		server_name: &str,
		system_prompt: Option<&str>,
	) -> Result<String, SimseError>;
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options for delegation tool registration.
pub struct DelegationToolsOptions {
	/// The delegator that provides server names and generation.
	pub delegator: Arc<dyn ServerDelegator>,
	/// The primary server name (tools are NOT registered for this server).
	pub primary_server: Option<String>,
	/// Optional lifecycle callbacks.
	pub callbacks: Option<Arc<DelegationCallbacks>>,
}

impl std::fmt::Debug for DelegationToolsOptions {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DelegationToolsOptions")
			.field("primary_server", &self.primary_server)
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

/// Sanitize a server name into a valid tool name suffix.
/// Replaces any non-alphanumeric character with `_`.
fn sanitize_name(name: &str) -> String {
	name.chars()
		.map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
		.collect()
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register a `delegate_{server_name}` tool for each non-primary ACP server.
///
/// For each server returned by `delegator.server_names()` that is not the
/// primary server, a tool named `delegate_{sanitized_name}` is registered.
pub fn register_delegation_tools(
	registry: &mut ToolRegistry,
	options: &DelegationToolsOptions,
) {
	let server_names = options.delegator.server_names();

	for server_name in &server_names {
		// Skip the primary server
		if let Some(ref primary) = options.primary_server
			&& server_name == primary {
				continue;
			}

		let safe_name = sanitize_name(server_name);

		let mut parameters = HashMap::new();
		parameters.insert(
			"task".to_string(),
			param("string", "The task/prompt to send to the server", true),
		);
		parameters.insert(
			"systemPrompt".to_string(),
			param(
				"string",
				"Optional system prompt for the delegation",
				false,
			),
		);

		let definition = ToolDefinition {
			name: format!("delegate_{}", safe_name),
			description: format!(
				"Delegate a task to the \"{}\" ACP server for a single-shot response. Use this to get a response from a different AI model/server.",
				server_name
			),
			parameters,
			category: ToolCategory::Subagent,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let delegator = Arc::clone(&options.delegator);
		let callbacks = options.callbacks.clone();
		let server_name_owned = server_name.clone();

		let handler: ToolHandler = Arc::new(move |args: Value| {
			let delegator = Arc::clone(&delegator);
			let callbacks = callbacks.clone();
			let server_name = server_name_owned.clone();

			Box::pin(async move {
				let id = next_delegation_id();
				let task = args
					.get("task")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let system_prompt = args
					.get("systemPrompt")
					.and_then(|v| v.as_str());

				let info = DelegationInfo {
					id: id.clone(),
					server_name: server_name.clone(),
					task: task.to_string(),
				};

				// Fire on_start callback
				if let Some(ref cbs) = callbacks
					&& let Some(ref on_start) = cbs.on_start {
						on_start(&info);
					}

				match delegator
					.generate(task, &server_name, system_prompt)
					.await
				{
					Ok(text) => {
						let result = DelegationResult {
							text: text.clone(),
							server_name: server_name.clone(),
							duration_ms: 0,
						};

						// Fire on_complete callback
						if let Some(ref cbs) = callbacks
							&& let Some(ref on_complete) = cbs.on_complete {
								on_complete(&id, &result);
							}

						Ok(text)
					}
					Err(err) => {
						// Fire on_error callback
						if let Some(ref cbs) = callbacks
							&& let Some(ref on_error) = cbs.on_error {
								on_error(&id, &err);
							}
						Err(err)
					}
				}
			})
		});

		registry.register_mut(definition, handler);
	}
}
