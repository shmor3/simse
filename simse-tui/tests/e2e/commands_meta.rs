//! E2E tests: meta commands (`/help`, `/clear`, `/exit`, `/verbose`, `/plan`,
//! `/context`, `/compact`, `/shortcuts`).
//!
//! These tests exercise the full dispatch pipeline through the App model's
//! `update()` → `dispatch_command()` → `view()` cycle, verifying that each meta
//! command produces the expected visible output and/or state changes.

use simse_tui::app::Screen;

use crate::harness::SimseTestHarness;

// ═══════════════════════════════════════════════════════════════
// 1. /help shows available commands
// ═══════════════════════════════════════════════════════════════

#[test]
fn help_command_shows_available_commands() {
	let mut h = SimseTestHarness::new();
	h.submit("/help");
	// The help output lists commands grouped by category.  The output is long
	// and the terminal is only 30 rows, so "Available commands" (at the top)
	// is scrolled off.  Verify by checking that a category header and some
	// command entries are visible.
	h.assert_contains("Meta:");
	h.assert_contains("/help");
}

// ═══════════════════════════════════════════════════════════════
// 2. /help search shows the search command description
// ═══════════════════════════════════════════════════════════════

#[test]
fn help_specific_command() {
	let mut h = SimseTestHarness::new();
	h.submit("/help search");
	// The help handler returns "/<name> -- <desc>\n  Usage: /<usage>\n"
	h.assert_contains("/search");
}

// ═══════════════════════════════════════════════════════════════
// 3. /clear clears output
// ═══════════════════════════════════════════════════════════════

#[test]
fn clear_command_clears_output() {
	let mut h = SimseTestHarness::new();

	// Submit something first so there is output.
	h.submit("hello from user");
	assert!(
		!h.app.output.is_empty(),
		"Expected output after submitting user text"
	);

	// Now clear.
	h.submit("/clear");
	assert!(
		h.app.output.is_empty(),
		"Expected output to be empty after /clear"
	);
	assert!(
		h.app.banner_visible,
		"Expected banner_visible to be true after /clear"
	);
}

// ═══════════════════════════════════════════════════════════════
// 4. /exit sets should_quit
// ═══════════════════════════════════════════════════════════════

#[test]
fn exit_command_quits() {
	let mut h = SimseTestHarness::new();
	assert!(!h.should_quit());
	h.submit("/exit");
	assert!(h.should_quit());
}

// ═══════════════════════════════════════════════════════════════
// 5. /verbose toggles verbose mode
// ═══════════════════════════════════════════════════════════════

#[test]
fn verbose_toggle() {
	let mut h = SimseTestHarness::new();
	assert!(!h.app.verbose, "verbose should start off");

	// Toggle on.
	h.submit("/verbose");
	assert!(h.app.verbose, "verbose should be on after toggle");
	h.assert_contains("Verbose mode on");

	// Toggle off.
	h.submit("/verbose");
	assert!(!h.app.verbose, "verbose should be off after second toggle");
	h.assert_contains("Verbose mode off");
}

// ═══════════════════════════════════════════════════════════════
// 6. /plan toggles plan mode
// ═══════════════════════════════════════════════════════════════

#[test]
fn plan_toggle() {
	let mut h = SimseTestHarness::new();
	assert!(!h.app.plan_mode, "plan_mode should start off");

	// Toggle on.
	h.submit("/plan");
	assert!(h.app.plan_mode, "plan_mode should be on after toggle");
	h.assert_contains("Plan mode on");

	// Toggle off.
	h.submit("/plan");
	assert!(!h.app.plan_mode, "plan_mode should be off after second toggle");
	h.assert_contains("Plan mode off");
}

// ═══════════════════════════════════════════════════════════════
// 7. /context shows token count info
// ═══════════════════════════════════════════════════════════════

#[test]
fn context_command_shows_stats() {
	let mut h = SimseTestHarness::new();
	// Default state has total_tokens = 0, context_percent = 0.
	h.submit("/context");
	// The handler returns "Tokens: 0 | Context: 0%".
	h.assert_contains("Tokens:");
	h.assert_contains("0%");
}

// ═══════════════════════════════════════════════════════════════
// 8. /compact produces a bridge action with feedback
// ═══════════════════════════════════════════════════════════════

#[test]
fn compact_command() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /compact"
	);

	h.submit("/compact");

	// Verify feedback message appears on screen.
	h.assert_contains("Compacting conversation history...");

	// /compact returns BridgeRequest(Compact), which is stored as a pending
	// bridge action for the event loop to pick up.
	assert!(
		h.app.pending_bridge_action.is_some(),
		"Expected a pending bridge action after /compact"
	);
}

// ═══════════════════════════════════════════════════════════════
// 9. /shortcuts opens the shortcuts overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn shortcuts_command_opens_overlay() {
	let mut h = SimseTestHarness::new();
	assert_eq!(
		h.current_screen(),
		&Screen::Chat,
		"Should start on Chat screen"
	);

	h.submit("/shortcuts");
	assert_eq!(
		h.current_screen(),
		&Screen::Shortcuts,
		"Expected Shortcuts screen after /shortcuts command"
	);
}
