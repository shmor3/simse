//! E2E tests: startup behavior.
//!
//! Verifies that the initial render (banner, input, status bar) contains the
//! expected text.  All assertions are based on the actual `view()` output
//! produced by `App::new()` with ratatui's `TestBackend`.

use crate::harness::SimseTestHarness;

#[test]
fn startup_shows_banner() {
	let harness = SimseTestHarness::new();
	// The banner left column renders "simse v<version>" on startup.
	harness.assert_contains("simse v");
}

#[test]
fn startup_shows_input_prompt() {
	let harness = SimseTestHarness::new();
	// The input block has the title "Input".
	harness.assert_contains("Input");
}

#[test]
fn startup_shows_status_bar() {
	let harness = SimseTestHarness::new();
	// Status bar shows the default permission mode "ask" with its cycle hint.
	harness.assert_contains("ask (shift+tab)");
}

#[test]
fn startup_shows_version() {
	let harness = SimseTestHarness::new();
	let screen = harness.screen_text();
	// The banner renders "simse v0.1.0" — check for the workspace version.
	assert!(
		screen.contains("v0.1.0") || screen.contains("v0."),
		"Expected version string on screen: {}",
		screen,
	);
}

#[test]
fn startup_shows_tips() {
	let harness = SimseTestHarness::new();
	// The banner right column renders a "Tips" header and several hints
	// including "/help" and "? for shortcuts" in the status bar.
	harness.assert_contains("Tips");
	harness.assert_contains("/help");
}
