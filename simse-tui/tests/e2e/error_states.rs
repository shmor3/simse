//! E2E error-state tests for simse-tui.
//!
//! Covers unknown commands, empty submit, bare slash, and error rendering.

use crate::harness::SimseTestHarness;

// ═══════════════════════════════════════════════════════════════
// 1. Unknown command shows error
// ═══════════════════════════════════════════════════════════════

#[test]
fn unknown_command_shows_error() {
	let mut h = SimseTestHarness::new();
	h.submit("/foobar");

	// The dispatch produces CommandOutput::Error("Unknown command: /foobar")
	// which gets pushed as OutputItem::Error { message } into app.output.
	assert!(
		h.app
			.output
			.iter()
			.any(|o| matches!(o, simse_ui_core::app::OutputItem::Error { message } if message.contains("Unknown command: /foobar"))),
		"Expected an error output containing 'Unknown command: /foobar', got: {:?}",
		h.app.output,
	);
}

// ═══════════════════════════════════════════════════════════════
// 2. Empty submit is a no-op
// ═══════════════════════════════════════════════════════════════

#[test]
fn empty_submit_is_noop() {
	let mut h = SimseTestHarness::new();
	let output_before = h.app.output.len();

	// Press Enter with empty input.
	h.press_enter();

	// No output should be added and the app should not crash.
	assert_eq!(
		h.app.output.len(),
		output_before,
		"Empty submit should not add any output",
	);
	assert!(!h.should_quit(), "Empty submit should not quit the app");
	assert_eq!(h.input_value(), "", "Input should remain empty");
}

// ═══════════════════════════════════════════════════════════════
// 3. Slash only shows error or no-op
// ═══════════════════════════════════════════════════════════════

#[test]
fn slash_only_shows_error_or_noop() {
	let mut h = SimseTestHarness::new();
	h.submit("/");

	// "/" starts with '/' so dispatch_command is called.
	// parse_command_line("/") returns None → pushes OutputItem::Error("Invalid command.").
	assert!(
		h.app
			.output
			.iter()
			.any(|o| matches!(o, simse_ui_core::app::OutputItem::Error { message } if message.contains("Invalid command"))),
		"Expected an error output containing 'Invalid command', got: {:?}",
		h.app.output,
	);

	// App should not crash or quit.
	assert!(!h.should_quit(), "Bare '/' should not quit the app");
}

// ═══════════════════════════════════════════════════════════════
// 4. Error output renders on screen
// ═══════════════════════════════════════════════════════════════

#[test]
fn error_output_renders_on_screen() {
	let mut h = SimseTestHarness::new();
	h.submit("/foobar");

	// The error message should appear in the rendered terminal output,
	// not just in app.output. The renderer prefixes errors with "✗ ".
	let screen = h.screen_text();
	assert!(
		screen.contains("Unknown command: /foobar"),
		"Expected 'Unknown command: /foobar' to appear on the rendered screen, but screen was:\n{}",
		screen,
	);
}
