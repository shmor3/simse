//! E2E tests: config commands (`/config`, `/settings`, `/init`, `/setup`,
//! `/factory-reset`).
//!
//! Config commands exercise three patterns:
//!
//! - **Info output** (`/config` with no loaded config) -- rendered to screen
//!   with actionable guidance.
//! - **Overlay transitions** (`/settings`, `/setup`) -- change `app.screen`.
//! - **Bridge requests** (`/init`) -- stored in `app.pending_bridge_action`
//!   with a feedback message on screen.
//! - **Confirmation dialog** (`/factory-reset`) -- opens a confirm screen
//!   before creating a bridge action.

use simse_tui::app::Screen;
use simse_tui::commands::BridgeAction;

use crate::harness::SimseTestHarness;

// ===================================================================
// 1. /config shows actionable guidance when config_values is empty
// ===================================================================

#[test]
fn config_command_shows_no_config() {
	let mut h = SimseTestHarness::new();
	h.submit("/config");
	// Default CommandContext has an empty config_values vec, so the handler
	// returns an Info message with actionable guidance.
	h.assert_contains("No configuration loaded");
	h.assert_contains("/init");
}

// ===================================================================
// 2. /settings opens the Settings overlay
// ===================================================================

#[test]
fn settings_command_opens_overlay() {
	let mut h = SimseTestHarness::new();
	// Starts on Chat screen.
	assert_eq!(
		*h.current_screen(),
		Screen::Chat,
		"Should start on Chat screen"
	);

	h.submit("/settings");

	assert_eq!(
		*h.current_screen(),
		Screen::Settings,
		"Screen should be Settings after /settings command"
	);
}

// ===================================================================
// 3. /init creates an InitConfig bridge action
// ===================================================================

#[test]
fn init_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /init"
	);

	h.submit("/init");

	// Verify feedback message appears on screen.
	h.assert_contains("Initializing project configuration...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /init");

	assert_eq!(
		*action,
		BridgeAction::InitConfig { force: false }
	);
}

// ===================================================================
// 4. /setup opens the Setup overlay
// ===================================================================

#[test]
fn setup_command_opens_overlay() {
	let mut h = SimseTestHarness::new();
	// Starts on Chat screen.
	assert_eq!(
		*h.current_screen(),
		Screen::Chat,
		"Should start on Chat screen"
	);

	h.submit("/setup");

	assert!(
		matches!(*h.current_screen(), Screen::Setup { preset: None }),
		"Screen should be Setup {{ preset: None }} after /setup command, got: {:?}",
		h.current_screen()
	);
}

// ===================================================================
// 5. /factory-reset opens confirmation dialog, then creates bridge action
// ===================================================================

#[test]
fn factory_reset_opens_confirm_dialog() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /factory-reset"
	);

	h.submit("/factory-reset");

	// Should be on the Confirm screen, not yet a bridge action.
	assert!(
		matches!(&h.app.screen, Screen::Confirm { .. }),
		"Expected Screen::Confirm after /factory-reset, got: {:?}",
		h.app.screen
	);

	// Verify the confirmation message appears on screen.
	h.assert_contains("Are you sure");

	assert!(
		h.app.pending_bridge_action.is_none(),
		"Should NOT have a pending bridge action before confirming"
	);
	assert!(
		h.app.pending_confirm_action.is_some(),
		"Should have a pending confirm action"
	);
}

#[test]
fn factory_reset_confirm_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	h.submit("/factory-reset");

	// Confirm by pressing Enter.
	h.press_enter();

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after confirming /factory-reset");

	assert_eq!(*action, BridgeAction::FactoryReset);
	assert_eq!(
		h.app.screen,
		Screen::Chat,
		"Should return to Chat after confirming"
	);
}

#[test]
fn factory_reset_escape_cancels() {
	let mut h = SimseTestHarness::new();
	h.submit("/factory-reset");

	// Cancel by pressing Escape.
	h.press_escape();

	assert!(
		h.app.pending_bridge_action.is_none(),
		"Should NOT have a pending bridge action after cancelling"
	);
	assert!(
		h.app.pending_confirm_action.is_none(),
		"Confirm action should be cleared after cancelling"
	);
	assert_eq!(
		h.app.screen,
		Screen::Chat,
		"Should return to Chat after cancelling"
	);
}
