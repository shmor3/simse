//! PTY tests for library commands (`/add`, `/recommend`, `/topics`, `/volumes`,
//! `/get`, `/delete`).
//!
//! Deduplication notes:
//!   - `/search` feedback -- already covered in command_feedback.rs.
//!   - `/librarians` overlay -- already covered in overlays.rs.
//!
//! Remaining tests here (6 after dedup):
//!   1. `/add topic some text` -- shows "Adding to library"
//!   2. `/recommend patterns` -- shows "Getting recommendations"
//!   3. `/topics` -- shows "Listing library topics"
//!   4. `/volumes rust` -- shows "Listing library volumes"
//!   5. `/get id-42` -- shows "Retrieving volume"
//!   6. `/delete id-99` -- shows "Deleting volume"

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /add shows "Adding to library..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn add_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/add topic some text");
	settle();

	h.wait_for_text("Adding to library")
		.expect("'/add topic some text' should show 'Adding to library' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 2. /recommend shows "Getting recommendations..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn recommend_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/recommend patterns");
	settle();

	h.wait_for_text("Getting recommendations")
		.expect("'/recommend patterns' should show 'Getting recommendations' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 3. /topics shows "Listing library topics..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn topics_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/topics");
	settle();

	h.wait_for_text("Listing library topics")
		.expect("'/topics' should show 'Listing library topics' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 4. /volumes shows "Listing library volumes..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn volumes_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/volumes rust");
	settle();

	h.wait_for_text("Listing library volumes")
		.expect("'/volumes rust' should show 'Listing library volumes' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 5. /get shows "Retrieving volume..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn get_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/get id-42");
	settle();

	h.wait_for_text("Retrieving volume")
		.expect("'/get id-42' should show 'Retrieving volume' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 6. /delete shows "Deleting volume..."
// ═══════════════════════════════════════════════════════════════

#[test]
fn delete_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/delete id-99");
	settle();

	h.wait_for_text("Deleting volume")
		.expect("'/delete id-99' should show 'Deleting volume' feedback");
}
