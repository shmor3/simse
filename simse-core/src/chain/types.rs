//! Chain types -- Provider, step config, step result, and callbacks.
//!
//! Ports `src/ai/chain/types.ts` (~114 lines).

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::agent::{TokenUsage, ToolMetrics};
use crate::chain::template::PromptTemplate;

// ---------------------------------------------------------------------------
// Type aliases for complex callback/function types (clippy::type_complexity)
// ---------------------------------------------------------------------------

/// A shared, thread-safe output transform function.
pub type OutputTransformFn = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// A shared, thread-safe custom merge function for parallel sub-results.
pub type CustomMergeFn = Arc<dyn Fn(&[ParallelSubResult]) -> String + Send + Sync>;

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/// Which AI backend a chain step executes against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
	/// Route to an ACP agent server.
	Acp,
	/// Route to an MCP tool on a specific server.
	Mcp,
	/// Route to the library (vector store) for search.
	Memory,
}

impl fmt::Display for Provider {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Acp => write!(f, "acp"),
			Self::Mcp => write!(f, "mcp"),
			Self::Memory => write!(f, "memory"),
		}
	}
}

// ---------------------------------------------------------------------------
// ParallelSubResult
// ---------------------------------------------------------------------------

/// Result produced by a single sub-step within a parallel group.
#[derive(Debug, Clone)]
pub struct ParallelSubResult {
	pub sub_step_name: String,
	pub provider: Provider,
	pub model: String,
	pub input: String,
	pub output: String,
	pub duration_ms: u64,
	pub usage: Option<TokenUsage>,
	pub tool_metrics: Option<ToolMetrics>,
}

// ---------------------------------------------------------------------------
// MergeStrategy
// ---------------------------------------------------------------------------

/// How parallel sub-step results are merged back into the chain values.
///
/// - `Concat`: join all sub-step outputs with a separator (default `"\n\n"`)
/// - `Keyed`: store each sub-step output under `{stepName}.{subStepName}`,
///   merged output is also concatenated under the parent step name
/// - `Custom`: user-provided function that reduces sub-results into a string
#[derive(Clone, Default)]
pub enum MergeStrategy {
	#[default]
	Concat,
	Keyed,
	Custom(CustomMergeFn),
}

impl fmt::Debug for MergeStrategy {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Concat => write!(f, "MergeStrategy::Concat"),
			Self::Keyed => write!(f, "MergeStrategy::Keyed"),
			Self::Custom(_) => write!(f, "MergeStrategy::Custom(fn)"),
		}
	}
}

// ---------------------------------------------------------------------------
// ParallelSubStepConfig
// ---------------------------------------------------------------------------

/// Configuration for a single sub-step within a parallel group.
#[derive(Clone)]
pub struct ParallelSubStepConfig {
	pub name: String,
	pub template: PromptTemplate,
	pub provider: Option<Provider>,
	pub agent_id: Option<String>,
	pub server_name: Option<String>,
	pub system_prompt: Option<String>,
	pub output_transform: Option<OutputTransformFn>,
	pub mcp_server_name: Option<String>,
	pub mcp_tool_name: Option<String>,
	pub mcp_arguments: Option<HashMap<String, String>>,
}

impl fmt::Debug for ParallelSubStepConfig {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ParallelSubStepConfig")
			.field("name", &self.name)
			.field("provider", &self.provider)
			.field("agent_id", &self.agent_id)
			.field("server_name", &self.server_name)
			.finish_non_exhaustive()
	}
}

// ---------------------------------------------------------------------------
// ParallelConfig
// ---------------------------------------------------------------------------

/// Options controlling the parallel execution mode for a chain step.
#[derive(Clone, Debug)]
pub struct ParallelConfig {
	pub sub_steps: Vec<ParallelSubStepConfig>,
	pub merge_strategy: MergeStrategy,
	pub fail_tolerant: bool,
	pub concat_separator: String,
}

impl Default for ParallelConfig {
	fn default() -> Self {
		Self {
			sub_steps: Vec::new(),
			merge_strategy: MergeStrategy::default(),
			fail_tolerant: false,
			concat_separator: "\n\n".to_string(),
		}
	}
}

// ---------------------------------------------------------------------------
// ChainStepConfig
// ---------------------------------------------------------------------------

/// Configuration for a single chain step.
#[derive(Clone)]
pub struct ChainStepConfig {
	/// Unique name for this step.
	pub name: String,
	/// The prompt template to fill and send.
	pub template: PromptTemplate,
	/// Which AI provider to use for this step.
	pub provider: Option<Provider>,
	/// ACP agent ID override for this step.
	pub agent_id: Option<String>,
	/// ACP server name override for this step.
	pub server_name: Option<String>,
	/// System prompt prepended to the request (where supported).
	pub system_prompt: Option<String>,
	/// Transform the raw LLM output before passing to the next step.
	pub output_transform: Option<OutputTransformFn>,
	/// Map previous step outputs to this step's template variables.
	pub input_mapping: Option<HashMap<String, String>>,
	/// MCP: name of the connected MCP server to call.
	pub mcp_server_name: Option<String>,
	/// MCP: name of the tool to invoke on the MCP server.
	pub mcp_tool_name: Option<String>,
	/// MCP: mapping from tool argument names to chain value keys.
	pub mcp_arguments: Option<HashMap<String, String>>,
	/// Store this step's output to the library vector store.
	pub store_to_memory: bool,
	/// Metadata to attach when storing to library.
	pub memory_metadata: Option<HashMap<String, String>>,
	/// When set, this step runs sub-steps concurrently instead of calling
	/// a single provider.
	pub parallel: Option<ParallelConfig>,
}

impl fmt::Debug for ChainStepConfig {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ChainStepConfig")
			.field("name", &self.name)
			.field("provider", &self.provider)
			.field("agent_id", &self.agent_id)
			.field("server_name", &self.server_name)
			.field("parallel", &self.parallel)
			.finish_non_exhaustive()
	}
}

// ---------------------------------------------------------------------------
// StepResult
// ---------------------------------------------------------------------------

/// Result of executing a single chain step.
#[derive(Debug, Clone)]
pub struct StepResult {
	/// Name of the step.
	pub step_name: String,
	/// Provider that was used.
	pub provider: Provider,
	/// Model / agent that was used.
	pub model: String,
	/// The fully-resolved prompt that was sent.
	pub input: String,
	/// The raw or transformed output from the provider.
	pub output: String,
	/// Wall-clock time for this step in milliseconds.
	pub duration_ms: u64,
	/// Zero-based index of this step in the chain.
	pub step_index: usize,
	/// Token usage from ACP provider, if available.
	pub usage: Option<TokenUsage>,
	/// Tool call metrics from MCP provider.
	pub tool_metrics: Option<ToolMetrics>,
	/// Sub-step results when this step ran in parallel mode.
	pub sub_results: Option<Vec<ParallelSubResult>>,
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// Information passed to `on_step_start`.
#[derive(Debug, Clone)]
pub struct StepStartInfo {
	pub step_name: String,
	pub step_index: usize,
	pub total_steps: usize,
	pub provider: Provider,
	pub prompt: String,
}

/// Information passed to `on_step_error`.
#[derive(Debug)]
pub struct StepErrorInfo {
	pub step_name: String,
	pub step_index: usize,
	pub error: crate::error::SimseError,
}

/// Information passed to `on_chain_error`.
#[derive(Debug)]
pub struct ChainErrorInfo {
	pub error: crate::error::SimseError,
	pub completed_steps: Vec<StepResult>,
}

/// Boxed async callback future type.
pub type CallbackFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Callback hooks that fire during chain execution.
/// All callbacks are optional and async-safe.
#[derive(Default)]
pub struct ChainCallbacks {
	/// Fired before each step begins.
	pub on_step_start: Option<Arc<dyn Fn(StepStartInfo) -> CallbackFuture + Send + Sync>>,
	/// Fired after each step completes successfully.
	pub on_step_complete: Option<Arc<dyn Fn(StepResult) -> CallbackFuture + Send + Sync>>,
	/// Fired when a step fails (before the error is propagated).
	pub on_step_error: Option<Arc<dyn Fn(StepErrorInfo) -> CallbackFuture + Send + Sync>>,
	/// Fired when the entire chain completes successfully.
	pub on_chain_complete: Option<Arc<dyn Fn(Vec<StepResult>) -> CallbackFuture + Send + Sync>>,
	/// Fired when the chain fails (before the error is propagated).
	pub on_chain_error: Option<Arc<dyn Fn(ChainErrorInfo) -> CallbackFuture + Send + Sync>>,
}

impl fmt::Debug for ChainCallbacks {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ChainCallbacks")
			.field("on_step_start", &self.on_step_start.is_some())
			.field("on_step_complete", &self.on_step_complete.is_some())
			.field("on_step_error", &self.on_step_error.is_some())
			.field("on_chain_complete", &self.on_chain_complete.is_some())
			.field("on_chain_error", &self.on_chain_error.is_some())
			.finish()
	}
}

impl Clone for ChainCallbacks {
	fn clone(&self) -> Self {
		Self {
			on_step_start: self.on_step_start.clone(),
			on_step_complete: self.on_step_complete.clone(),
			on_step_error: self.on_step_error.clone(),
			on_chain_complete: self.on_chain_complete.clone(),
			on_chain_error: self.on_chain_error.clone(),
		}
	}
}
