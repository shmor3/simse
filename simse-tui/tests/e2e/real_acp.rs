//! Real ACP integration tests — always run.
//!
//! Tests ACP-related command flows at the App model level.
//! Verifies command dispatch, bridge action creation, and result handling.

use super::harness::SimseTestHarness;
use simse_tui::app::AppMessage;

#[test]
fn acp_restart_command_sets_bridge_action() {
	let mut h = SimseTestHarness::new();
	h.submit("/acp restart");
	assert!(h.app.pending_bridge_action.is_some());
	h.assert_contains("Restarting ACP connection");
}

#[test]
fn server_switch_command_sets_bridge_action() {
	let mut h = SimseTestHarness::new();
	h.submit("/server claude-code");
	assert!(h.app.pending_bridge_action.is_some());
	h.assert_contains("Switching to server: claude-code");
}

#[test]
fn model_switch_command_sets_bridge_action() {
	let mut h = SimseTestHarness::new();
	h.submit("/model llama3.1");
	assert!(h.app.pending_bridge_action.is_some());
	h.assert_contains("Switching to model: llama3.1");
}

#[test]
fn acp_status_shows_disconnected_by_default() {
	let mut h = SimseTestHarness::new();
	h.submit("/acp status");
	h.assert_contains("disconnected");
}

#[test]
fn mcp_restart_command_sets_bridge_action() {
	let mut h = SimseTestHarness::new();
	h.submit("/mcp restart");
	assert!(h.app.pending_bridge_action.is_some());
	h.assert_contains("Restarting MCP connections");
}

#[test]
fn bridge_result_success_displays_in_output() {
	let mut h = SimseTestHarness::new();
	h.send(AppMessage::BridgeResult {
		action: "acp-restart".into(),
		text: "ACP connection restarted.".into(),
		is_error: false,
	});
	h.assert_contains("ACP connection restarted");
}
