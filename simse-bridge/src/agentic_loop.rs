//! Agentic loop — core AI interaction engine.
//!
//! Drives multi-turn conversations with an ACP backend, parsing tool calls,
//! executing them via the [`ToolRegistry`], and managing doom-loop detection.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use simse_ui_core::tools::{ToolCallRequest, ToolCallResult, format_tools_for_system_prompt};

use crate::acp_client::AcpClient;
use crate::acp_types::{GenerateOptions, StreamEvent};
use crate::tool_registry::ToolRegistry;

use simse_ui_core::state::conversation::ConversationBuffer;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options for the agentic loop.
#[derive(Debug, Clone)]
pub struct AgenticLoopOptions {
	/// Maximum number of turns before the loop exits. Default: 10.
	pub max_turns: usize,
	/// ACP server name to use for generation. Default: None.
	pub server_name: Option<String>,
	/// Agent ID for prompt metadata. Default: None.
	pub agent_id: Option<String>,
	/// Optional system prompt to inject. Default: None.
	pub system_prompt: Option<String>,
	/// If true, the agent manages its own tool calls and the loop does not
	/// inject tool definitions into the system prompt or parse tool calls.
	pub agent_manages_tools: bool,
	/// Existing ACP session ID to reuse. If None, a new session is created per turn.
	pub session_id: Option<String>,
	/// Raw user input for session-reuse mode. When both `session_id` and
	/// `user_input` are set, the loop sends only this text as the prompt
	/// instead of the full serialized conversation (since the ACP server
	/// already maintains conversation state in the session).
	pub user_input: Option<String>,
}

impl Default for AgenticLoopOptions {
	fn default() -> Self {
		Self {
			max_turns: 10,
			server_name: None,
			agent_id: None,
			system_prompt: None,
			agent_manages_tools: false,
			session_id: None,
			user_input: None,
		}
	}
}

// ---------------------------------------------------------------------------
// Turn types
// ---------------------------------------------------------------------------

/// Classification of a single turn in the agentic loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnType {
	/// The model responded with a final text answer (no tool calls).
	Response,
	/// The model issued one or more tool calls.
	ToolUse,
	/// An error occurred during the turn.
	Error,
}

/// A single turn in the agentic loop.
#[derive(Debug, Clone)]
pub struct LoopTurn {
	/// 1-based turn number.
	pub turn: usize,
	/// Classification of what happened during this turn.
	pub turn_type: TurnType,
	/// Text content from the model (if any).
	pub text: Option<String>,
	/// Tool call requests parsed from the model response.
	pub tool_calls: Vec<ToolCallRequest>,
	/// Results from executing the tool calls.
	pub tool_results: Vec<ToolCallResult>,
}

/// Final result of running the agentic loop.
#[derive(Debug, Clone)]
pub struct AgenticLoopResult {
	/// The last text response from the model.
	pub final_text: String,
	/// All turns that were executed.
	pub turns: Vec<LoopTurn>,
	/// Total number of turns executed.
	pub total_turns: usize,
	/// True if the loop exited because it reached `max_turns`.
	pub hit_turn_limit: bool,
	/// True if the loop was aborted via the abort signal.
	pub aborted: bool,
}

// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

/// Callback trait for agentic loop events.
///
/// All methods have default no-op implementations so consumers can override
/// only the events they care about.
#[allow(unused_variables)]
pub trait LoopCallbacks: Send + Sync {
	/// Called when the stream starts for a new turn.
	fn on_stream_start(&self) {}

	/// Called for each text delta received from the stream.
	fn on_stream_delta(&self, delta: &str) {}

	/// Called when a tool call is about to be executed.
	fn on_tool_call_start(&self, call: &ToolCallRequest) {}

	/// Called after a tool call completes.
	fn on_tool_call_end(&self, result: &ToolCallResult) {}

	/// Called when a full turn completes.
	fn on_turn_complete(&self, turn: &LoopTurn) {}

	/// Called when an error occurs.
	fn on_error(&self, error: &str) {}

	/// Permission check before executing a tool call.
	/// Return `true` to allow, `false` to deny.
	fn on_permission_check(&self, call: &ToolCallRequest) -> bool {
		true
	}

	/// Called when consecutive identical tool calls are detected (doom loop).
	fn on_doom_loop(&self, tool_key: &str, count: usize) {}

	/// Called when the conversation is auto-compacted.
	fn on_compaction(&self, summary: &str) {}

	/// Called with token usage for a turn.
	fn on_token_usage(&self, input: u64, output: u64) {}
}

/// No-op callback implementation for use in tests or when no callbacks are needed.
pub struct NoopCallbacks;
impl LoopCallbacks for NoopCallbacks {}

// ---------------------------------------------------------------------------
// Doom loop tracker
// ---------------------------------------------------------------------------

/// Default threshold for consecutive identical tool calls before a warning.
pub const DEFAULT_DOOM_LOOP_THRESHOLD: usize = 3;

/// Tracks consecutive identical tool calls to detect doom loops.
///
/// A "tool key" is computed as `name:hash(args)`. If the same key appears
/// `threshold` times in a row, the tracker fires a warning.
#[derive(Debug)]
pub struct DoomLoopTracker {
	/// Number of consecutive identical calls before firing.
	pub threshold: usize,
	/// Current consecutive count per tool key.
	counts: HashMap<String, usize>,
	/// The last tool key that was tracked.
	last_key: Option<String>,
}

impl DoomLoopTracker {
	/// Create a new tracker with the given threshold.
	pub fn new(threshold: usize) -> Self {
		Self {
			threshold,
			counts: HashMap::new(),
			last_key: None,
		}
	}

	/// Create a new tracker with the default threshold (3).
	pub fn with_default_threshold() -> Self {
		Self::new(DEFAULT_DOOM_LOOP_THRESHOLD)
	}

	/// Compute a stable key for a tool call: `name:hash(args)`.
	pub fn tool_key(call: &ToolCallRequest) -> String {
		use std::collections::hash_map::DefaultHasher;
		use std::hash::{Hash, Hasher};

		let mut hasher = DefaultHasher::new();
		call.arguments.to_string().hash(&mut hasher);
		format!("{}:{:x}", call.name, hasher.finish())
	}

	/// Track a tool call. Returns `Some((key, count))` if the doom loop
	/// threshold has been reached or exceeded, `None` otherwise.
	pub fn track(&mut self, call: &ToolCallRequest) -> Option<(String, usize)> {
		let key = Self::tool_key(call);

		let current = if self.last_key.as_ref() == Some(&key) {
			// Same key as last call — increment
			let count = self.counts.entry(key.clone()).or_insert(1);
			*count += 1;
			*count
		} else {
			// Different key — reset tracking
			self.counts.clear();
			self.counts.insert(key.clone(), 1);
			1
		};

		self.last_key = Some(key.clone());

		if current >= self.threshold {
			Some((key, current))
		} else {
			None
		}
	}

	/// Reset all tracking state.
	pub fn reset(&mut self) {
		self.counts.clear();
		self.last_key = None;
	}

	/// Get the current count for a given key (0 if not tracked).
	pub fn current_count(&self, key: &str) -> usize {
		self.counts.get(key).copied().unwrap_or(0)
	}
}

impl Default for DoomLoopTracker {
	fn default() -> Self {
		Self::with_default_threshold()
	}
}

// ---------------------------------------------------------------------------
// Core agentic loop
// ---------------------------------------------------------------------------

/// Run the agentic loop.
///
/// Drives a multi-turn conversation with an ACP backend. For each turn:
/// 1. Checks the abort signal
/// 2. Auto-compacts the conversation if it exceeds the threshold
/// 3. Sends the conversation to ACP and streams the response
/// 4. Parses tool calls (unless agent manages its own tools)
/// 5. If no tool calls, returns the final text response
/// 6. Detects doom loops (consecutive identical tool calls)
/// 7. Checks permissions and executes tool calls
/// 8. Adds results to the conversation and loops
///
/// The loop exits when:
/// - The model responds without tool calls (normal completion)
/// - The abort signal is set
/// - `max_turns` is reached
pub async fn run_agentic_loop(
	conversation: &mut ConversationBuffer,
	acp_client: &AcpClient,
	tool_registry: &ToolRegistry,
	options: &AgenticLoopOptions,
	callbacks: &dyn LoopCallbacks,
	abort_signal: Arc<AtomicBool>,
) -> AgenticLoopResult {
	let mut turns: Vec<LoopTurn> = Vec::new();
	let mut doom_tracker = DoomLoopTracker::with_default_threshold();
	let mut final_text = String::new();
	let mut aborted = false;

	// When reusing an ACP session, the server manages conversation state and
	// system prompts. Skip local system prompt injection in that case.
	let session_manages_state = options.session_id.is_some() && options.user_input.is_some();

	if !session_manages_state {
		// Build system prompt with tool definitions (unless agent manages tools)
		if !options.agent_manages_tools {
			let tool_defs = tool_registry.get_tool_definitions();
			let mut system_prompt_parts = Vec::new();

			if let Some(ref user_prompt) = options.system_prompt {
				system_prompt_parts.push(user_prompt.clone());
			}

			if !tool_defs.is_empty() {
				system_prompt_parts.push(format_tools_for_system_prompt(&tool_defs));
			}

			if !system_prompt_parts.is_empty() {
				conversation.set_system_prompt(&system_prompt_parts.join("\n\n"));
			}
		} else if let Some(ref user_prompt) = options.system_prompt {
			conversation.set_system_prompt(user_prompt);
		}
	}

	// Main loop
	for turn_num in 1..=options.max_turns {
		// (a) Check abort signal
		if abort_signal.load(Ordering::Relaxed) {
			aborted = true;
			break;
		}

		// (b) Auto-compact if needed and not the first turn
		if conversation.needs_compaction() && turn_num > 1 {
			let summary = "[Auto-compacted: conversation exceeded token budget]";
			conversation.compact(summary);
			callbacks.on_compaction(summary);
		}

		// (c) Serialize conversation and stream from ACP
		callbacks.on_stream_start();

		// When reusing an ACP session, the server already tracks conversation
		// history. Send only the new user input instead of the full history.
		let prompt_text = if session_manages_state {
			options.user_input.clone().unwrap_or_else(|| conversation.serialize())
		} else {
			conversation.serialize()
		};
		let gen_options = GenerateOptions {
			agent_id: options.agent_id.clone(),
			server_name: options.server_name.clone(),
			system_prompt: None, // Already set in conversation
			..Default::default()
		};

		// Reuse existing ACP session or create a new one
		let session_id = if let Some(ref sid) = options.session_id {
			sid.clone()
		} else {
			match acp_client.new_session().await {
				Ok(id) => id,
				Err(e) => {
					let error_msg = format!("Failed to create ACP session: {e}");
					callbacks.on_error(&error_msg);
					turns.push(LoopTurn {
						turn: turn_num,
						turn_type: TurnType::Error,
						text: Some(error_msg.clone()),
						tool_calls: Vec::new(),
						tool_results: Vec::new(),
					});
					final_text = error_msg;
					break;
				}
			}
		};

		let mut stream = match acp_client
			.generate_stream(&session_id, &prompt_text, gen_options)
			.await
		{
			Ok(rx) => rx,
			Err(e) => {
				let error_msg = format!("Stream error: {e}");
				callbacks.on_error(&error_msg);
				turns.push(LoopTurn {
					turn: turn_num,
					turn_type: TurnType::Error,
					text: Some(error_msg.clone()),
					tool_calls: Vec::new(),
					tool_results: Vec::new(),
				});
				final_text = error_msg;
				break;
			}
		};

		// Collect the full response from the stream
		let mut full_response = String::new();
		let mut turn_usage: Option<(u64, u64)> = None;

		while let Some(event) = stream.recv().await {
			match event {
				StreamEvent::Delta(text) => {
					callbacks.on_stream_delta(&text);
					full_response.push_str(&text);
				}
				StreamEvent::Complete(result) => {
					// Extract usage from the prompt response
					if let Some(usage) = &result.usage {
						turn_usage =
							Some((usage.prompt_tokens, usage.completion_tokens));
					}
				}
				StreamEvent::Usage(usage) => {
					turn_usage = Some((usage.prompt_tokens, usage.completion_tokens));
				}
				StreamEvent::Error(msg) => {
					callbacks.on_error(&msg);
				}
				StreamEvent::ToolCall { .. } | StreamEvent::ToolCallUpdate { .. } => {
					// These are ACP-level tool call events; we handle tool calls
					// via parsing the response text ourselves.
				}
			}
		}

		// Report token usage if available
		if let Some((input, output)) = turn_usage {
			callbacks.on_token_usage(input, output);
		}

		// (d) Parse tool calls from response (unless agent manages tools)
		let parsed = if options.agent_manages_tools {
			simse_ui_core::tools::parser::ParsedResponse {
				text: full_response.clone(),
				tool_calls: Vec::new(),
			}
		} else {
			simse_ui_core::tools::parser::parse_tool_calls(&full_response)
		};

		// (e) If no tool calls, this is a final response
		if parsed.tool_calls.is_empty() {
			let turn = LoopTurn {
				turn: turn_num,
				turn_type: TurnType::Response,
				text: Some(parsed.text.clone()),
				tool_calls: Vec::new(),
				tool_results: Vec::new(),
			};
			callbacks.on_turn_complete(&turn);
			turns.push(turn);
			final_text = parsed.text;
			break;
		}

		// (f) Doom loop detection
		for tc in &parsed.tool_calls {
			if let Some((key, count)) = doom_tracker.track(tc) {
				callbacks.on_doom_loop(&key, count);
			}
		}

		// (g) Execute tool calls: check permission, execute, fire callbacks
		let mut tool_results = Vec::new();

		for tc in &parsed.tool_calls {
			// Permission check
			if !callbacks.on_permission_check(tc) {
				tool_results.push(ToolCallResult {
					id: tc.id.clone(),
					name: tc.name.clone(),
					output: "Permission denied by user.".into(),
					is_error: true,
					duration_ms: None,
					diff: None,
				});
				continue;
			}

			callbacks.on_tool_call_start(tc);
			let result = tool_registry.execute(tc).await;
			callbacks.on_tool_call_end(&result);
			tool_results.push(result);
		}

		// (h) Add assistant message + tool results to conversation
		conversation.add_assistant(&full_response);

		for result in &tool_results {
			conversation.add_tool_result(&result.id, &result.name, &result.output);
		}

		let turn = LoopTurn {
			turn: turn_num,
			turn_type: TurnType::ToolUse,
			text: if parsed.text.is_empty() {
				None
			} else {
				Some(parsed.text.clone())
			},
			tool_calls: parsed.tool_calls,
			tool_results: tool_results.clone(),
		};
		callbacks.on_turn_complete(&turn);
		turns.push(turn);

		// If this was the last turn, record that we hit the limit
		if turn_num == options.max_turns {
			final_text = parsed.text;
		}
	}

	let total_turns = turns.len();
	let hit_turn_limit = !aborted
		&& total_turns == options.max_turns
		&& turns
			.last()
			.map_or(false, |t| t.turn_type == TurnType::ToolUse);

	AgenticLoopResult {
		final_text,
		turns,
		total_turns,
		hit_turn_limit,
		aborted,
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;
	use simse_ui_core::tools::ToolCallRequest;

	// -- AgenticLoopOptions defaults --

	#[test]
	fn options_defaults() {
		let opts = AgenticLoopOptions::default();
		assert_eq!(opts.max_turns, 10);
		assert!(opts.server_name.is_none());
		assert!(opts.agent_id.is_none());
		assert!(opts.system_prompt.is_none());
		assert!(!opts.agent_manages_tools);
		assert!(opts.session_id.is_none());
	}

	#[test]
	fn options_custom_values() {
		let opts = AgenticLoopOptions {
			max_turns: 5,
			server_name: Some("ollama".into()),
			agent_id: Some("agent-1".into()),
			system_prompt: Some("Be helpful".into()),
			agent_manages_tools: true,
			session_id: Some("sess-42".into()),
			user_input: Some("hello".into()),
		};
		assert_eq!(opts.max_turns, 5);
		assert_eq!(opts.server_name.as_deref(), Some("ollama"));
		assert_eq!(opts.agent_id.as_deref(), Some("agent-1"));
		assert_eq!(opts.system_prompt.as_deref(), Some("Be helpful"));
		assert!(opts.agent_manages_tools);
		assert_eq!(opts.session_id.as_deref(), Some("sess-42"));
		assert_eq!(opts.user_input.as_deref(), Some("hello"));
	}

	// -- TurnType enum --

	#[test]
	fn turn_type_variants() {
		assert_eq!(TurnType::Response, TurnType::Response);
		assert_eq!(TurnType::ToolUse, TurnType::ToolUse);
		assert_eq!(TurnType::Error, TurnType::Error);
		assert_ne!(TurnType::Response, TurnType::ToolUse);
		assert_ne!(TurnType::Response, TurnType::Error);
		assert_ne!(TurnType::ToolUse, TurnType::Error);
	}

	#[test]
	fn turn_type_debug() {
		assert_eq!(format!("{:?}", TurnType::Response), "Response");
		assert_eq!(format!("{:?}", TurnType::ToolUse), "ToolUse");
		assert_eq!(format!("{:?}", TurnType::Error), "Error");
	}

	#[test]
	fn turn_type_clone() {
		let t = TurnType::ToolUse;
		let t2 = t.clone();
		assert_eq!(t, t2);
	}

	// -- LoopTurn construction --

	#[test]
	fn loop_turn_response() {
		let turn = LoopTurn {
			turn: 1,
			turn_type: TurnType::Response,
			text: Some("Hello!".into()),
			tool_calls: Vec::new(),
			tool_results: Vec::new(),
		};
		assert_eq!(turn.turn, 1);
		assert_eq!(turn.turn_type, TurnType::Response);
		assert_eq!(turn.text.as_deref(), Some("Hello!"));
		assert!(turn.tool_calls.is_empty());
		assert!(turn.tool_results.is_empty());
	}

	#[test]
	fn loop_turn_tool_use() {
		let call = ToolCallRequest {
			id: "tc_1".into(),
			name: "search".into(),
			arguments: json!({"query": "rust"}),
		};
		let result = ToolCallResult {
			id: "tc_1".into(),
			name: "search".into(),
			output: "Found 3 results".into(),
			is_error: false,
			duration_ms: None,
			diff: None,
		};
		let turn = LoopTurn {
			turn: 2,
			turn_type: TurnType::ToolUse,
			text: Some("Let me search for that.".into()),
			tool_calls: vec![call],
			tool_results: vec![result],
		};
		assert_eq!(turn.turn, 2);
		assert_eq!(turn.turn_type, TurnType::ToolUse);
		assert_eq!(turn.tool_calls.len(), 1);
		assert_eq!(turn.tool_calls[0].name, "search");
		assert_eq!(turn.tool_results.len(), 1);
		assert!(!turn.tool_results[0].is_error);
	}

	#[test]
	fn loop_turn_error() {
		let turn = LoopTurn {
			turn: 1,
			turn_type: TurnType::Error,
			text: Some("Connection failed".into()),
			tool_calls: Vec::new(),
			tool_results: Vec::new(),
		};
		assert_eq!(turn.turn_type, TurnType::Error);
		assert_eq!(turn.text.as_deref(), Some("Connection failed"));
	}

	#[test]
	fn loop_turn_no_text() {
		let turn = LoopTurn {
			turn: 3,
			turn_type: TurnType::ToolUse,
			text: None,
			tool_calls: Vec::new(),
			tool_results: Vec::new(),
		};
		assert!(turn.text.is_none());
	}

	// -- AgenticLoopResult --

	#[test]
	fn agentic_loop_result_normal() {
		let result = AgenticLoopResult {
			final_text: "Done!".into(),
			turns: vec![LoopTurn {
				turn: 1,
				turn_type: TurnType::Response,
				text: Some("Done!".into()),
				tool_calls: Vec::new(),
				tool_results: Vec::new(),
			}],
			total_turns: 1,
			hit_turn_limit: false,
			aborted: false,
		};
		assert_eq!(result.final_text, "Done!");
		assert_eq!(result.total_turns, 1);
		assert!(!result.hit_turn_limit);
		assert!(!result.aborted);
	}

	#[test]
	fn agentic_loop_result_aborted() {
		let result = AgenticLoopResult {
			final_text: String::new(),
			turns: Vec::new(),
			total_turns: 0,
			hit_turn_limit: false,
			aborted: true,
		};
		assert!(result.aborted);
		assert!(!result.hit_turn_limit);
	}

	#[test]
	fn agentic_loop_result_hit_limit() {
		let result = AgenticLoopResult {
			final_text: String::new(),
			turns: Vec::new(),
			total_turns: 10,
			hit_turn_limit: true,
			aborted: false,
		};
		assert!(result.hit_turn_limit);
		assert!(!result.aborted);
	}

	// -- LoopCallbacks defaults --

	#[test]
	fn noop_callbacks_compile_and_default() {
		let cb = NoopCallbacks;

		// All methods should have default no-op implementations
		cb.on_stream_start();
		cb.on_stream_delta("hello");
		cb.on_tool_call_start(&ToolCallRequest {
			id: "1".into(),
			name: "test".into(),
			arguments: json!({}),
		});
		cb.on_tool_call_end(&ToolCallResult {
			id: "1".into(),
			name: "test".into(),
			output: "ok".into(),
			is_error: false,
			duration_ms: None,
			diff: None,
		});
		cb.on_turn_complete(&LoopTurn {
			turn: 1,
			turn_type: TurnType::Response,
			text: None,
			tool_calls: Vec::new(),
			tool_results: Vec::new(),
		});
		cb.on_error("something bad");
		cb.on_doom_loop("key", 3);
		cb.on_compaction("summary");
		cb.on_token_usage(100, 50);
	}

	#[test]
	fn noop_permission_check_allows_by_default() {
		let cb = NoopCallbacks;
		let call = ToolCallRequest {
			id: "1".into(),
			name: "dangerous_tool".into(),
			arguments: json!({}),
		};
		assert!(cb.on_permission_check(&call));
	}

	// Custom callback that records events
	struct RecordingCallbacks {
		stream_starts: std::sync::Mutex<usize>,
		deltas: std::sync::Mutex<Vec<String>>,
		errors: std::sync::Mutex<Vec<String>>,
		doom_loops: std::sync::Mutex<Vec<(String, usize)>>,
		compactions: std::sync::Mutex<Vec<String>>,
		token_usage: std::sync::Mutex<Vec<(u64, u64)>>,
		permission_result: bool,
	}

	impl RecordingCallbacks {
		fn new() -> Self {
			Self {
				stream_starts: std::sync::Mutex::new(0),
				deltas: std::sync::Mutex::new(Vec::new()),
				errors: std::sync::Mutex::new(Vec::new()),
				doom_loops: std::sync::Mutex::new(Vec::new()),
				compactions: std::sync::Mutex::new(Vec::new()),
				token_usage: std::sync::Mutex::new(Vec::new()),
				permission_result: true,
			}
		}

		fn denying() -> Self {
			Self {
				permission_result: false,
				..Self::new()
			}
		}
	}

	impl LoopCallbacks for RecordingCallbacks {
		fn on_stream_start(&self) {
			*self.stream_starts.lock().unwrap() += 1;
		}
		fn on_stream_delta(&self, delta: &str) {
			self.deltas.lock().unwrap().push(delta.to_string());
		}
		fn on_error(&self, error: &str) {
			self.errors.lock().unwrap().push(error.to_string());
		}
		fn on_doom_loop(&self, tool_key: &str, count: usize) {
			self.doom_loops
				.lock()
				.unwrap()
				.push((tool_key.to_string(), count));
		}
		fn on_compaction(&self, summary: &str) {
			self.compactions.lock().unwrap().push(summary.to_string());
		}
		fn on_token_usage(&self, input: u64, output: u64) {
			self.token_usage.lock().unwrap().push((input, output));
		}
		fn on_permission_check(&self, _call: &ToolCallRequest) -> bool {
			self.permission_result
		}
	}

	#[test]
	fn recording_callbacks_stream_start() {
		let cb = RecordingCallbacks::new();
		cb.on_stream_start();
		cb.on_stream_start();
		assert_eq!(*cb.stream_starts.lock().unwrap(), 2);
	}

	#[test]
	fn recording_callbacks_deltas() {
		let cb = RecordingCallbacks::new();
		cb.on_stream_delta("hello ");
		cb.on_stream_delta("world");
		let deltas = cb.deltas.lock().unwrap();
		assert_eq!(deltas.len(), 2);
		assert_eq!(deltas[0], "hello ");
		assert_eq!(deltas[1], "world");
	}

	#[test]
	fn recording_callbacks_errors() {
		let cb = RecordingCallbacks::new();
		cb.on_error("failed");
		let errors = cb.errors.lock().unwrap();
		assert_eq!(errors.len(), 1);
		assert_eq!(errors[0], "failed");
	}

	#[test]
	fn recording_callbacks_permission_deny() {
		let cb = RecordingCallbacks::denying();
		let call = ToolCallRequest {
			id: "1".into(),
			name: "test".into(),
			arguments: json!({}),
		};
		assert!(!cb.on_permission_check(&call));
	}

	#[test]
	fn recording_callbacks_doom_loop() {
		let cb = RecordingCallbacks::new();
		cb.on_doom_loop("search:abc123", 3);
		let doom = cb.doom_loops.lock().unwrap();
		assert_eq!(doom.len(), 1);
		assert_eq!(doom[0].0, "search:abc123");
		assert_eq!(doom[0].1, 3);
	}

	#[test]
	fn recording_callbacks_compaction() {
		let cb = RecordingCallbacks::new();
		cb.on_compaction("compacted summary");
		let compactions = cb.compactions.lock().unwrap();
		assert_eq!(compactions.len(), 1);
		assert_eq!(compactions[0], "compacted summary");
	}

	#[test]
	fn recording_callbacks_token_usage() {
		let cb = RecordingCallbacks::new();
		cb.on_token_usage(100, 50);
		cb.on_token_usage(200, 100);
		let usage = cb.token_usage.lock().unwrap();
		assert_eq!(usage.len(), 2);
		assert_eq!(usage[0], (100, 50));
		assert_eq!(usage[1], (200, 100));
	}

	// -- DoomLoopTracker --

	#[test]
	fn doom_tracker_default_threshold() {
		let tracker = DoomLoopTracker::default();
		assert_eq!(tracker.threshold, DEFAULT_DOOM_LOOP_THRESHOLD);
		assert_eq!(tracker.threshold, 3);
	}

	#[test]
	fn doom_tracker_custom_threshold() {
		let tracker = DoomLoopTracker::new(5);
		assert_eq!(tracker.threshold, 5);
	}

	#[test]
	fn doom_tracker_tool_key_deterministic() {
		let call = ToolCallRequest {
			id: "tc_1".into(),
			name: "search".into(),
			arguments: json!({"query": "rust"}),
		};
		let key1 = DoomLoopTracker::tool_key(&call);
		let key2 = DoomLoopTracker::tool_key(&call);
		assert_eq!(key1, key2);
		assert!(key1.starts_with("search:"));
	}

	#[test]
	fn doom_tracker_tool_key_different_args() {
		let call1 = ToolCallRequest {
			id: "tc_1".into(),
			name: "search".into(),
			arguments: json!({"query": "rust"}),
		};
		let call2 = ToolCallRequest {
			id: "tc_2".into(),
			name: "search".into(),
			arguments: json!({"query": "python"}),
		};
		let key1 = DoomLoopTracker::tool_key(&call1);
		let key2 = DoomLoopTracker::tool_key(&call2);
		assert_ne!(key1, key2);
	}

	#[test]
	fn doom_tracker_tool_key_different_names() {
		let call1 = ToolCallRequest {
			id: "tc_1".into(),
			name: "search".into(),
			arguments: json!({}),
		};
		let call2 = ToolCallRequest {
			id: "tc_2".into(),
			name: "read".into(),
			arguments: json!({}),
		};
		let key1 = DoomLoopTracker::tool_key(&call1);
		let key2 = DoomLoopTracker::tool_key(&call2);
		assert_ne!(key1, key2);
	}

	#[test]
	fn doom_tracker_tool_key_ignores_id() {
		let call1 = ToolCallRequest {
			id: "tc_1".into(),
			name: "search".into(),
			arguments: json!({"query": "test"}),
		};
		let call2 = ToolCallRequest {
			id: "tc_999".into(),
			name: "search".into(),
			arguments: json!({"query": "test"}),
		};
		assert_eq!(
			DoomLoopTracker::tool_key(&call1),
			DoomLoopTracker::tool_key(&call2)
		);
	}

	#[test]
	fn doom_tracker_no_trigger_below_threshold() {
		let mut tracker = DoomLoopTracker::new(3);
		let call = ToolCallRequest {
			id: "1".into(),
			name: "search".into(),
			arguments: json!({"q": "test"}),
		};

		// First call: count = 1
		assert!(tracker.track(&call).is_none());
		// Second call: count = 2
		assert!(tracker.track(&call).is_none());
	}

	#[test]
	fn doom_tracker_triggers_at_threshold() {
		let mut tracker = DoomLoopTracker::new(3);
		let call = ToolCallRequest {
			id: "1".into(),
			name: "search".into(),
			arguments: json!({"q": "test"}),
		};

		assert!(tracker.track(&call).is_none()); // 1
		assert!(tracker.track(&call).is_none()); // 2
		let result = tracker.track(&call); // 3 = threshold
		assert!(result.is_some());
		let (key, count) = result.unwrap();
		assert_eq!(count, 3);
		assert!(key.starts_with("search:"));
	}

	#[test]
	fn doom_tracker_keeps_triggering_after_threshold() {
		let mut tracker = DoomLoopTracker::new(3);
		let call = ToolCallRequest {
			id: "1".into(),
			name: "search".into(),
			arguments: json!({"q": "test"}),
		};

		tracker.track(&call); // 1
		tracker.track(&call); // 2
		tracker.track(&call); // 3 → triggers
		let result = tracker.track(&call); // 4 → still triggers
		assert!(result.is_some());
		assert_eq!(result.unwrap().1, 4);
	}

	#[test]
	fn doom_tracker_resets_on_different_call() {
		let mut tracker = DoomLoopTracker::new(3);
		let call_a = ToolCallRequest {
			id: "1".into(),
			name: "search".into(),
			arguments: json!({"q": "test"}),
		};
		let call_b = ToolCallRequest {
			id: "2".into(),
			name: "read".into(),
			arguments: json!({"path": "/tmp/file"}),
		};

		// Two calls to A
		assert!(tracker.track(&call_a).is_none()); // 1
		assert!(tracker.track(&call_a).is_none()); // 2

		// Switch to B — resets
		assert!(tracker.track(&call_b).is_none()); // 1

		// Back to A — count starts at 1 again
		assert!(tracker.track(&call_a).is_none()); // 1
		assert!(tracker.track(&call_a).is_none()); // 2
		let result = tracker.track(&call_a); // 3 → triggers
		assert!(result.is_some());
	}

	#[test]
	fn doom_tracker_threshold_one() {
		let mut tracker = DoomLoopTracker::new(1);
		let call = ToolCallRequest {
			id: "1".into(),
			name: "test".into(),
			arguments: json!({}),
		};

		// First call triggers immediately since threshold is 1
		let result = tracker.track(&call);
		assert!(result.is_some());
		assert_eq!(result.unwrap().1, 1);
	}

	#[test]
	fn doom_tracker_reset() {
		let mut tracker = DoomLoopTracker::new(3);
		let call = ToolCallRequest {
			id: "1".into(),
			name: "search".into(),
			arguments: json!({"q": "test"}),
		};

		tracker.track(&call);
		tracker.track(&call);

		let key = DoomLoopTracker::tool_key(&call);
		assert_eq!(tracker.current_count(&key), 2);

		tracker.reset();
		assert_eq!(tracker.current_count(&key), 0);
		assert!(tracker.last_key.is_none());
	}

	#[test]
	fn doom_tracker_current_count_unknown_key() {
		let tracker = DoomLoopTracker::new(3);
		assert_eq!(tracker.current_count("nonexistent:key"), 0);
	}

	// -- Threshold constant --

	#[test]
	fn default_doom_loop_threshold_is_three() {
		assert_eq!(DEFAULT_DOOM_LOOP_THRESHOLD, 3);
	}
}
