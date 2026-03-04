//! E2E tests: config commands (`/config`, `/settings`, `/init`, `/setup`,
//! `/factory-reset`).
//!
//! Config commands exercise three patterns:
//!
//! - **Info output** (`/config` with no loaded config) -- rendered to screen.
//! - **Overlay transitions** (`/settings`, `/setup`) -- change `app.screen`.
//! - **Bridge requests** (`/init`, `/factory-reset`) -- stored in
//!   `app.pending_bridge_action` for the event loop to dispatch.

use simse_tui::app::Screen;
use simse_tui::commands::BridgeAction;

use crate::harness::SimseTestHarness;

// ===================================================================
// 1. /config shows "No configuration loaded" when config_values is empty
// ===================================================================

#[test]
fn config_command_shows_no_config() {
	let mut h = SimseTestHarness::new();
	h.submit("/config");
	// Default CommandContext has an empty config_values vec, so the handler
	// returns CommandOutput::Info("No configuration loaded.").
	h.assert_contains("No configuration loaded");
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
// 5. /factory-reset creates a FactoryReset bridge action
// ===================================================================

#[test]
fn factory_reset_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /factory-reset"
	);

	h.submit("/factory-reset");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /factory-reset");

	assert_eq!(*action, BridgeAction::FactoryReset);
}
