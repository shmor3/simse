//! E2E tests: ACP integration behavior.
//!
//! These tests verify bridge action dispatch and UI state when ACP-related
//! commands are issued.  No real ACP server is needed — they exercise the
//! UI state machine only (Elm Architecture update/view cycle).

use simse_tui::app::{AppMessage, LoopStatus};
use simse_tui::commands::BridgeAction;
use simse_ui_core::app::OutputItem;

use crate::harness::SimseTestHarness;

// ===================================================================
// 1. /acp restart creates an AcpRestart bridge action
// ===================================================================

#[test]
fn acp_command_restart_creates_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /acp restart"
	);

	h.submit("/acp restart");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /acp restart");

	assert_eq!(*action, BridgeAction::AcpRestart);
}

// ===================================================================
// 2. /acp status shows ACP status info on screen
// ===================================================================

#[test]
fn acp_command_status_shows_info() {
	let mut h = SimseTestHarness::new();
	h.submit("/acp status");

	// Default CommandContext has acp_connected=false, so the handler returns:
	// "ACP status: disconnected"
	h.assert_contains("ACP status");
	h.assert_contains("disconnected");
}

// ===================================================================
// 3. /acp (no args) shows ACP status (same as /acp status)
// ===================================================================

#[test]
fn acp_command_no_args_shows_usage() {
	let mut h = SimseTestHarness::new();
	h.submit("/acp");

	// With no subcommand, handle_acp falls through to the "" | "status" arm
	// and displays "ACP status: disconnected" (since acp_connected defaults to false).
	h.assert_contains("ACP status");
	h.assert_contains("disconnected");
}

// ===================================================================
// 4. /server claude-code creates a SwitchServer bridge action
// ===================================================================

#[test]
fn server_switch_creates_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /server"
	);

	h.submit("/server claude-code");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /server claude-code");

	assert_eq!(
		*action,
		BridgeAction::SwitchServer {
			name: "claude-code".into(),
		}
	);
}

// ===================================================================
// 5. /model llama3 creates a SwitchModel bridge action
// ===================================================================

#[test]
fn model_switch_creates_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /model"
	);

	h.submit("/model llama3");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /model llama3");

	assert_eq!(
		*action,
		BridgeAction::SwitchModel {
			name: "llama3".into(),
		}
	);
}

// ===================================================================
// 6. Non-command text is stored as a user message in output
// ===================================================================

#[test]
fn chat_message_stores_pending() {
	let mut h = SimseTestHarness::new();
	assert!(h.app.output.is_empty(), "Output should be empty initially");

	h.submit("Hello AI");

	// Non-command text (no "/" prefix) gets pushed as OutputItem::Message
	// with role "user".
	assert!(
		!h.app.output.is_empty(),
		"Output should have at least one item after submitting chat text"
	);

	let has_user_msg = h.app.output.iter().any(|item| {
		matches!(item, OutputItem::Message { role, text } if role == "user" && text == "Hello AI")
	});
	assert!(
		has_user_msg,
		"Expected a user message 'Hello AI' in output, got: {:?}",
		h.app.output
	);
}

// ===================================================================
// 7. App starts with LoopStatus::Idle
// ===================================================================

#[test]
fn loop_status_starts_idle() {
	let h = SimseTestHarness::new();
	assert_eq!(
		h.app.loop_status,
		LoopStatus::Idle,
		"App should start with LoopStatus::Idle"
	);
}

// ===================================================================
// 8. Escape during Streaming interrupts and returns to Idle
// ===================================================================

#[test]
fn escape_during_streaming_interrupts() {
	let mut h = SimseTestHarness::new();

	// Simulate that the agentic loop started streaming.
	h.send(AppMessage::StreamStart);
	assert_eq!(
		h.app.loop_status,
		LoopStatus::Streaming,
		"After StreamStart, loop_status should be Streaming"
	);

	// Press Escape to interrupt.
	h.press_escape();

	// Verify the loop went back to Idle.
	assert_eq!(
		h.app.loop_status,
		LoopStatus::Idle,
		"After Escape during Streaming, loop_status should be Idle"
	);

	// Verify "Interrupted." appears in the output.
	let has_interrupted = h.app.output.iter().any(|item| {
		matches!(item, OutputItem::Info { text } if text == "Interrupted.")
	});
	assert!(
		has_interrupted,
		"Expected 'Interrupted.' info item in output after Escape during Streaming"
	);

	// Also verify it renders on screen.
	h.assert_contains("Interrupted");
}
