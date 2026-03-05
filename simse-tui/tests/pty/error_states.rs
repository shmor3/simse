//! PTY tests for error states: unknown commands, empty submit, bare slash.
//!
//! These tests verify that the real binary handles invalid input gracefully:
//! - Unknown commands produce visible error messages
//! - Empty submit is a no-op (app keeps running)
//! - Bare "/" shows an "Invalid command" error

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. Unknown command shows error
// ═══════════════════════════════════════════════════════════════

#[test]
fn unknown_command_shows_error() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/foobar");

	h.wait_for_text("Unknown command")
		.expect("'/foobar' should show 'Unknown command' error on screen");
}

// ═══════════════════════════════════════════════════════════════
// 2. Empty submit is a no-op
// ═══════════════════════════════════════════════════════════════

#[test]
fn empty_submit_is_noop() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Press Enter with empty input.
	h.send_key(KeyCode::Enter).unwrap();
	settle();

	// App should still be running and not crash.
	assert!(h.is_running(), "App should still be running after empty submit");

	// The banner should still be visible (app is in normal state).
	let contents = h.screen_contents();
	assert!(
		contents.contains("simse v"),
		"Banner should still be visible after empty submit. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 3. Slash only shows error
// ═══════════════════════════════════════════════════════════════

#[test]
fn slash_only_shows_error() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/");

	h.wait_for_text("Invalid command")
		.expect("'/' should show 'Invalid command' error on screen");
}
