//! PTY tests for command autocomplete in simse-tui.
//!
//! Covers autocomplete activation, filtering, Tab acceptance, Escape dismissal,
//! and backspace re-filtering — all verified through observable screen output.
//!
//! The autocomplete renders inline completions below the input area in the format:
//! ```text
//!  > /help       Show help information
//!    /clear      Clear the screen
//! ```

use super::r#mod::*;
use ratatui_testlib::KeyCode;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. Typing "/" activates autocomplete with command names
// ═══════════════════════════════════════════════════════════════

#[test]
fn slash_triggers_autocomplete_dropdown() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Type "/" to trigger autocomplete.
	h.send_keys("/").unwrap();
	settle();

	// The inline autocomplete should appear showing commands.
	// Look for the "Show help information" description which uniquely
	// identifies the autocomplete dropdown (vs the Tips area which shows "/help").
	h.wait_for_text("Show help information")
		.expect("Autocomplete should show 'Show help information' description after typing '/'");

	let contents = h.screen_contents();
	// Verify multiple commands are visible (the bare "/" shows all non-hidden commands).
	assert!(
		contents.contains("/clear"),
		"Autocomplete should show '/clear' among all commands. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 2. Typing further filters autocomplete matches
// ═══════════════════════════════════════════════════════════════

#[test]
fn typing_filters_autocomplete() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Type "/he" to filter to commands matching "he" (should match "help").
	h.send_keys("/he").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("Show help information"),
		"Autocomplete should show 'Show help information' for '/he'. Screen:\n{contents}"
	);

	// Commands that don't match "he" should not appear in the autocomplete area.
	// "/config" should be filtered out.
	assert!(
		!contents.contains("Show configuration"),
		"Autocomplete should NOT show '/config' description for '/he'. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 3. Tab accepts the completion
// ═══════════════════════════════════════════════════════════════

#[test]
fn tab_accepts_completion() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Type "/hel" — only "help" should match.
	h.send_keys("/hel").unwrap();
	settle();

	// Verify "help" is in the completions.
	h.wait_for_text("Show help information")
		.expect("Autocomplete should show 'Show help information' for '/hel'");

	// Press Tab to accept the completion.
	h.send_key(KeyCode::Tab).unwrap();
	settle();

	// After acceptance, the autocomplete descriptions should disappear
	// since the autocomplete deactivates.
	let contents = h.screen_contents();
	assert!(
		!contents.contains("Show help information"),
		"Autocomplete completions should close after Tab acceptance. Screen:\n{contents}"
	);

	// The input should now contain "/help" (the completed command).
	assert!(
		contents.contains("/help"),
		"Input should contain '/help' after Tab acceptance. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 4. Escape dismisses autocomplete
// ═══════════════════════════════════════════════════════════════

#[test]
fn escape_dismisses_autocomplete() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Type "/" to activate autocomplete.
	h.send_keys("/").unwrap();
	settle();

	// Verify the completions are visible.
	h.wait_for_text("Show help information")
		.expect("Autocomplete should appear after typing '/'");

	// Press Escape to dismiss.
	send_escape(&mut h);
	settle();

	// The autocomplete descriptions should be gone.
	let contents = h.screen_contents();
	assert!(
		!contents.contains("Show help information"),
		"Autocomplete completions should disappear after Escape. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 5. Backspace re-filters autocomplete (more matches appear)
// ═══════════════════════════════════════════════════════════════

#[test]
fn backspace_refilters_autocomplete() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Type "/he" — should show "help" (prefix match "he").
	// Commands like "search" (contains "h" but not "he") should be filtered.
	h.send_keys("/he").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("Show help information"),
		"Should show 'Show help information' for '/he'. Screen:\n{contents}"
	);
	// "/search" should be filtered out since "search" does not match "he".
	assert!(
		!contents.contains("Search the library"),
		"Should NOT show 'Search the library' for '/he'. Screen:\n{contents}"
	);

	// Press Backspace to go from "/he" to "/h" — more commands should now match.
	h.send_key(KeyCode::Backspace).unwrap();
	settle();

	let contents = h.screen_contents();
	// Now "/help" should still appear.
	assert!(
		contents.contains("Show help information"),
		"Should show 'Show help information' after backspace to '/h'. Screen:\n{contents}"
	);
	// And commands containing "h" should now also appear, like "/shortcuts" or "/search" or "/chain".
	// At minimum, we should see more completions than before.
	assert!(
		contents.contains("Show keyboard shortcuts")
			|| contents.contains("Search the library")
			|| contents.contains("Run a prompt chain"),
		"Should show more completions after backspace to '/h' (e.g. shortcuts, search, or chain). Screen:\n{contents}"
	);
}
