//! E2E tests: library commands (`/add`, `/search`, `/recommend`, `/topics`,
//! `/volumes`, `/get`, `/delete`).
//!
//! Library commands produce `BridgeRequest(BridgeAction::Library*)` items that
//! are stored in `app.pending_bridge_action` for the event loop to dispatch
//! asynchronously via the bridge.  Each also emits an Info feedback message
//! that should appear on screen.

use simse_tui::commands::BridgeAction;

use crate::harness::SimseTestHarness;

// ===================================================================
// 1. /add <topic> <text> creates a LibraryAdd bridge action
// ===================================================================

#[test]
fn add_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /add"
	);

	h.submit("/add topic some text");

	// Verify feedback message appears on screen.
	h.assert_contains("Adding to library...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /add");

	assert_eq!(
		*action,
		BridgeAction::LibraryAdd {
			topic: "topic".into(),
			text: "some text".into(),
		}
	);
}

// ===================================================================
// 2. /search <query> creates a LibrarySearch bridge action
// ===================================================================

#[test]
fn search_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /search"
	);

	h.submit("/search query");

	// Verify feedback message appears on screen.
	h.assert_contains("Searching library for: query");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /search");

	assert_eq!(
		*action,
		BridgeAction::LibrarySearch {
			query: "query".into(),
		}
	);
}

// ===================================================================
// 3. /recommend <query> creates a LibraryRecommend bridge action
// ===================================================================

#[test]
fn recommend_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /recommend"
	);

	h.submit("/recommend patterns");

	// Verify feedback message appears on screen.
	h.assert_contains("Getting recommendations...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /recommend");

	assert_eq!(
		*action,
		BridgeAction::LibraryRecommend {
			query: "patterns".into(),
		}
	);
}

// ===================================================================
// 4. /topics creates a LibraryTopics bridge action
// ===================================================================

#[test]
fn topics_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /topics"
	);

	h.submit("/topics");

	// Verify feedback message appears on screen.
	h.assert_contains("Listing library topics...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /topics");

	assert_eq!(*action, BridgeAction::LibraryTopics);
}

// ===================================================================
// 5. /volumes <topic> creates a LibraryVolumes bridge action
// ===================================================================

#[test]
fn volumes_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /volumes"
	);

	h.submit("/volumes rust");

	// Verify feedback message appears on screen.
	h.assert_contains("Listing library volumes...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /volumes");

	assert_eq!(
		*action,
		BridgeAction::LibraryVolumes {
			topic: Some("rust".into()),
		}
	);
}

// ===================================================================
// 6. /get <id> creates a LibraryGet bridge action
// ===================================================================

#[test]
fn get_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /get"
	);

	h.submit("/get id-42");

	// Verify feedback message appears on screen.
	h.assert_contains("Retrieving volume...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /get");

	assert_eq!(
		*action,
		BridgeAction::LibraryGet {
			id: "id-42".into(),
		}
	);
}

// ===================================================================
// 7. /delete <id> creates a LibraryDelete bridge action
// ===================================================================

#[test]
fn delete_command_creates_bridge_action() {
	let mut h = SimseTestHarness::new();
	assert!(
		h.app.pending_bridge_action.is_none(),
		"No pending action before /delete"
	);

	h.submit("/delete id-99");

	// Verify feedback message appears on screen.
	h.assert_contains("Deleting volume...");

	let action = h
		.app
		.pending_bridge_action
		.as_ref()
		.expect("Expected a pending bridge action after /delete");

	assert_eq!(
		*action,
		BridgeAction::LibraryDelete {
			id: "id-99".into(),
		}
	);
}
