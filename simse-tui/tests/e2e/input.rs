//! E2E input tests for simse-tui.
//!
//! Covers text input, cursor movement, deletion, paste, history navigation,
//! Ctrl+C behavior, and word-level deletion.

use crate::harness::SimseTestHarness;

// ═══════════════════════════════════════════════════════════════
// 1. Typing text appears in input
// ═══════════════════════════════════════════════════════════════

#[test]
fn typing_text_appears_in_input() {
	let mut h = SimseTestHarness::new();
	h.type_text("hello");
	assert_eq!(h.input_value(), "hello");
	assert_eq!(h.app.input.cursor, 5);
}

// ═══════════════════════════════════════════════════════════════
// 2. Backspace deletes character
// ═══════════════════════════════════════════════════════════════

#[test]
fn backspace_deletes_character() {
	let mut h = SimseTestHarness::new();
	h.type_text("hello");
	assert_eq!(h.input_value(), "hello");

	h.press_backspace();
	assert_eq!(h.input_value(), "hell");
	assert_eq!(h.app.input.cursor, 4);
}

// ═══════════════════════════════════════════════════════════════
// 3. Delete key works
// ═══════════════════════════════════════════════════════════════

#[test]
fn delete_key_works() {
	let mut h = SimseTestHarness::new();
	h.type_text("hello");
	// Cursor is at position 5 (end). Move left once to position 4 (before 'o').
	h.press_left();
	assert_eq!(h.app.input.cursor, 4);

	// Delete removes the character at cursor (the 'o').
	h.press_delete();
	assert_eq!(h.input_value(), "hell");
	assert_eq!(h.app.input.cursor, 4);
}

// ═══════════════════════════════════════════════════════════════
// 4. Arrow keys move cursor (insert at cursor position)
// ═══════════════════════════════════════════════════════════════

#[test]
fn arrow_keys_move_cursor() {
	let mut h = SimseTestHarness::new();
	h.type_text("hello");
	assert_eq!(h.app.input.cursor, 5);

	// Move left once: cursor goes to 4 (between 'l' and 'o').
	h.press_left();
	assert_eq!(h.app.input.cursor, 4);

	// Type 'X' at position 4: inserts before the 'o'.
	h.type_text("X");
	assert_eq!(h.input_value(), "hellXo");
	assert_eq!(h.app.input.cursor, 5);

	// Move right once: cursor goes to 6 (end).
	h.press_right();
	assert_eq!(h.app.input.cursor, 6);
}

// ═══════════════════════════════════════════════════════════════
// 5. Paste inserts text
// ═══════════════════════════════════════════════════════════════

#[test]
fn paste_inserts_text() {
	let mut h = SimseTestHarness::new();
	h.paste("pasted text");
	assert_eq!(h.input_value(), "pasted text");
	assert_eq!(h.app.input.cursor, 11);
}

// ═══════════════════════════════════════════════════════════════
// 6. History up/down navigation
// ═══════════════════════════════════════════════════════════════

#[test]
fn history_up_down() {
	let mut h = SimseTestHarness::new();

	// Submit two entries to build history.
	h.submit("first");
	assert!(h.app.history.contains(&"first".to_string()));
	h.submit("second");
	assert!(h.app.history.contains(&"second".to_string()));

	// Input should be clear after submitting.
	assert_eq!(h.input_value(), "");

	// Press up: should recall "second" (most recent).
	h.press_up();
	assert_eq!(h.input_value(), "second");

	// Press up again: should recall "first".
	h.press_up();
	assert_eq!(h.input_value(), "first");

	// Press down: should go back to "second".
	h.press_down();
	assert_eq!(h.input_value(), "second");

	// Press down again: should restore empty draft.
	h.press_down();
	assert_eq!(h.input_value(), "");
}

// ═══════════════════════════════════════════════════════════════
// 7. Ctrl+C behavior (pending then quit)
// ═══════════════════════════════════════════════════════════════

#[test]
fn ctrl_c_behavior() {
	let mut h = SimseTestHarness::new();
	assert!(!h.app.ctrl_c_pending);
	assert!(!h.should_quit());

	// First Ctrl+C: sets pending state.
	h.press_ctrl_c();
	assert!(h.app.ctrl_c_pending);
	assert!(!h.should_quit());

	// Second Ctrl+C: triggers quit.
	h.press_ctrl_c();
	assert!(h.should_quit());
}

// ═══════════════════════════════════════════════════════════════
// 8. Delete word back (Alt+Backspace)
// ═══════════════════════════════════════════════════════════════

#[test]
fn delete_word_back() {
	let mut h = SimseTestHarness::new();
	h.type_text("hello world");
	assert_eq!(h.input_value(), "hello world");

	// Alt+Backspace deletes the word "world", leaving "hello ".
	h.press_delete_word_back();
	assert_eq!(h.input_value(), "hello ");
	assert_eq!(h.app.input.cursor, 6);
}
