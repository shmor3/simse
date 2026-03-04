//! E2E autocomplete tests for simse-tui.
//!
//! Covers autocomplete activation, filtering, Tab acceptance, Up/Down navigation,
//! Escape dismissal, ghost text, and backspace re-filtering — all driven through the
//! full Elm Architecture update/view cycle via `SimseTestHarness`.

use crate::harness::SimseTestHarness;

// ═══════════════════════════════════════════════════════════════
// 1. Slash triggers autocomplete
// ═══════════════════════════════════════════════════════════════

#[test]
fn slash_triggers_autocomplete() {
	let mut h = SimseTestHarness::new();
	assert!(!h.app.autocomplete.is_active());

	// Typing "/" should activate autocomplete with matches (all non-hidden commands).
	h.type_text("/");
	assert!(
		h.app.autocomplete.is_active(),
		"Autocomplete should activate when '/' is typed",
	);
	assert!(
		!h.app.autocomplete.matches.is_empty(),
		"Autocomplete should have matches after typing '/'",
	);
}

// ═══════════════════════════════════════════════════════════════
// 2. Typing filters matches
// ═══════════════════════════════════════════════════════════════

#[test]
fn typing_filters_matches() {
	let mut h = SimseTestHarness::new();

	// Type "/" to get all matches.
	h.type_text("/");
	let all_count = h.app.autocomplete.matches.len();
	assert!(all_count > 1, "Bare '/' should produce multiple matches");

	// Continue typing "he" to filter — only "help" starts with "he".
	h.type_text("he");
	assert!(h.app.autocomplete.is_active());
	let filtered_count = h.app.autocomplete.matches.len();
	assert!(
		filtered_count < all_count,
		"Typing '/he' should produce fewer matches ({}) than bare '/' ({})",
		filtered_count,
		all_count,
	);
	assert!(
		h.app.autocomplete.matches.iter().any(|m| m.name == "help"),
		"'help' should be among the matches for '/he'",
	);
}

// ═══════════════════════════════════════════════════════════════
// 3. Tab accepts completion
// ═══════════════════════════════════════════════════════════════

#[test]
fn tab_accepts_completion() {
	let mut h = SimseTestHarness::new();

	// Type "/hel" — only "help" should match.
	h.type_text("/hel");
	assert!(h.app.autocomplete.is_active());
	assert_eq!(h.app.autocomplete.matches.len(), 1);
	assert_eq!(h.app.autocomplete.matches[0].name, "help");

	// Press Tab to accept the completion.
	h.press_tab();

	// Autocomplete should deactivate after acceptance.
	assert!(
		!h.app.autocomplete.is_active(),
		"Autocomplete should deactivate after Tab acceptance",
	);

	// Input should now contain the completed command with a trailing space.
	assert_eq!(
		h.input_value(),
		"/help ",
		"Input should be '/help ' after Tab acceptance",
	);
}

// ═══════════════════════════════════════════════════════════════
// 4. Up/Down navigate matches
// ═══════════════════════════════════════════════════════════════

#[test]
fn up_down_navigate_matches() {
	let mut h = SimseTestHarness::new();

	// Type "/" to activate autocomplete with multiple matches.
	h.type_text("/");
	assert!(h.app.autocomplete.is_active());
	assert!(
		h.app.autocomplete.matches.len() > 1,
		"Need multiple matches to test navigation",
	);
	let initial_selected = h.app.autocomplete.selected;

	// Press Down: selected index should increase by 1.
	h.press_down();
	assert_eq!(
		h.app.autocomplete.selected,
		initial_selected + 1,
		"Down arrow should advance selected index",
	);

	// Press Up: selected index should return to initial.
	h.press_up();
	assert_eq!(
		h.app.autocomplete.selected,
		initial_selected,
		"Up arrow should return selected index to initial",
	);
}

// ═══════════════════════════════════════════════════════════════
// 5. Escape dismisses autocomplete
// ═══════════════════════════════════════════════════════════════

#[test]
fn escape_dismisses_autocomplete() {
	let mut h = SimseTestHarness::new();

	// Activate autocomplete.
	h.type_text("/");
	assert!(
		h.app.autocomplete.is_active(),
		"Autocomplete should be active after typing '/'",
	);

	// Press Escape to dismiss.
	h.press_escape();
	assert!(
		!h.app.autocomplete.is_active(),
		"Autocomplete should deactivate after Escape",
	);
}

// ═══════════════════════════════════════════════════════════════
// 6. Ghost text shows for single match
// ═══════════════════════════════════════════════════════════════

#[test]
fn ghost_text_shows_for_single_match() {
	let mut h = SimseTestHarness::new();

	// Type "/compac" — only "compact" should match, ghost text = "t".
	h.type_text("/compac");
	assert!(h.app.autocomplete.is_active());
	assert_eq!(
		h.app.autocomplete.matches.len(),
		1,
		"Only 'compact' should match '/compac'",
	);
	assert_eq!(h.app.autocomplete.matches[0].name, "compact");

	let ghost = h.app.autocomplete.ghost_text();
	assert_eq!(
		ghost,
		Some("t".into()),
		"Ghost text should be 't' to complete 'compac' -> 'compact'",
	);
}

// ═══════════════════════════════════════════════════════════════
// 7. Backspace updates matches
// ═══════════════════════════════════════════════════════════════

#[test]
fn backspace_updates_matches() {
	let mut h = SimseTestHarness::new();

	// Type "/he" — should match a subset.
	h.type_text("/he");
	assert!(h.app.autocomplete.is_active());
	let count_he = h.app.autocomplete.matches.len();

	// Press Backspace to go from "/he" -> "/h".
	h.press_backspace();
	assert_eq!(h.input_value(), "/h");
	assert!(h.app.autocomplete.is_active());
	let count_h = h.app.autocomplete.matches.len();

	assert!(
		count_h >= count_he,
		"Backspace from '/he' ({} matches) to '/h' ({} matches) should not reduce match count",
		count_he,
		count_h,
	);
}
