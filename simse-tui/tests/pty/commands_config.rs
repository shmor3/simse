//! PTY tests for config commands (`/config`, `/settings`, `/init`, `/setup`,
//! `/factory-reset`).
//!
//! These tests verify OBSERVABLE BEHAVIOR through the real binary:
//!
//! - **Info output** (`/config` with no loaded config) — rendered text on screen.
//! - **Overlay transitions** (`/settings`, `/setup`) — overlay title text appears.
//! - **Bridge dispatch** (`/init`) — filesystem side effects (`.simse/` created).
//! - **Confirmation dialog** (`/factory-reset`) — confirm overlay text, then
//!   bridge dispatch on Enter, or cancel on Escape.

use super::r#mod::*;
use portable_pty::CommandBuilder;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

/// Spawn simse-tui with an explicit working directory (for `/init` tests).
fn spawn_simse_with_cwd(data_dir: &std::path::Path, work_dir: &std::path::Path) -> PtyHarness {
	let binary = env!("CARGO_BIN_EXE_simse-tui");
	let mut cmd = CommandBuilder::new(binary);
	cmd.arg("--data-dir");
	cmd.arg(data_dir.to_str().expect("data_dir must be valid UTF-8"));
	cmd.cwd(work_dir);
	PtyHarness::spawn(cmd, 120, 40, Duration::from_secs(15))
}

// ═══════════════════════════════════════════════════════════════
// 1. /config shows actionable guidance when no config is loaded
// ═══════════════════════════════════════════════════════════════

#[test]
fn config_command_shows_no_config() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/config");

	h.wait_for_text("No configuration loaded")
		.expect("'/config' with empty config should show 'No configuration loaded'");
}

// ═══════════════════════════════════════════════════════════════
// 2. /settings opens the Settings overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn settings_command_opens_overlay() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/settings");

	h.wait_for_text("Settings")
		.expect("'/settings' should open the Settings overlay");
}

// ═══════════════════════════════════════════════════════════════
// 3. /init creates the .simse/ project directory
// ═══════════════════════════════════════════════════════════════

#[test]
fn init_command_creates_project_directory() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&work_dir).unwrap();

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	type_command(&mut h, "/init");

	h.wait_for_text("Initialized")
		.expect("'/init' should show 'Initialized' after creating .simse/ directory");

	// Verify the .simse/ directory was actually created in the work_dir.
	let simse_dir = work_dir.join(".simse");
	assert!(
		simse_dir.exists(),
		"Expected .simse/ directory to be created at {}",
		simse_dir.display()
	);
}

// ═══════════════════════════════════════════════════════════════
// 4. /setup opens the Setup overlay
// ═══════════════════════════════════════════════════════════════

#[test]
fn setup_command_opens_overlay() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/setup");

	h.wait_for_text("Setup")
		.expect("'/setup' should open the Setup overlay");
}

// ═══════════════════════════════════════════════════════════════
// 5. /factory-reset opens confirmation dialog
// ═══════════════════════════════════════════════════════════════

#[test]
fn factory_reset_opens_confirm_dialog() {
	let tmp = TempDir::new().unwrap();
	let mut h = spawn_simse_configured(tmp.path());
	wait_for_startup(&h);

	type_command(&mut h, "/factory-reset");

	h.wait_for_text("Are you sure")
		.expect("'/factory-reset' should show confirmation dialog with 'Are you sure'");
}

// ═══════════════════════════════════════════════════════════════
// 6. /factory-reset + Enter confirms and deletes config
// ═══════════════════════════════════════════════════════════════

#[test]
fn factory_reset_confirm_deletes_config_and_shows_message() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&work_dir).unwrap();

	// Pre-configure the data_dir so the app starts in configured mode.
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::write(
		data_dir.join("config.json"),
		r#"{"logLevel": "warn"}"#,
	)
	.unwrap();
	std::fs::write(
		data_dir.join("acp.json"),
		r#"{"servers": [{"name": "claude-code", "command": "claude"}]}"#,
	)
	.unwrap();

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	type_command(&mut h, "/factory-reset");

	h.wait_for_text("Are you sure")
		.expect("Confirmation dialog should appear");

	// Confirm by pressing Enter.
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Factory reset complete")
		.expect("Should show 'Factory reset complete' after confirming");

	// Verify the data_dir was deleted.
	assert!(
		!data_dir.exists(),
		"Expected data_dir to be deleted after factory reset: {}",
		data_dir.display()
	);
}

// ═══════════════════════════════════════════════════════════════
// 7. /factory-reset + Escape cancels
// ═══════════════════════════════════════════════════════════════

#[test]
fn factory_reset_escape_cancels() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&work_dir).unwrap();

	// Pre-configure the data_dir so the app starts in configured mode.
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::write(
		data_dir.join("config.json"),
		r#"{"logLevel": "warn"}"#,
	)
	.unwrap();
	std::fs::write(
		data_dir.join("acp.json"),
		r#"{"servers": [{"name": "claude-code", "command": "claude"}]}"#,
	)
	.unwrap();

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	type_command(&mut h, "/factory-reset");

	h.wait_for_text("Are you sure")
		.expect("Confirmation dialog should appear");

	// Cancel by pressing Escape.
	send_escape(&mut h);
	settle();

	// Verify the data_dir still exists (not deleted).
	assert!(
		data_dir.exists(),
		"Expected data_dir to still exist after cancelling factory reset: {}",
		data_dir.display()
	);

	// Verify we're back on the chat screen (banner/input should be visible).
	let contents = h.screen_contents();
	assert!(
		!contents.contains("Are you sure"),
		"Confirmation dialog should be dismissed after Escape. Screen:\n{contents}"
	);
}
