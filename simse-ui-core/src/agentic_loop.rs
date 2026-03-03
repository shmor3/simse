//! Agentic loop orchestration: conversation -> generate -> tool calls -> repeat.

use serde::{Deserialize, Serialize};

use crate::tools::ToolCallRequest;
use crate::tools::ToolCallResult;

/// A single turn in the agentic loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopTurn {
	pub turn: usize,
	pub turn_type: TurnType,
	pub text: Option<String>,
	pub tool_calls: Vec<ToolCallRequest>,
	pub tool_results: Vec<ToolCallResult>,
}

/// Type of turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnType {
	Text,
	ToolUse,
}

/// Result of running the full agentic loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticLoopResult {
	pub final_text: String,
	pub turns: Vec<LoopTurn>,
	pub total_turns: usize,
	pub hit_turn_limit: bool,
	pub aborted: bool,
}

/// Token usage stats from a generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
	pub prompt_tokens: u64,
	pub completion_tokens: u64,
	pub total_tokens: u64,
}
