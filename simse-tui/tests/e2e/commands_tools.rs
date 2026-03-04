//! E2E tests: tool/AI commands (`/tools`, `/agents`, `/skills`, `/prompts`, `/chain`).
//!
//! Tool and AI commands that return sync info messages are rendered directly to
//! the screen.  Commands that require async bridge operations produce
//! `BridgeRequest(BridgeAction::*)` items stored in `app.pending_bridge_action`
//! for the event loop to dispatch.

use simse_tui::commands::BridgeAction;

use crate::harness::SimseTestHarness;

// ===================================================================
// 1. /tools shows "No tools registered" when list is empty
// ===================================================================

#[test]
fn tools_command_shows_no_tools() {
	let mut h = SimseTestHarness::new();
	h.submit("/tools");
	// Default CommandContext has an empty tool_defs vec, so the handler returns
	// CommandOutput::Info("No tools registered."), rendered as an Info item.
	h.assert_contains("No tools registered");
}

// ===================================================================
// 2. /agents shows "No agents configured" when list is empty
// ===================================================================

#[test]
fn agents_command_shows_no_agents() {
	let mut h = SimseTestHarness::new();
	h.submit("/agents");
	// Default CommandContext has an empty agents vec, so the handler returns
	// CommandOutput::Info("No agents configured."), rendered as an Info item.
	h.assert_contains("No agents configured");
}

// ===================================================================
// 3. /skills shows "No skills configured" when list is empty
// ===================================================================

#[test]
fn skills_command_shows_no_skills() {
	let mut h = SimseTestHarness::new();
	h.submit("/skills");
	// Default CommandContext has an empty skills vec, so the handler returns
	// CommandOutput::Info("No skills configured."), rendered as an Info item.
	h.assert_contains("No skills configured");
}

// ===================================================================
// 4. /prompts shows "No prompt templates configured" when list is empty
// ===================================================================

#[test]
fn prompts_command_shows_no_prompts() {
	let mut h = SimseTestHarness::new();
	h.submit("/prompts");
	// Default CommandContext has an empty prompts vec, so the handler returns
	// CommandOutput::Info("No prompt templates configured."), rendered as an Info item.
	h.assert_contains("No prompt templates configured");
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
