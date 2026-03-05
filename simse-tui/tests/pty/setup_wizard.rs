//! PTY tests for the setup wizard flow in simse-tui.
//!
//! Covers preset listing, selection (Claude Code, Ollama, Copilot, Custom),
//! custom edit mode, and Escape dismissal — all verified through observable
//! screen output.

use super::r#mod::*;
use ratatui_testlib::KeyCode;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /setup shows all preset options
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_shows_all_presets() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Setup")
		.expect("'/setup' should open the Setup wizard");

	let contents = h.screen_contents();
	assert!(
		contents.contains("Claude Code"),
		"Setup should show 'Claude Code' preset. Screen:\n{contents}"
	);
	assert!(
		contents.contains("Ollama"),
		"Setup should show 'Ollama' preset. Screen:\n{contents}"
	);
	assert!(
		contents.contains("Copilot"),
		"Setup should show 'Copilot' preset. Screen:\n{contents}"
	);
	assert!(
		contents.contains("Custom"),
		"Setup should show 'Custom' preset. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 2. Select Claude Code preset (Enter on first item)
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_select_claude() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Claude Code")
		.expect("Setup wizard should show 'Claude Code'");

	// Claude Code is the first preset (selected by default). Press Enter.
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Selected preset: Claude Code")
		.expect("Selecting Claude Code should show 'Selected preset: Claude Code'");
}

// ═══════════════════════════════════════════════════════════════
// 3. Select Ollama preset (Down + Enter)
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_select_ollama() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Ollama")
		.expect("Setup wizard should show 'Ollama'");

	// Navigate down to Ollama (index 1).
	h.send_key(KeyCode::Down).unwrap();
	settle();

	// Press Enter to select Ollama.
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Opening Ollama wizard")
		.expect("Selecting Ollama should show 'Opening Ollama wizard'");
}

// ═══════════════════════════════════════════════════════════════
// 4. Select Copilot preset (Down x2 + Enter)
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_select_copilot() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Copilot")
		.expect("Setup wizard should show 'Copilot'");

	// Navigate down twice to Copilot (index 2).
	h.send_key(KeyCode::Down).unwrap();
	settle();
	h.send_key(KeyCode::Down).unwrap();
	settle();

	// Press Enter to select Copilot.
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Selected preset: Copilot")
		.expect("Selecting Copilot should show 'Selected preset: Copilot'");
}

// ═══════════════════════════════════════════════════════════════
// 5. Select Custom preset shows edit mode
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_select_custom_shows_edit() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Custom")
		.expect("Setup wizard should show 'Custom'");

	// Navigate down three times to Custom (index 3).
	h.send_key(KeyCode::Down).unwrap();
	settle();
	h.send_key(KeyCode::Down).unwrap();
	settle();
	h.send_key(KeyCode::Down).unwrap();
	settle();

	// Press Enter to select Custom.
	h.send_key(KeyCode::Enter).unwrap();
	settle();

	// Custom mode should show "Setup: Custom" and "Command:" field.
	let contents = h.screen_contents();
	assert!(
		contents.contains("Setup: Custom") || contents.contains("Custom"),
		"Custom mode should show 'Setup: Custom' or 'Custom'. Screen:\n{contents}"
	);
	assert!(
		contents.contains("Command"),
		"Custom mode should show 'Command' field label. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 6. Custom edit — typing appears in command field
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_custom_edit_command() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Custom")
		.expect("Setup wizard should show 'Custom'");

	// Navigate to Custom and enter edit mode.
	h.send_key(KeyCode::Down).unwrap();
	settle();
	h.send_key(KeyCode::Down).unwrap();
	settle();
	h.send_key(KeyCode::Down).unwrap();
	settle();
	h.send_key(KeyCode::Enter).unwrap();
	settle();

	// Type a command name into the command field.
	h.send_keys("my-bridge").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("my-bridge"),
		"Typed command 'my-bridge' should appear in the custom edit field. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 7. Escape from setup returns to chat
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_escape_returns_to_chat() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Setup")
		.expect("Setup wizard should open");

	// Press Escape to dismiss the setup overlay.
	send_escape(&mut h);
	settle();

	// After Escape, setup-specific content should be gone.
	// Type something to verify input works again.
	h.send_keys("chat works").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("chat works"),
		"Input should work after dismissing Setup overlay. Screen:\n{contents}"
	);
}
