//! PTY tests for overlay behavior in simse-tui.
//!
//! Covers opening and dismissing overlays (Settings, Librarians, Shortcuts),
//! and verifying that overlays block chat input — all through observable screen output.

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /settings opens the Settings overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn settings_overlay_opens() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/settings");

	h.wait_for_text("Settings")
		.expect("'/settings' should open the Settings overlay");

	let contents = h.screen_contents();
	assert!(
		contents.contains("config.json"),
		"Settings overlay should show 'config.json'. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 2. Settings overlay Escape returns to chat
// ═══════════════════════════════════════════════════════════════

#[test]
fn settings_overlay_escape_returns() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/settings");

	h.wait_for_text("Settings")
		.expect("Settings overlay should open");

	// Press Escape to close the overlay.
	send_escape(&mut h);
	settle();

	// After closing, the overlay text should be gone and we should be back to chat.
	// The input area should be visible again (indicated by the banner/ready state).
	let contents = h.screen_contents();

	// Verify the settings-specific content is gone.
	// Note: "Settings" might still appear in the status bar or tips,
	// but "config.json" is unique to the settings overlay.
	assert!(
		!contents.contains("config.json"),
		"Settings overlay content ('config.json') should be gone after Escape. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 3. /librarians opens the Librarians overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn librarians_overlay_opens() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/librarians");

	h.wait_for_text("Librarian")
		.expect("'/librarians' should open the Librarians overlay with 'Librarian' text");
}

// ═══════════════════════════════════════════════════════════════
// 4. Librarians overlay Escape returns to chat
// ═══════════════════════════════════════════════════════════════

#[test]
fn librarians_overlay_escape_returns() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/librarians");

	h.wait_for_text("Librarian")
		.expect("Librarians overlay should open");

	// Press Escape to close.
	send_escape(&mut h);
	settle();

	// After Escape, we should be back to the chat. Type something
	// to verify input works again.
	h.send_keys("test input works").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("test input works"),
		"Input should work after dismissing Librarians overlay. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 5. "?" shows Keyboard Shortcuts overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn shortcuts_overlay_shows() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// "?" with empty input opens the shortcuts overlay.
	h.send_keys("?").unwrap();
	settle();

	h.wait_for_text("Keyboard Shortcuts")
		.expect("'?' should open the Keyboard Shortcuts overlay");

	// Any key should dismiss it.
	h.send_keys("a").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		!contents.contains("Keyboard Shortcuts"),
		"Keyboard Shortcuts overlay should dismiss after pressing any key. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 6. Overlay blocks chat input
// ═══════════════════════════════════════════════════════════════

#[test]
fn overlay_blocks_chat_input() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/settings");

	h.wait_for_text("Settings")
		.expect("Settings overlay should open");

	// Type characters while the overlay is open. These should NOT appear
	// in the input field — they go to the overlay instead.
	h.send_keys("xyz").unwrap();
	settle();

	// The overlay should still be visible (typing didn't dismiss it).
	let contents = h.screen_contents();
	assert!(
		contents.contains("Settings"),
		"Settings overlay should still be open after typing. Screen:\n{contents}"
	);

	// Close the overlay and verify that "xyz" did not end up in the input.
	send_escape(&mut h);
	settle();

	let contents = h.screen_contents();
	assert!(
		!contents.contains("xyz"),
		"Typed characters 'xyz' should NOT appear in the input after overlay was open. Screen:\n{contents}"
	);
}
