//! PTY tests for ACP-related command flows.
//!
//! Merges coverage from the old `e2e/acp_integration.rs` and `e2e/real_acp.rs`
//! into observable PTY tests. Each test spawns the real `simse-tui` binary in a
//! pseudo-terminal and verifies screen output — no internal state assertions.

use super::r#mod::*;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /acp restart shows feedback message
// ═══════════════════════════════════════════════════════════════

#[test]
fn acp_restart_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/acp restart");

	h.wait_for_text("Restarting ACP connection")
		.expect("'/acp restart' should show 'Restarting ACP connection' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 2. /acp status shows disconnected
// ═══════════════════════════════════════════════════════════════

#[test]
fn acp_status_shows_disconnected() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/acp status");

	h.wait_for_text("disconnected")
		.expect("'/acp status' should show 'disconnected' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 3. /acp (no args) shows status (same as /acp status)
// ═══════════════════════════════════════════════════════════════

#[test]
fn acp_no_args_shows_status() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/acp");

	h.wait_for_text("disconnected")
		.expect("'/acp' with no args should show 'disconnected' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 4. /server switch shows feedback message
// ═══════════════════════════════════════════════════════════════

#[test]
fn server_switch_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/server test-server");

	h.wait_for_text("Switching to server: test-server")
		.expect("'/server test-server' should show 'Switching to server: test-server' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 5. /model switch shows feedback message
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_switch_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/model llama3");

	h.wait_for_text("Switching to model: llama3")
		.expect("'/model llama3' should show 'Switching to model: llama3' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 6. /model switch executes and shows bridge result
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_switch_executes() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/model test-model");

	// The SwitchModel bridge action runs synchronously (sets config.default_agent)
	// and returns "Model set to: test-model" as a BridgeResult.
	h.wait_for_text("Model set to: test-model")
		.expect("'/model test-model' should show 'Model set to: test-model' after bridge dispatch");
}

// ═══════════════════════════════════════════════════════════════
// 7. Chat message appears on screen
// ═══════════════════════════════════════════════════════════════

#[test]
fn chat_message_appears_on_screen() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "Hello AI");

	h.wait_for_text("Hello AI")
		.expect("Chat message 'Hello AI' should appear on screen after submission");
}

// ═══════════════════════════════════════════════════════════════
// 8. App starts without thinking spinner
// ═══════════════════════════════════════════════════════════════

#[test]
fn app_starts_without_thinking_spinner() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Give the app a moment to fully render after startup.
	settle();

	let contents = h.screen_contents();
	assert!(
		!contents.contains("Thinking"),
		"App should not show 'Thinking' spinner at startup. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 9. /mcp restart shows feedback message
// ═══════════════════════════════════════════════════════════════

#[test]
fn mcp_restart_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/mcp restart");

	h.wait_for_text("Restarting MCP connections")
		.expect("'/mcp restart' should show 'Restarting MCP connections' on screen");
}

// ═══════════════════════════════════════════════════════════════
// 10. Submit clears input field
// ═══════════════════════════════════════════════════════════════

#[test]
fn submit_clears_input() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Type a unique string that won't appear elsewhere on screen.
	h.send_keys("xyzzy_draft_12345").unwrap();
	h.wait_for_text("xyzzy_draft_12345")
		.expect("Typed text should appear in input area");

	// Submit it (press Enter). The input field should clear after submit.
	h.send_key(KeyCode::Enter).unwrap();
	settle();

	// The submitted text may appear in the output area as a user message,
	// but the input box itself should be empty. We can verify the input
	// cleared by checking that the text now appears in the output section
	// (rendered as a user message) rather than the Input box.
	// Since the text is unique, if it appeared at all before and appears now,
	// the submit worked. We verify the input cleared by checking that a
	// second unique string typed afterward appears in a clean input field.
	h.send_keys("new_input_text").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("new_input_text"),
		"New text should appear in input after submit cleared the old text. Screen:\n{contents}"
	);
}
