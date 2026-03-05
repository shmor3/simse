//! E2E tests: file commands (`/files`, `/save`, `/validate`, `/discard`, `/diff`).
//!
//! File commands produce `BridgeRequest(BridgeAction::*)` items that are stored
//! in `app.pending_bridge_action` for the event loop to dispatch asynchronously
//! via the bridge.  Each also emits an Info feedback message that should appear
//! on screen.

use simse_tui::commands::BridgeAction;

use crate::harness::SimseTestHarness;

// ===================================================================
// 1. /files <path> creates a ListFiles bridge action
// ===================================================================

#[test]
fn files_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /files"
	);

	h.submit("/files src");

	// Verify feedback message appears on screen.
	h.assert_contains("Listing files...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /files");

	assert_eq!(
		*action,
		BridgeAction::ListFiles {
			path: Some("src".into()),
		}
	);
}

// ===================================================================
// 2. /save <path> creates a SaveFiles bridge action
// ===================================================================

#[test]
fn save_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /save"
	);

	h.submit("/save output.txt");

	// Verify feedback message appears on screen (includes target path).
	h.assert_contains("Saving to: output.txt");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /save");

	assert_eq!(
		*action,
		BridgeAction::SaveFiles {
			path: Some("output.txt".into()),
		}
	);
}

// ===================================================================
// 3. /validate creates a ValidateFiles bridge action
// ===================================================================

#[test]
fn validate_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /validate"
	);

	h.submit("/validate");

	// Verify feedback message appears on screen.
	h.assert_contains("Validating files...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /validate");

	assert_eq!(
		*action,
		BridgeAction::ValidateFiles { path: None }
	);
}

// ===================================================================
// 4. /discard <path> creates a DiscardFile bridge action
// ===================================================================

#[test]
fn discard_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /discard"
	);

	h.submit("/discard temp.rs");

	// Verify feedback message appears on screen (includes target path).
	h.assert_contains("Discarding changes to: temp.rs");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /discard");

	assert_eq!(
		*action,
		BridgeAction::DiscardFile {
			path: "temp.rs".into(),
		}
	);
}

// ===================================================================
// 5. /diff <path> creates a DiffFiles bridge action
// ===================================================================

#[test]
fn diff_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /diff"
	);

	h.submit("/diff lib.rs");

	// Verify feedback message appears on screen.
	h.assert_contains("Generating diff...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /diff");

	assert_eq!(
		*action,
		BridgeAction::DiffFiles {
			path: Some("lib.rs".into()),
		}
	);
}
