//! Agentic loop — multi-turn orchestration with doom loop detection,
//! auto-compaction, and retry.
//!
//! The loop drives the conversation between an ACP client, a tool executor,
//! and optional compaction providers. It detects doom loops (repeated identical
//! tool calls), supports two-stage compaction (prune then summarise), and
//! retries transient errors.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use crate::error::SimseError;
use crate::events::event_types;
use crate::events::EventBus;
use crate::tools::types::{ParsedResponse, ToolCallRequest, ToolCallResult};

// ---------------------------------------------------------------------------
// Retry config
// ---------------------------------------------------------------------------

/// Retry parameters for stream generation or tool execution.
#[derive(Debug, Clone)]
pub struct RetryConfig {
	pub max_attempts: usize,
	pub base_delay_ms: u64,
}

impl Default for RetryConfig {
	fn default() -> Self {
		Self {
			max_attempts: 2,
			base_delay_ms: 500,
		}
	}
}

// ---------------------------------------------------------------------------
// AgenticLoopOptions
// ---------------------------------------------------------------------------

/// Options for creating an agentic loop.
#[derive(Clone)]
pub struct AgenticLoopOptions {
	/// Maximum turns before the loop stops (default: 10).
	pub max_turns: usize,
	/// Whether to auto-compact the conversation when it grows too large.
	pub auto_compact: bool,
	/// Custom compaction prompt (overrides the default).
	pub compaction_prompt: Option<String>,
	/// Number of identical consecutive tool calls before doom loop fires
	/// (default: 3).
	pub max_identical_tool_calls: usize,
	/// When true, skip tool call parsing — the agent handles tools itself.
	pub agent_manages_tools: bool,
	/// Retry config for ACP stream generation.
	pub stream_retry: RetryConfig,
	/// Retry config for tool execution.
	pub tool_retry: RetryConfig,
	/// Optional system prompt prepended to every ACP call.
	pub system_prompt: Option<String>,
	/// Optional event bus for publishing lifecycle events.
	pub event_bus: Option<Arc<EventBus>>,
}

impl Default for AgenticLoopOptions {
	fn default() -> Self {
		Self {
			max_turns: 10,
			auto_compact: false,
			compaction_prompt: None,
			max_identical_tool_calls: 3,
			agent_manages_tools: false,
			stream_retry: RetryConfig {
				max_attempts: 2,
				base_delay_ms: 1000,
			},
			tool_retry: RetryConfig {
				max_attempts: 2,
				base_delay_ms: 500,
			},
			system_prompt: None,
			event_bus: None,
		}
	}
}

impl std::fmt::Debug for AgenticLoopOptions {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AgenticLoopOptions")
			.field("max_turns", &self.max_turns)
			.field("auto_compact", &self.auto_compact)
			.field("compaction_prompt", &self.compaction_prompt)
			.field("max_identical_tool_calls", &self.max_identical_tool_calls)
			.field("agent_manages_tools", &self.agent_manages_tools)
			.field("stream_retry", &self.stream_retry)
			.field("tool_retry", &self.tool_retry)
			.field("system_prompt", &self.system_prompt)
			.field("event_bus", &self.event_bus.is_some())
			.finish()
	}
}

// ---------------------------------------------------------------------------
// Token usage
// ---------------------------------------------------------------------------

/// Token usage tracking for a single generation or accumulated over the loop.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
	pub prompt_tokens: Option<u64>,
	pub completion_tokens: Option<u64>,
	pub total_tokens: Option<u64>,
}

impl TokenUsage {
	/// Merge another usage into this one, summing each field.
	pub fn accumulate(&mut self, other: &TokenUsage) {
		fn add(a: &mut Option<u64>, b: Option<u64>) {
			match (a.as_mut(), b) {
				(Some(existing), Some(val)) => *existing += val,
				(None, Some(val)) => *a = Some(val),
				_ => {}
			}
		}
		add(&mut self.prompt_tokens, other.prompt_tokens);
		add(&mut self.completion_tokens, other.completion_tokens);
		add(&mut self.total_tokens, other.total_tokens);
	}
}

// ---------------------------------------------------------------------------
// Turn types
// ---------------------------------------------------------------------------

/// Classifies a loop turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnType {
	Text,
	ToolUse,
}

/// A single turn in the agentic loop.
#[derive(Debug, Clone)]
pub struct LoopTurn {
	pub turn: usize,
	pub turn_type: TurnType,
	pub text: Option<String>,
	pub tool_calls: Vec<ToolCallRequest>,
	pub tool_results: Vec<ToolCallResult>,
	pub duration_ms: u64,
	pub usage: Option<TokenUsage>,
}

// ---------------------------------------------------------------------------
// AgenticLoopResult
// ---------------------------------------------------------------------------

/// Result of running the agentic loop.
#[derive(Debug, Clone)]
pub struct AgenticLoopResult {
	pub final_text: String,
	pub turns: Vec<LoopTurn>,
	pub total_turns: usize,
	pub hit_turn_limit: bool,
	pub aborted: bool,
	pub total_duration_ms: u64,
	pub total_usage: Option<TokenUsage>,
}

// ---------------------------------------------------------------------------
// Message types (conversation messages passed to the ACP client)
// ---------------------------------------------------------------------------

/// Role for a conversation message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
	User,
	Assistant,
	System,
}

/// A single message in the conversation fed to the ACP client.
#[derive(Debug, Clone)]
pub struct Message {
	pub role: MessageRole,
	pub content: String,
}

// ---------------------------------------------------------------------------
// GenerateResponse
// ---------------------------------------------------------------------------

/// Response returned by an ACP client generate call.
#[derive(Debug, Clone)]
pub struct GenerateResponse {
	pub text: String,
	pub usage: Option<TokenUsage>,
}

// ---------------------------------------------------------------------------
// Traits (dependency injection)
// ---------------------------------------------------------------------------

/// ACP client trait for generating responses.
#[async_trait]
pub trait AcpClient: Send + Sync {
	async fn generate(
		&self,
		messages: &[Message],
		system: Option<&str>,
	) -> Result<GenerateResponse, SimseError>;
}

/// Tool executor trait for parsing and executing tool calls.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
	fn parse_tool_calls(&self, response: &str) -> ParsedResponse;
	async fn execute(&self, call: &ToolCallRequest) -> ToolCallResult;
}

/// Context pruner for lightweight (no-LLM) compaction.
pub trait ContextPruner: Send + Sync {
	fn prune(&self, messages: &[Message]) -> Vec<Message>;
}

/// Text generation provider for LLM-based compaction (summarisation).
#[async_trait]
pub trait CompactionProvider: Send + Sync {
	async fn generate(&self, prompt: &str) -> Result<String, SimseError>;
}

// ---------------------------------------------------------------------------
// CancellationToken
// ---------------------------------------------------------------------------

/// Lightweight cooperative cancellation token.
#[derive(Clone)]
pub struct CancellationToken {
	cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
	pub fn new() -> Self {
		Self {
			cancelled: Arc::new(AtomicBool::new(false)),
		}
	}

	pub fn cancel(&self) {
		self.cancelled.store(true, Ordering::Release);
	}

	pub fn is_cancelled(&self) -> bool {
		self.cancelled.load(Ordering::Acquire)
	}
}

impl Default for CancellationToken {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Callback type aliases (clippy type_complexity fix)
// ---------------------------------------------------------------------------

type StreamStartFn = dyn Fn() + Send + Sync;
type StreamDeltaFn = dyn Fn(&str) + Send + Sync;
type ToolCallStartFn = dyn Fn(&ToolCallRequest) + Send + Sync;
type ToolCallEndFn = dyn Fn(&ToolCallResult) + Send + Sync;
type TurnCompleteFn = dyn Fn(&LoopTurn) + Send + Sync;
type UsageUpdateFn = dyn Fn(&TokenUsage) + Send + Sync;
type CompactionFn = dyn Fn(&str) + Send + Sync;
type PreCompactionFn = dyn Fn(&str) -> Option<String> + Send + Sync;
type ErrorFn = dyn Fn(&SimseError) + Send + Sync;
type DoomLoopFn = dyn Fn(&str, usize) + Send + Sync;

/// Callbacks for observing the agentic loop lifecycle.
#[derive(Default)]
pub struct LoopCallbacks {
	pub on_stream_start: Option<Box<StreamStartFn>>,
	pub on_stream_delta: Option<Box<StreamDeltaFn>>,
	pub on_tool_call_start: Option<Box<ToolCallStartFn>>,
	pub on_tool_call_end: Option<Box<ToolCallEndFn>>,
	pub on_turn_complete: Option<Box<TurnCompleteFn>>,
	pub on_usage_update: Option<Box<UsageUpdateFn>>,
	pub on_compaction: Option<Box<CompactionFn>>,
	pub on_pre_compaction: Option<Box<PreCompactionFn>>,
	pub on_error: Option<Box<ErrorFn>>,
	pub on_doom_loop: Option<Box<DoomLoopFn>>,
}

// ---------------------------------------------------------------------------
// Default compaction prompt
// ---------------------------------------------------------------------------

const DEFAULT_COMPACTION_PROMPT: &str = "Summarize this conversation for continued operation. Preserve:\n\
1. Goal: The original user request\n\
2. Progress: What's been accomplished\n\
3. Current State: Where we are now\n\
4. Key Decisions: Important choices and rationale\n\
5. Relevant Files: File paths referenced or modified\n\
6. Next Steps: What remains to be done\n\
\n\
Be concise but complete.";

// ---------------------------------------------------------------------------
// Transient error detection
// ---------------------------------------------------------------------------

/// Checks whether an error is transient and thus suitable for retry.
fn is_transient_error(err: &SimseError) -> bool {
	err.is_retriable()
}

/// Heuristic: check if a tool output string indicates a transient failure.
fn is_transient_tool_output(output: &str) -> bool {
	let lower = output.to_lowercase();
	lower.contains("timeout")
		|| lower.contains("unavailable")
		|| lower.contains("econnrefused")
		|| lower.contains("econnreset")
		|| lower.contains("etimedout")
		|| lower.contains("socket hang up")
		|| lower.contains("network")
		|| lower.contains("503")
		|| lower.contains("429")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Stream generation with retry on transient errors.
async fn stream_with_retry(
	acp_client: &dyn AcpClient,
	messages: &[Message],
	system: Option<&str>,
	config: &RetryConfig,
) -> Result<GenerateResponse, SimseError> {
	let max = config.max_attempts.max(1);
	let mut last_err: Option<SimseError> = None;

	for attempt in 0..max {
		match acp_client.generate(messages, system).await {
			Ok(resp) => return Ok(resp),
			Err(err) => {
				if attempt + 1 < max && is_transient_error(&err) {
					let delay = config
						.base_delay_ms
						.saturating_mul(1u64.checked_shl(attempt as u32).unwrap_or(u64::MAX));
					tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
					last_err = Some(err);
				} else {
					return Err(err);
				}
			}
		}
	}

	Err(last_err.unwrap_or_else(|| SimseError::other("stream_with_retry: no attempts made")))
}

/// Execute a single tool call with retry on transient output.
async fn execute_with_retry(
	executor: &dyn ToolExecutor,
	call: &ToolCallRequest,
	config: &RetryConfig,
) -> ToolCallResult {
	let max = config.max_attempts.max(1);

	let mut result = executor.execute(call).await;

	for attempt in 1..max {
		if result.is_error && is_transient_tool_output(&result.output) {
			let delay = config
				.base_delay_ms
				.saturating_mul(1u64.checked_shl(attempt as u32).unwrap_or(u64::MAX));
			tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
			result = executor.execute(call).await;
		} else {
			break;
		}
	}

	result
}

/// Publish an event if an event bus is available.
fn emit(bus: &Option<Arc<EventBus>>, event: &str, payload: serde_json::Value) {
	if let Some(bus) = bus {
		bus.publish(event, payload);
	}
}

/// Estimate character count of the message list.
fn estimate_chars(messages: &[Message]) -> usize {
	messages.iter().map(|m| m.content.len()).sum()
}

// ---------------------------------------------------------------------------
// run_agentic_loop
// ---------------------------------------------------------------------------

/// Run the agentic loop: repeatedly call the ACP client, parse tool calls,
/// execute tools, and feed results back until the model produces a final text
/// response or the turn limit is reached.
///
/// # Arguments
///
/// * `acp_client` - Generates text responses from messages.
/// * `tool_executor` - Parses and executes tool calls.
/// * `messages` - Mutable conversation history. Updated in place.
/// * `options` - Loop configuration.
/// * `callbacks` - Optional lifecycle callbacks.
/// * `cancellation_token` - Optional token for cooperative cancellation.
/// * `context_pruner` - Optional lightweight (no-LLM) message pruner.
/// * `compaction_provider` - Optional LLM-based compaction (summarisation).
#[allow(clippy::too_many_arguments)]
pub async fn run_agentic_loop(
	acp_client: &dyn AcpClient,
	tool_executor: &dyn ToolExecutor,
	messages: &mut Vec<Message>,
	options: AgenticLoopOptions,
	callbacks: Option<LoopCallbacks>,
	cancellation_token: Option<&CancellationToken>,
	context_pruner: Option<&dyn ContextPruner>,
	compaction_provider: Option<&dyn CompactionProvider>,
) -> Result<AgenticLoopResult, SimseError> {
	let loop_start = Instant::now();
	let callbacks = callbacks.unwrap_or_default();
	let mut turns: Vec<LoopTurn> = Vec::new();
	let mut total_usage = TokenUsage::default();

	// Doom loop tracking: (tool_name, serialised_args)
	let mut recent_calls: Vec<(String, String)> = Vec::new();

	emit(
		&options.event_bus,
		event_types::LOOP_START,
		json!({ "max_turns": options.max_turns }),
	);

	for turn_idx in 1..=options.max_turns {
		let turn_start = Instant::now();

		// 1. Check cancellation
		if let Some(token) = cancellation_token
			&& token.is_cancelled() {
				let result = AgenticLoopResult {
					final_text: turns
						.last()
						.and_then(|t| t.text.clone())
						.unwrap_or_default(),
					total_turns: turns.len(),
					hit_turn_limit: false,
					aborted: true,
					total_duration_ms: loop_start.elapsed().as_millis() as u64,
					total_usage: Some(total_usage),
					turns,
				};

				emit(
					&options.event_bus,
					event_types::LOOP_COMPLETE,
					json!({ "aborted": true, "total_turns": result.total_turns }),
				);

				return Ok(result);
			}

		// 2. Two-stage compaction (if auto_compact and conversation is long enough)
		if options.auto_compact && estimate_chars(messages) > 100_000 {
			// Stage 1: prune (no LLM)
			if let Some(pruner) = context_pruner {
				let pruned = pruner.prune(messages);
				*messages = pruned;
			}

			// Stage 2: summarise (with LLM) — if still over threshold
			// Use 100_000 chars as the threshold (matching conversation.rs default)
			if let Some(provider) = compaction_provider
				&& estimate_chars(messages) > 100_000 {
					let compaction_prompt = options
						.compaction_prompt
						.as_deref()
						.unwrap_or(DEFAULT_COMPACTION_PROMPT);

					// Build the full prompt: conversation + compaction instruction
					let conversation_text: String = messages
						.iter()
						.map(|m| {
							let role = match m.role {
								MessageRole::User => "User",
								MessageRole::Assistant => "Assistant",
								MessageRole::System => "System",
							};
							format!("[{}]\n{}", role, m.content)
						})
						.collect::<Vec<_>>()
						.join("\n\n");

					let full_prompt =
						format!("{}\n\n---\n\n{}", conversation_text, compaction_prompt);

					// Allow pre-compaction hook to inject extra context
					let final_prompt =
						if let Some(ref hook) = callbacks.on_pre_compaction {
							if let Some(extra) = hook(&full_prompt) {
								format!("{}\n\n{}", full_prompt, extra)
							} else {
								full_prompt
							}
						} else {
							full_prompt
						};

					match provider.generate(&final_prompt).await {
						Ok(summary) => {
							// Replace messages with a single summary
							messages.clear();
							messages.push(Message {
								role: MessageRole::User,
								content: format!("[Conversation summary]\n{}", summary),
							});

							if let Some(ref cb) = callbacks.on_compaction {
								cb(&summary);
							}

							emit(
								&options.event_bus,
								event_types::LOOP_COMPACTION,
								json!({ "turn": turn_idx, "summary_len": summary.len() }),
							);
						}
						Err(err) => {
							// Log but do not fail the loop
							if let Some(ref cb) = callbacks.on_error {
								cb(&err);
							}
							tracing::warn!("compaction failed: {err}");
						}
					}
				}
		}

		// 3. Generate response from ACP with retry
		emit(
			&options.event_bus,
			event_types::LOOP_TURN_START,
			json!({ "turn": turn_idx }),
		);

		// Fire stream start callback before each generation call
		if let Some(ref cb) = callbacks.on_stream_start {
			cb();
		}

		let response = match stream_with_retry(
			acp_client,
			messages,
			options.system_prompt.as_deref(),
			&options.stream_retry,
		)
		.await
		{
			Ok(resp) => resp,
			Err(err) => {
				if let Some(ref cb) = callbacks.on_error {
					cb(&err);
				}
				emit(
					&options.event_bus,
					event_types::LOOP_COMPLETE,
					json!({
						"turns": turns.len(),
						"error": true,
						"error_message": err.to_string()
					}),
				);
				return Err(err);
			}
		};

		// Fire stream delta callback
		if let Some(ref cb) = callbacks.on_stream_delta {
			cb(&response.text);
		}

		// Accumulate usage
		if let Some(ref usage) = response.usage {
			total_usage.accumulate(usage);
			if let Some(ref cb) = callbacks.on_usage_update {
				cb(&total_usage);
			}
		}

		// 4. Parse tool calls (skip if agent_manages_tools)
		let parsed = if options.agent_manages_tools {
			ParsedResponse {
				text: response.text.clone(),
				tool_calls: vec![],
			}
		} else {
			tool_executor.parse_tool_calls(&response.text)
		};

		// 5. If no tool calls -> final text, return
		if parsed.tool_calls.is_empty() {
			let turn_duration = turn_start.elapsed().as_millis() as u64;
			let turn = LoopTurn {
				turn: turn_idx,
				turn_type: TurnType::Text,
				text: Some(parsed.text.clone()),
				tool_calls: vec![],
				tool_results: vec![],
				duration_ms: turn_duration,
				usage: response.usage,
			};

			if let Some(ref cb) = callbacks.on_turn_complete {
				cb(&turn);
			}

			emit(
				&options.event_bus,
				event_types::LOOP_TURN_END,
				json!({ "turn": turn_idx, "type": "text" }),
			);

			turns.push(turn);

			// Add assistant response to messages
			messages.push(Message {
				role: MessageRole::Assistant,
				content: parsed.text.clone(),
			});

			let result = AgenticLoopResult {
				final_text: parsed.text,
				total_turns: turns.len(),
				hit_turn_limit: false,
				aborted: false,
				total_duration_ms: loop_start.elapsed().as_millis() as u64,
				total_usage: Some(total_usage),
				turns,
			};

			emit(
				&options.event_bus,
				event_types::LOOP_COMPLETE,
				json!({
					"total_turns": result.total_turns,
					"hit_turn_limit": false,
					"aborted": false
				}),
			);

			return Ok(result);
		}

		// 6. Execute tool calls with retry + doom loop detection
		let mut tool_results: Vec<ToolCallResult> = Vec::new();

		// Add assistant message once before executing tool calls
		messages.push(Message {
			role: MessageRole::Assistant,
			content: response.text.clone(),
		});

		for call in &parsed.tool_calls {
			// Doom loop detection
			let args_hash = serde_json::to_string(&call.arguments).unwrap_or_default();
			recent_calls.push((call.name.clone(), args_hash));

			let max_ident = options.max_identical_tool_calls;
			if max_ident > 0 && recent_calls.len() >= max_ident {
				let tail = &recent_calls[recent_calls.len() - max_ident..];
				let all_same = tail.windows(2).all(|w| w[0] == w[1]);
				if all_same {
					// Fire doom loop callback
					if let Some(ref cb) = callbacks.on_doom_loop {
						cb(&call.name, max_ident);
					}

					emit(
						&options.event_bus,
						event_types::LOOP_DOOM_LOOP,
						json!({
							"tool_name": call.name,
							"count": max_ident,
							"turn": turn_idx
						}),
					);

					// Inject system warning into messages
					messages.push(Message {
						role: MessageRole::System,
						content: format!(
							"WARNING: You have called the tool '{}' {} times in a row with identical arguments. \
							 This appears to be a loop. Please try a different approach or respond with text.",
							call.name, max_ident
						),
					});
				}
			}

			// Keep recent_calls bounded
			if max_ident > 0 && recent_calls.len() > max_ident {
				recent_calls.drain(..recent_calls.len() - max_ident);
			}

			// Fire tool call start
			if let Some(ref cb) = callbacks.on_tool_call_start {
				cb(call);
			}

			emit(
				&options.event_bus,
				event_types::LOOP_TOOL_START,
				json!({ "tool": call.name, "id": call.id }),
			);

			// Execute with retry
			let tool_result =
				execute_with_retry(tool_executor, call, &options.tool_retry).await;

			// Fire tool call end
			if let Some(ref cb) = callbacks.on_tool_call_end {
				cb(&tool_result);
			}

			emit(
				&options.event_bus,
				event_types::LOOP_TOOL_END,
				json!({
					"tool": tool_result.name,
					"id": tool_result.id,
					"is_error": tool_result.is_error
				}),
			);

			// Add tool result to conversation
			messages.push(Message {
				role: MessageRole::User,
				content: format!(
					"[Tool Result: {}]\n{}",
					tool_result.name, tool_result.output
				),
			});

			tool_results.push(tool_result);
		}

		let turn_duration = turn_start.elapsed().as_millis() as u64;
		let turn = LoopTurn {
			turn: turn_idx,
			turn_type: TurnType::ToolUse,
			text: if parsed.text.is_empty() {
				None
			} else {
				Some(parsed.text.clone())
			},
			tool_calls: parsed.tool_calls,
			tool_results,
			duration_ms: turn_duration,
			usage: response.usage,
		};

		if let Some(ref cb) = callbacks.on_turn_complete {
			cb(&turn);
		}

		emit(
			&options.event_bus,
			event_types::LOOP_TURN_END,
			json!({ "turn": turn_idx, "type": "tool_use" }),
		);

		turns.push(turn);
	}

	// Hit the turn limit
	let result = AgenticLoopResult {
		final_text: turns
			.last()
			.and_then(|t| t.text.clone())
			.unwrap_or_default(),
		total_turns: turns.len(),
		hit_turn_limit: true,
		aborted: false,
		total_duration_ms: loop_start.elapsed().as_millis() as u64,
		total_usage: Some(total_usage),
		turns,
	};

	emit(
		&options.event_bus,
		event_types::LOOP_COMPLETE,
		json!({
			"total_turns": result.total_turns,
			"hit_turn_limit": true,
			"aborted": false
		}),
	);

	Ok(result)
}

// ---------------------------------------------------------------------------
// Tests (unit tests for internal helpers)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_cancellation_token_new_not_cancelled() {
		let token = CancellationToken::new();
		assert!(!token.is_cancelled());
	}

	#[test]
	fn test_cancellation_token_cancel() {
		let token = CancellationToken::new();
		token.cancel();
		assert!(token.is_cancelled());
	}

	#[test]
	fn test_cancellation_token_clone_shares_state() {
		let token = CancellationToken::new();
		let clone = token.clone();
		token.cancel();
		assert!(clone.is_cancelled());
	}

	#[test]
	fn test_cancellation_token_default() {
		let token = CancellationToken::default();
		assert!(!token.is_cancelled());
	}

	#[test]
	fn test_token_usage_accumulate() {
		let mut total = TokenUsage::default();
		let u1 = TokenUsage {
			prompt_tokens: Some(10),
			completion_tokens: Some(20),
			total_tokens: Some(30),
		};
		total.accumulate(&u1);
		assert_eq!(total.prompt_tokens, Some(10));
		assert_eq!(total.completion_tokens, Some(20));
		assert_eq!(total.total_tokens, Some(30));

		let u2 = TokenUsage {
			prompt_tokens: Some(5),
			completion_tokens: None,
			total_tokens: Some(5),
		};
		total.accumulate(&u2);
		assert_eq!(total.prompt_tokens, Some(15));
		assert_eq!(total.completion_tokens, Some(20));
		assert_eq!(total.total_tokens, Some(35));
	}

	#[test]
	fn test_token_usage_accumulate_none_plus_none() {
		let mut total = TokenUsage::default();
		let empty = TokenUsage::default();
		total.accumulate(&empty);
		assert!(total.prompt_tokens.is_none());
		assert!(total.completion_tokens.is_none());
		assert!(total.total_tokens.is_none());
	}

	#[test]
	fn test_is_transient_tool_output_timeout() {
		assert!(is_transient_tool_output("Error: request Timeout"));
		assert!(is_transient_tool_output("Service Unavailable"));
		assert!(is_transient_tool_output("ECONNREFUSED 127.0.0.1:3000"));
		assert!(is_transient_tool_output("socket hang up"));
		assert!(is_transient_tool_output("network error"));
		assert!(is_transient_tool_output("HTTP 503 error"));
		assert!(is_transient_tool_output("rate limited 429"));
		assert!(is_transient_tool_output("ECONNRESET by peer"));
		assert!(is_transient_tool_output("ETIMEDOUT after 30s"));
	}

	#[test]
	fn test_is_transient_tool_output_not_transient() {
		assert!(!is_transient_tool_output("file not found"));
		assert!(!is_transient_tool_output("permission denied"));
		assert!(!is_transient_tool_output("success"));
		assert!(!is_transient_tool_output(""));
	}

	#[test]
	fn test_is_transient_error_retriable() {
		use crate::error::ProviderErrorCode;
		let err = SimseError::provider(ProviderErrorCode::Timeout, "timed out", None);
		assert!(is_transient_error(&err));
	}

	#[test]
	fn test_is_transient_error_not_retriable() {
		use crate::error::ConfigErrorCode;
		let err = SimseError::config(ConfigErrorCode::InvalidField, "bad field");
		assert!(!is_transient_error(&err));
	}

	#[test]
	fn test_default_compaction_prompt_is_not_empty() {
		assert!(!DEFAULT_COMPACTION_PROMPT.is_empty());
		assert!(DEFAULT_COMPACTION_PROMPT.contains("Goal"));
		assert!(DEFAULT_COMPACTION_PROMPT.contains("Next Steps"));
	}

	#[test]
	fn test_retry_config_default() {
		let config = RetryConfig::default();
		assert_eq!(config.max_attempts, 2);
		assert_eq!(config.base_delay_ms, 500);
	}

	#[test]
	fn test_agentic_loop_options_default() {
		let opts = AgenticLoopOptions::default();
		assert_eq!(opts.max_turns, 10);
		assert!(!opts.auto_compact);
		assert!(opts.compaction_prompt.is_none());
		assert_eq!(opts.max_identical_tool_calls, 3);
		assert!(!opts.agent_manages_tools);
		assert_eq!(opts.stream_retry.max_attempts, 2);
		assert_eq!(opts.stream_retry.base_delay_ms, 1000);
		assert_eq!(opts.tool_retry.max_attempts, 2);
		assert_eq!(opts.tool_retry.base_delay_ms, 500);
		assert!(opts.system_prompt.is_none());
		assert!(opts.event_bus.is_none());
	}

	#[test]
	fn test_loop_callbacks_default() {
		let cb = LoopCallbacks::default();
		assert!(cb.on_stream_start.is_none());
		assert!(cb.on_stream_delta.is_none());
		assert!(cb.on_tool_call_start.is_none());
		assert!(cb.on_tool_call_end.is_none());
		assert!(cb.on_turn_complete.is_none());
		assert!(cb.on_usage_update.is_none());
		assert!(cb.on_compaction.is_none());
		assert!(cb.on_pre_compaction.is_none());
		assert!(cb.on_error.is_none());
		assert!(cb.on_doom_loop.is_none());
	}

	#[test]
	fn test_turn_type_eq() {
		assert_eq!(TurnType::Text, TurnType::Text);
		assert_eq!(TurnType::ToolUse, TurnType::ToolUse);
		assert_ne!(TurnType::Text, TurnType::ToolUse);
	}

	#[test]
	fn test_message_role_eq() {
		assert_eq!(MessageRole::User, MessageRole::User);
		assert_eq!(MessageRole::Assistant, MessageRole::Assistant);
		assert_eq!(MessageRole::System, MessageRole::System);
		assert_ne!(MessageRole::User, MessageRole::Assistant);
	}

	#[test]
	fn test_estimate_chars() {
		let msgs = vec![
			Message {
				role: MessageRole::User,
				content: "hello".to_string(),
			},
			Message {
				role: MessageRole::Assistant,
				content: "world!".to_string(),
			},
		];
		assert_eq!(estimate_chars(&msgs), 11);
	}

	#[test]
	fn test_estimate_chars_empty() {
		let msgs: Vec<Message> = vec![];
		assert_eq!(estimate_chars(&msgs), 0);
	}

	#[test]
	fn test_generate_response_clone() {
		let resp = GenerateResponse {
			text: "hello".to_string(),
			usage: Some(TokenUsage {
				prompt_tokens: Some(5),
				completion_tokens: Some(3),
				total_tokens: Some(8),
			}),
		};
		let cloned = resp.clone();
		assert_eq!(cloned.text, "hello");
		assert!(cloned.usage.is_some());
	}
}
