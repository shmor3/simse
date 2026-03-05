//! PTY tests for file commands (`/files`, `/save`, `/validate`, `/discard`, `/diff`).
//!
//! Each command should show immediate feedback text before dispatching to the bridge.

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /files shows "Listing files..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn files_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/files src");
	settle();

	h.wait_for_text("Listing files")
		.expect("'/files src' should show 'Listing files' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 2. /save shows "Saving to: output.txt"
// ═══════════════════════════════════════════════════════════════

#[test]
fn save_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/save output.txt");
	settle();

	h.wait_for_text("Saving to")
		.expect("'/save output.txt' should show 'Saving to' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 3. /validate shows "Validating files..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn validate_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/validate");
	settle();

	h.wait_for_text("Validating files")
		.expect("'/validate' should show 'Validating files' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 4. /discard shows "Discarding changes to: temp.rs"
// ═══════════════════════════════════════════════════════════════

#[test]
fn discard_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/discard temp.rs");
	settle();

	h.wait_for_text("Discarding changes to")
		.expect("'/discard temp.rs' should show 'Discarding changes to' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 5. /diff shows "Generating diff..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn diff_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/diff lib.rs");
	settle();

	h.wait_for_text("Generating diff")
		.expect("'/diff lib.rs' should show 'Generating diff' feedback");
}
