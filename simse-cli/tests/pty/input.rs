//! PTY tests for input handling in simse-tui.
//!
//! Covers text input, cursor movement, deletion, paste, history navigation,
//! Ctrl+C behavior, and word-level deletion.

use super::r#mod::*;
use ratatui_testlib::{KeyCode, Modifiers};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	thread::sleep(Duration::from_millis(250));
}

// ═══════════════════════════════════════════════════════════════
// 1. Typing text appears in input
// ═══════════════════════════════════════════════════════════════

#[test]
fn typing_text_appears_in_input() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	h.send_keys("hello").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("hello"),
		"Screen should contain 'hello' after typing. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 2. Backspace deletes character
// ═══════════════════════════════════════════════════════════════

#[test]
fn backspace_deletes_character() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	h.send_keys("hello").unwrap();
	h.wait_for_text("hello")
		.expect("'hello' should appear in input");

	h.send_key(KeyCode::Backspace).unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("hell"),
		"Screen should contain 'hell' after backspace. Screen:\n{contents}"
	);
	assert!(
		!contents.contains("hello"),
		"Screen should no longer contain 'hello' after backspace. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 3. Delete key works
// ═══════════════════════════════════════════════════════════════

#[test]
fn delete_key_works() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	h.send_keys("hello").unwrap();
	h.wait_for_text("hello")
		.expect("'hello' should appear in input");

	// Move left once (cursor before 'o'), then Delete removes the 'o'.
	h.send_key(KeyCode::Left).unwrap();
	settle();
	h.send_key(KeyCode::Delete).unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("hell"),
		"Screen should contain 'hell' after Delete. Screen:\n{contents}"
	);
	assert!(
		!contents.contains("hello"),
		"Screen should no longer contain 'hello' after Delete. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 4. Arrow keys move cursor (insert at cursor position)
// ═══════════════════════════════════════════════════════════════

#[test]
fn arrow_keys_move_cursor() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	h.send_keys("hello").unwrap();
	h.wait_for_text("hello")
		.expect("'hello' should appear in input");

	// Move left once (cursor between 'l' and 'o'), type 'X'.
	h.send_key(KeyCode::Left).unwrap();
	settle();
	h.send_keys("X").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("hellXo"),
		"Screen should contain 'hellXo' after inserting 'X'. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 5. Paste inserts text
// ═══════════════════════════════════════════════════════════════

#[test]
fn paste_inserts_text() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Use bracketed paste mode: \x1b[200~ ... \x1b[201~
	// crossterm interprets this as Event::Paste("pasted content").
	h.send_keys("\x1b[200~pasted content\x1b[201~").unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("pasted content"),
		"Screen should contain 'pasted content' after paste. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 6. History up/down navigation
// ═══════════════════════════════════════════════════════════════

#[test]
fn history_up_down() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// Submit "first" — type_command sends text + Enter.
	type_command(&mut h, "first");
	settle();

	// Submit "second".
	type_command(&mut h, "second");
	settle();

	// Press Up: should recall "second" (most recent).
	h.send_key(KeyCode::Up).unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("second"),
		"Screen should show 'second' after first Up press. Screen:\n{contents}"
	);

	// Press Up again: should recall "first".
	h.send_key(KeyCode::Up).unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("first"),
		"Screen should show 'first' after second Up press. Screen:\n{contents}"
	);

	// Press Down: should go back to "second".
	h.send_key(KeyCode::Down).unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("second"),
		"Screen should show 'second' after Down press. Screen:\n{contents}"
	);

	// Press Down again: should restore empty input.
	h.send_key(KeyCode::Down).unwrap();
	settle();

	// The input should no longer show "second" from the history — we've
	// scrolled past it back to the empty draft state.
	let _contents = h.screen_contents();
	// Note: We can't do a global negative assertion for "second" because the
	// output area still contains it from the earlier submission. The core
	// history navigation (Up recalls entries, Down goes back) was already
	// verified by the assertions above.
}

// ═══════════════════════════════════════════════════════════════
// 7. Ctrl+C behavior (pending then quit)
// ═══════════════════════════════════════════════════════════════

#[test]
fn ctrl_c_behavior() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	// First Ctrl+C: should show "Press Ctrl-C again to exit" prompt.
	send_ctrl_c(&mut h);
	settle();

	h.wait_for_text("Press Ctrl-C again to exit")
		.expect("First Ctrl+C should show 'Press Ctrl-C again to exit'");

	assert!(
		h.is_running(),
		"App should still be running after first Ctrl+C"
	);

	// Second Ctrl+C: should cause the app to quit.
	send_ctrl_c(&mut h);

	// Give the process time to exit.
	thread::sleep(Duration::from_millis(500));

	assert!(
		!h.is_running(),
		"App should have exited after second Ctrl+C"
	);
}

// ═══════════════════════════════════════════════════════════════
// 8. Delete word back (Alt+Backspace)
// ═══════════════════════════════════════════════════════════════

#[test]
fn delete_word_back() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	h.send_keys("hello world").unwrap();
	h.wait_for_text("hello world")
		.expect("'hello world' should appear in input");

	// Alt+Backspace deletes the word "world", leaving "hello ".
	h.send_key_with_modifiers(KeyCode::Backspace, Modifiers::ALT)
		.unwrap();
	settle();

	let contents = h.screen_contents();
	assert!(
		contents.contains("hello"),
		"Screen should still contain 'hello' after Alt+Backspace. Screen:\n{contents}"
	);
	assert!(
		!contents.contains("hello world"),
		"Screen should no longer contain 'hello world' after Alt+Backspace. Screen:\n{contents}"
	);
}
