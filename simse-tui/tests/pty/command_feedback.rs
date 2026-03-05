//! PTY tests for command feedback, error messages, and suggestions.
//!
//! These tests verify observable command feedback through the real binary:
//!
//! - **Search feedback** — `/search test query` shows "Searching library for: test query".
//! - **Typo suggestion** — `/sarch test` suggests "/search".
//! - **Missing args** — `/add` with no args shows "Usage:".
//! - **Empty sessions** — `/sessions` with no sessions shows guidance text.
//! - **Cancel confirmation** — `/factory-reset` → Escape shows "Cancelled".

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /search shows feedback with the query
// ═══════════════════════════════════════════════════════════════

#[test]
fn search_command_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/search test query");

	h.wait_for_text("Searching library for")
		.expect("'/search test query' should show 'Searching library for' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 2. Typo command suggests similar
// ═══════════════════════════════════════════════════════════════

#[test]
fn unknown_command_suggests_similar() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/sarch test");

	h.wait_for_text("Did you mean")
		.expect("'/sarch' (typo) should show 'Did you mean' suggestion");
}

// ═══════════════════════════════════════════════════════════════
// 3. Missing args shows usage
// ═══════════════════════════════════════════════════════════════

#[test]
fn missing_args_shows_usage() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/add");

	h.wait_for_text("Usage:")
		.expect("'/add' with no args should show 'Usage:' guidance");
}

// ═══════════════════════════════════════════════════════════════
// 4. /sessions with no sessions shows guidance
// ═══════════════════════════════════════════════════════════════

#[test]
fn empty_sessions_shows_guidance() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/sessions");

	h.wait_for_text("No saved sessions")
		.expect("'/sessions' with no sessions should show 'No saved sessions' guidance");
}

// ═══════════════════════════════════════════════════════════════
// 5. /factory-reset → Escape shows "Cancelled"
// ═══════════════════════════════════════════════════════════════

#[test]
fn cancel_confirm_shows_cancelled() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/factory-reset");

	h.wait_for_text("Are you sure")
		.expect("Confirmation dialog should appear");

	// Cancel by pressing Escape.
	send_escape(&mut h);
	settle();

	h.wait_for_text("Cancelled")
		.expect("Cancelling the confirmation should show 'Cancelled'");
}
