//! E2E tests: tool/AI commands (`/tools`, `/agents`, `/skills`, `/prompts`, `/chain`).
//!
//! Tool and AI commands that return sync info messages are rendered directly to
//! the screen with actionable guidance.  Commands that require async bridge
//! operations produce `BridgeRequest(BridgeAction::*)` items stored in
//! `app.pending_bridge_action` for the event loop to dispatch, with a feedback
//! message on screen.

use simse_tui::commands::BridgeAction;

use crate::harness::SimseTestHarness;

// ===================================================================
// 1. /tools shows actionable guidance when list is empty
// ===================================================================

#[test]
fn tools_command_shows_no_tools() {
	let mut h = SimseTestHarness::new();
	h.submit("/tools");
	// Default CommandContext has an empty tool_defs vec, so the handler returns
	// an Info with actionable guidance.
	h.assert_contains("No tools registered");
	h.assert_contains("/setup");
}

// ===================================================================
// 2. /agents shows actionable guidance when list is empty
// ===================================================================

#[test]
fn agents_command_shows_no_agents() {
	let mut h = SimseTestHarness::new();
	h.submit("/agents");
	// Default CommandContext has an empty agents vec, so the handler returns
	// an Info with actionable guidance.
	h.assert_contains("No agents configured");
	h.assert_contains(".simse/agents/");
}

// ===================================================================
// 3. /skills shows actionable guidance when list is empty
// ===================================================================

#[test]
fn skills_command_shows_no_skills() {
	let mut h = SimseTestHarness::new();
	h.submit("/skills");
	// Default CommandContext has an empty skills vec, so the handler returns
	// an Info with actionable guidance.
	h.assert_contains("No skills configured");
	h.assert_contains(".simse/skills/");
}

// ===================================================================
// 4. /prompts shows actionable guidance when list is empty
// ===================================================================

#[test]
fn prompts_command_shows_no_prompts() {
	let mut h = SimseTestHarness::new();
	h.submit("/prompts");
	// Default CommandContext has an empty prompts vec, so the handler returns
	// an Info with actionable guidance.
	h.assert_contains("No prompt templates configured");
	h.assert_contains(".simse/prompts.json");
}

// ===================================================================
// 5. /chain summarize creates a RunChain bridge action
// ===================================================================

#[test]
fn chain_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /chain"
	);

	h.submit("/chain summarize");

	// Verify feedback message appears on screen.
	h.assert_contains("Running chain: summarize");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /chain");

	assert_eq!(
		*action,
		BridgeAction::RunChain {
			name: "summarize".into(),
			args: "".into(),
		}
	);
}
