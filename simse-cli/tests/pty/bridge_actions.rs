//! PTY tests for bridge action dispatch — verifying that commands go through
//! the bridge dispatch pipeline and produce visible results on screen.
//!
//! These tests cover the CRITICAL gap that caused the factory-reset bug:
//! commands must produce a BridgeAction, main.rs must pick it up from
//! `app.pending_bridge_action`, dispatch it through `TuiRuntime`, and
//! feed the `BridgeResult` back into `App::update`.
//!
//! Deduplication notes (tests already covered elsewhere):
//!   - factory_reset dispatches → commands_config.rs
//!   - factory_reset_project dispatches → config_settings.rs
//!   - init dispatches → commands_config.rs
//!   - model_switch dispatches → acp_flow.rs
//!   - factory_reset returns to initial state → onboarding.rs
//!
//! Truly new tests here:
//!   1. compact_dispatches_and_shows_result
//!   2. bridge_error_shows_on_screen
//!   3. multiple_actions_dispatch_sequentially
//!   4. factory_reset_full_cycle (reset → confirm → data deleted → can /setup)

use super::r#mod::*;
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════
// 1. /compact dispatches through bridge and shows result
// ═══════════════════════════════════════════════════════════════

#[test]
fn compact_dispatches_and_shows_result() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&work_dir).unwrap();
	write_default_config(&data_dir);

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	type_command(&mut h, "/compact");

	// The bridge action BridgeAction::Compact is dispatched. The runtime
	// compacts the (empty) conversation and returns a result like
	// "Conversation compacted (0 messages -> 1 summary)."
	// We just verify the word "compacted" appears anywhere on screen,
	// proving the bridge round-trip completed.
	h.wait_for_text("compacted")
		.expect("'/compact' should dispatch through bridge and show 'compacted' result");
}

// ═══════════════════════════════════════════════════════════════
// 2. Bridge errors are displayed on screen (not silently swallowed)
// ═══════════════════════════════════════════════════════════════

#[test]
fn bridge_error_shows_on_screen() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&work_dir).unwrap();
	write_default_config(&data_dir);

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	// Resume a nonexistent session. The bridge will try to load it from the
	// session_store, fail with "Session not found: nonexistent-session-xyz",
	// and return a BridgeResult with is_error=true. The app should display
	// this as an error (OutputItem::Error) on screen.
	type_command(&mut h, "/resume nonexistent-session-xyz");

	// First, the info message "Resuming session..." appears.
	h.wait_for_text("Resuming session")
		.expect("'/resume' should show 'Resuming session' feedback");

	// Then the bridge error should appear. The error text contains
	// "Session not found" or "not found".
	h.wait_for_text("not found")
		.expect("Bridge error for nonexistent session should be displayed on screen, not swallowed");

	// The app should remain running after a bridge error (not crash/panic).
	assert!(
		h.is_running(),
		"App should not crash on bridge error"
	);
}

// ═══════════════════════════════════════════════════════════════
// 3. Multiple bridge actions dispatch sequentially
// ═══════════════════════════════════════════════════════════════

#[test]
fn multiple_actions_dispatch_sequentially() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&work_dir).unwrap();

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	// First action: /init creates the .simse/ directory.
	type_command(&mut h, "/init");
	h.wait_for_text("Initialized")
		.expect("'/init' should show 'Initialized' after bridge dispatch");

	// Verify the directory was created.
	let simse_dir = work_dir.join(".simse");
	assert!(
		simse_dir.exists(),
		"Expected .simse/ directory to exist after /init: {}",
		simse_dir.display()
	);

	// Second action: /compact should also dispatch and complete.
	type_command(&mut h, "/compact");
	h.wait_for_text("compacted")
		.expect("'/compact' after '/init' should also dispatch and show result");
}

// ═══════════════════════════════════════════════════════════════
// 4. Factory reset full cycle: reset → confirm → deleted → /setup
// ═══════════════════════════════════════════════════════════════
//
// This is the MOST IMPORTANT test. It proves the entire factory-reset
// flow works end-to-end:
//   1. Start configured (acp.json + config.json exist)
//   2. /factory-reset → confirmation dialog
//   3. Enter → bridge dispatch → "Factory reset complete"
//   4. data_dir deleted
//   5. After reset, /setup still works (the app is usable)

#[test]
fn factory_reset_full_cycle() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&work_dir).unwrap();
	write_default_config(&data_dir);

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	// Step 1: Trigger factory reset.
	type_command(&mut h, "/factory-reset");

	h.wait_for_text("Are you sure")
		.expect("Factory reset should show confirmation dialog");

	// Step 2: Confirm.
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Factory reset complete")
		.expect("Should show 'Factory reset complete' after confirming");

	// Step 3: Verify data_dir was deleted.
	assert!(
		!data_dir.exists(),
		"Expected data_dir to be deleted after factory reset: {}",
		data_dir.display()
	);

	// Step 4: The app should still be running and usable.
	assert!(h.is_running(), "App should still be running after factory reset");

	// Step 5: After factory reset, we can still open /setup — the app
	// hasn't crashed and the UI is responsive.
	type_command(&mut h, "/setup");

	h.wait_for_text("Setup")
		.expect("'/setup' should work after factory reset, proving the app is still functional");
}
