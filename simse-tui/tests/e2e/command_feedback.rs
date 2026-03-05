//! E2E tests for command feedback, confirmations, and error display.

use super::harness::SimseTestHarness;
use simse_tui::app::Screen;

#[test]
fn factory_reset_shows_confirmation_dialog() {
	let mut h = SimseTestHarness::new();
	h.submit("/factory-reset");
	assert!(matches!(h.current_screen(), Screen::Confirm { .. }));
	h.assert_contains("Are you sure");
}

#[test]
fn factory_reset_confirm_executes_action() {
	let mut h = SimseTestHarness::new();
	h.submit("/factory-reset");
	assert!(matches!(h.current_screen(), Screen::Confirm { .. }));
	h.press_enter();
	assert_eq!(*h.current_screen(), Screen::Chat);
	assert!(h.app.pending_bridge_action.is_some());
}

#[test]
fn factory_reset_cancel_returns_to_chat() {
	let mut h = SimseTestHarness::new();
	h.submit("/factory-reset");
	assert!(matches!(h.current_screen(), Screen::Confirm { .. }));
	h.press_escape();
	assert_eq!(*h.current_screen(), Screen::Chat);
	assert!(h.app.pending_bridge_action.is_none());
	assert!(h.app.pending_confirm_action.is_none());
	h.assert_contains("Cancelled");
}

#[test]
fn search_command_shows_feedback() {
	let mut h = SimseTestHarness::new();
	h.submit("/search test query");
	h.assert_contains("Searching library for: test query");
}

#[test]
fn unknown_command_typo_suggests_similar() {
	let mut h = SimseTestHarness::new();
	h.submit("/sarch test");
	h.assert_contains("Did you mean");
}

#[test]
fn missing_args_shows_usage() {
	let mut h = SimseTestHarness::new();
	h.submit("/add");
	h.assert_contains("Usage:");
}

#[test]
fn status_bar_shows_server_info() {
	let mut h = SimseTestHarness::new();
	h.app.server_name = Some("claude-code".into());
	h.app.model_name = Some("opus-4".into());
	h.render();
	h.assert_contains("claude-code");
	h.assert_contains("opus-4");
}

#[test]
fn empty_sessions_shows_guidance() {
	let mut h = SimseTestHarness::new();
	h.submit("/sessions");
	h.assert_contains("Start chatting");
}
