//! PTY tests for meta commands (`/help`, `/clear`, `/exit`, `/verbose`, `/plan`,
//! `/context`, `/compact`, `/shortcuts`).
//!
//! These tests verify observable screen output through the real binary.

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /help shows command categories
// ═══════════════════════════════════════════════════════════════

#[test]
fn help_shows_commands() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/help");
	settle();

	h.wait_for_text("Meta")
		.expect("'/help' should show 'Meta' category heading");

	let contents = h.screen_contents();
	assert!(
		contents.contains("/help"),
		"'/help' output should include '/help' command. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 2. /help search shows specific command info
// ═══════════════════════════════════════════════════════════════

#[test]
fn help_specific_command() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/help search");
	settle();

	h.wait_for_text("/search")
		.expect("'/help search' should show '/search' command details");
}

// ═══════════════════════════════════════════════════════════════
// 3. /clear re-shows the banner
// ═══════════════════════════════════════════════════════════════

#[test]
fn clear_clears_output() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	// Submit some text so there is content on screen.
	type_command(&mut h, "Hello from user");
	settle();

	h.wait_for_text("Hello from user")
		.expect("User message should appear on screen");

	// Now /clear should reset the screen and re-show the banner.
	type_command(&mut h, "/clear");
	settle();

	h.wait_for_text("simse v")
		.expect("After '/clear', the banner ('simse v') should reappear");
}

// ═══════════════════════════════════════════════════════════════
// 4. /exit quits the app (process terminates)
// ═══════════════════════════════════════════════════════════════

#[test]
fn exit_quits_app() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/exit");

	// Give the process time to exit.
	std::thread::sleep(Duration::from_millis(1500));

	assert!(
		!h.is_running(),
		"'/exit' should cause the process to terminate"
	);
}

// ═══════════════════════════════════════════════════════════════
// 5. /verbose toggles verbose mode on and off
// ═══════════════════════════════════════════════════════════════

#[test]
fn verbose_toggle() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	// First toggle: off -> on
	type_command(&mut h, "/verbose");
	settle();

	h.wait_for_text("Verbose mode on")
		.expect("First '/verbose' should show 'Verbose mode on'");

	// Second toggle: on -> off
	type_command(&mut h, "/verbose");
	settle();

	h.wait_for_text("Verbose mode off")
		.expect("Second '/verbose' should show 'Verbose mode off'");
}

// ═══════════════════════════════════════════════════════════════
// 6. /plan toggles plan mode on and off
// ═══════════════════════════════════════════════════════════════

#[test]
fn plan_toggle() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	// First toggle: off -> on
	type_command(&mut h, "/plan");
	settle();

	h.wait_for_text("Plan mode on")
		.expect("First '/plan' should show 'Plan mode on'");

	// Second toggle: on -> off
	type_command(&mut h, "/plan");
	settle();

	h.wait_for_text("Plan mode off")
		.expect("Second '/plan' should show 'Plan mode off'");
}

// ═══════════════════════════════════════════════════════════════
// 7. /context shows token count and percentage
// ═══════════════════════════════════════════════════════════════

#[test]
fn context_shows_stats() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/context");
	settle();

	h.wait_for_text("Tokens:")
		.expect("'/context' should show 'Tokens:' in output");

	let contents = h.screen_contents();
	assert!(
		contents.contains("0%"),
		"'/context' should show '0%' for a fresh session. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 8. /compact shows "Compacting conversation"
// ═══════════════════════════════════════════════════════════════

#[test]
fn compact_shows_feedback() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/compact");
	settle();

	h.wait_for_text("Compacting conversation")
		.expect("'/compact' should show 'Compacting conversation' feedback");
}

// ═══════════════════════════════════════════════════════════════
// 9. /shortcuts opens the Keyboard Shortcuts overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn shortcuts_opens_overlay() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/shortcuts");
	settle();

	h.wait_for_text("Keyboard Shortcuts")
		.expect("'/shortcuts' should open the Keyboard Shortcuts overlay");
}
