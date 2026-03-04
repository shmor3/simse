//! E2E tests: overlay behavior (Settings, Librarians, Setup, Shortcuts).
//!
//! These tests exercise overlay opening, navigation, dismissal, and focus
//! routing through the full App model's `update()` → `view()` cycle.

use simse_tui::app::Screen;

use crate::harness::SimseTestHarness;

// ═══════════════════════════════════════════════════════════════
// 1. /settings opens and renders settings overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn settings_overlay_opens_and_renders() {
	let mut h = SimseTestHarness::new();
	h.submit("/settings");

	// Screen should transition to Settings.
	assert_eq!(h.current_screen(), &Screen::Settings);

	// The settings explorer renders config file entries. Verify some
	// are visible (rendered by render_settings_explorer).
	h.assert_contains("Settings");
	h.assert_contains("config.json");
}

// ═══════════════════════════════════════════════════════════════
// 2. Settings overlay navigation (Down/Up changes selected_file)
// ═══════════════════════════════════════════════════════════════

#[test]
fn settings_overlay_navigation() {
	let mut h = SimseTestHarness::new();
	h.submit("/settings");
	assert_eq!(h.current_screen(), &Screen::Settings);

	// Initially selected_file is 0.
	assert_eq!(h.app.settings_state.selected_file, 0);

	// Press Down: selected_file should advance to 1.
	h.press_down();
	assert_eq!(h.app.settings_state.selected_file, 1);

	// Press Down again: selected_file should advance to 2.
	h.press_down();
	assert_eq!(h.app.settings_state.selected_file, 2);

	// Press Up: selected_file should go back to 1.
	h.press_up();
	assert_eq!(h.app.settings_state.selected_file, 1);
}

// ═══════════════════════════════════════════════════════════════
// 3. Settings overlay Escape closes and returns to Chat
// ═══════════════════════════════════════════════════════════════

#[test]
fn settings_overlay_escape_closes() {
	let mut h = SimseTestHarness::new();
	h.submit("/settings");
	assert_eq!(h.current_screen(), &Screen::Settings);

	// At FileList level, Escape calls settings_state.back() which
	// returns true → screen transitions to Chat.
	h.press_escape();
	assert_eq!(h.current_screen(), &Screen::Chat);
}

// ═══════════════════════════════════════════════════════════════
// 4. /librarians opens librarians overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn librarians_overlay_opens() {
	let mut h = SimseTestHarness::new();
	h.submit("/librarians");

	// Screen should transition to Librarians.
	assert_eq!(h.current_screen(), &Screen::Librarians);

	// The librarian explorer renders its content. Verify the overlay
	// title or characteristic text is visible.
	h.assert_contains("Librarian");
}

// ═══════════════════════════════════════════════════════════════
// 5. Librarians overlay Escape closes and returns to Chat
// ═══════════════════════════════════════════════════════════════

#[test]
fn librarians_overlay_escape_closes() {
	let mut h = SimseTestHarness::new();
	h.submit("/librarians");
	assert_eq!(h.current_screen(), &Screen::Librarians);

	// At top level, back() returns true → screen transitions to Chat.
	h.press_escape();
	assert_eq!(h.current_screen(), &Screen::Chat);
}

// ═══════════════════════════════════════════════════════════════
// 6. /setup opens setup overlay with no preset
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_overlay_opens() {
	let mut h = SimseTestHarness::new();
	h.submit("/setup");

	// Screen should transition to Setup with no preset.
	assert_eq!(
		h.current_screen(),
		&Screen::Setup { preset: None }
	);

	// The setup selector renders its content. Verify visible text.
	h.assert_contains("Setup");
}

// ═══════════════════════════════════════════════════════════════
// 7. "?" opens shortcuts overlay, any key dismisses it
// ═══════════════════════════════════════════════════════════════

#[test]
fn shortcuts_overlay_opens_and_dismisses() {
	let mut h = SimseTestHarness::new();

	// Input must be empty for "?" to trigger shortcuts.
	assert!(h.input_value().is_empty());

	// Type "?" to open shortcuts.
	h.type_text("?");
	assert_eq!(h.current_screen(), &Screen::Shortcuts);

	// Shortcuts overlay should show keyboard shortcut info.
	h.assert_contains("Keyboard Shortcuts");

	// Any character input dismisses the overlay (goes back to Chat).
	h.send(simse_tui::app::AppMessage::CharInput('a'));
	assert_eq!(h.current_screen(), &Screen::Chat);
}

// ═══════════════════════════════════════════════════════════════
// 8. Overlay blocks input from reaching the text field
// ═══════════════════════════════════════════════════════════════

#[test]
fn overlay_blocks_input() {
	let mut h = SimseTestHarness::new();

	// Open the settings overlay via /settings command.
	h.submit("/settings");
	assert_eq!(h.current_screen(), &Screen::Settings);

	// The submit cleared the input field. Verify it is now empty.
	assert_eq!(h.input_value(), "");

	// Type characters while the settings overlay is active.
	// Because screen == Settings, CharInput is routed to the overlay
	// (settings_state.type_char), NOT to the input field.
	h.type_text("abc");

	// Input value should remain empty — chars went to the overlay.
	assert_eq!(h.input_value(), "");

	// Also verify that Backspace is routed to the overlay, not the input.
	h.press_backspace();
	assert_eq!(h.input_value(), "");

	// The screen should still be Settings (not dismissed by typing).
	assert_eq!(h.current_screen(), &Screen::Settings);
}
