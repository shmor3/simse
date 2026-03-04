//! E2E tests: session commands (`/sessions`, `/resume`, `/rename`, `/server`,
//! `/model`, `/mcp`, `/acp`).
//!
//! Session commands that require async bridge operations produce
//! `BridgeRequest(BridgeAction::*)` items stored in `app.pending_bridge_action`
//! for the event loop to dispatch.  Commands that return sync info or success
//! messages are rendered directly to the screen.

use simse_tui::commands::BridgeAction;

use crate::harness::SimseTestHarness;

// ===================================================================
// 1. /sessions shows "No saved sessions" when list is empty
// ===================================================================

#[test]
fn sessions_command_shows_no_sessions() {
	let mut h = SimseTestHarness::new();
	h.submit("/sessions");
	// Default CommandContext has an empty sessions vec, so the handler returns
	// CommandOutput::Info("No saved sessions."), rendered as an Info item.
	h.assert_contains("No saved sessions");
}

// ===================================================================
// 2. /resume sess-1 creates a ResumeSession bridge action
// ===================================================================

#[test]
fn resume_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /resume"
	);

	h.submit("/resume sess-1");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /resume");

	assert_eq!(
		*action,
		BridgeAction::ResumeSession {
			id: "sess-1".into(),
		}
	);
}

// ===================================================================
// 3. /rename Cool Name creates a RenameSession bridge action
// ===================================================================

#[test]
fn rename_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /rename"
	);

	h.submit("/rename Cool Name");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /rename");

	assert_eq!(
		*action,
		BridgeAction::RenameSession {
			title: "Cool Name".into(),
		}
	);
}

// ===================================================================
// 4. /server ollama creates a SwitchServer bridge action
// ===================================================================

#[test]
fn server_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /server"
	);

	h.submit("/server ollama");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /server");

	assert_eq!(
		*action,
		BridgeAction::SwitchServer {
			name: "ollama".into(),
		}
	);
}

// ===================================================================
// 5. /model gpt-4o creates a SwitchModel bridge action
// ===================================================================

#[test]
fn model_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /model"
	);

	h.submit("/model gpt-4o");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /model");

	assert_eq!(
		*action,
		BridgeAction::SwitchModel {
			name: "gpt-4o".into(),
		}
	);
}

// ===================================================================
// 6. /mcp status shows MCP status output
// ===================================================================

#[test]
fn mcp_command_shows_status() {
	let mut h = SimseTestHarness::new();
	h.submit("/mcp status");
	// Default CommandContext has server_name=None and acp_connected=false, so
	// the handler returns: "MCP status: server=none, status=disconnected"
	h.assert_contains("MCP status");
	h.assert_contains("disconnected");
}

// ===================================================================
// 7. /acp restart creates an AcpRestart bridge action
// ===================================================================

#[test]
fn acp_restart_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /acp"
	);

	h.submit("/acp restart");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /acp restart");

	assert_eq!(*action, BridgeAction::AcpRestart);
}
