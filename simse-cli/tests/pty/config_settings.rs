//! PTY tests for config settings behavior.
//!
//! These tests verify settings persistence (actual file I/O on disk):
//! - Settings form loads config data from a pre-written config file
//! - `/factory-reset-project` deletes `.simse/` in work_dir
//! - `/init` creates `.simse/` and config files can be verified
//! - Global data_dir and project work_dir are separate paths
//!
//! Tests already covered in commands_config.rs are not duplicated here:
//! - factory_reset_deletes_global_config (covered by factory_reset_confirm_deletes_config)
//! - init_creates_project_directory (covered by init_command_creates_project_directory)

use super::r#mod::*;
use std::time::Duration;
use tempfile::TempDir;

/// Small delay to let the PTY propagate key events and re-render.
fn settle() {
	std::thread::sleep(Duration::from_millis(300));
}

// ═══════════════════════════════════════════════════════════════
// 1. /settings opens config.json and shows values loaded from disk
// ═══════════════════════════════════════════════════════════════

/// Open /settings → press Enter to open config.json → verify field values from disk are shown.
#[test]
fn settings_form_loads_config_from_disk() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&work_dir).unwrap();

	// Write a config.json with a known value.
	let config_data = serde_json::json!({"logLevel": "debug"});
	std::fs::write(
		data_dir.join("config.json"),
		serde_json::to_string_pretty(&config_data).unwrap(),
	)
	.unwrap();

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	type_command(&mut h, "/settings");
	h.wait_for_text("Settings")
		.expect("Settings overlay should open");

	// Press Enter to open config.json (first item in the file list).
	h.send_keys("\r").unwrap();
	settle();
	settle(); // extra settle for async file load

	// Should show the field value loaded from disk.
	let contents = h.screen_contents();
	assert!(
		contents.contains("debug") || contents.contains("logLevel"),
		"Settings form should show loaded config data from disk. Screen:\n{contents}"
	);
}

// ═══════════════════════════════════════════════════════════════
// 2. /factory-reset-project deletes .simse/ in work_dir
// ═══════════════════════════════════════════════════════════════

#[test]
fn factory_reset_project_deletes_project_config() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&work_dir).unwrap();

	write_default_config(&data_dir);

	// Create a .simse/ project directory in work_dir.
	let project_dir = work_dir.join(".simse");
	std::fs::create_dir_all(&project_dir).unwrap();
	std::fs::write(project_dir.join("config.json"), r#"{"project": true}"#).unwrap();
	assert!(project_dir.exists(), ".simse/ should exist before reset");

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	type_command(&mut h, "/factory-reset-project");

	h.wait_for_text("Are you sure")
		.expect("Confirmation dialog should appear for /factory-reset-project");

	// Confirm by pressing Enter.
	h.send_key(KeyCode::Enter).unwrap();

	h.wait_for_text("Project configuration reset")
		.expect("Should show 'Project configuration reset' after confirming");

	// Verify the .simse/ directory was deleted.
	assert!(
		!project_dir.exists(),
		"Expected .simse/ directory to be deleted after /factory-reset-project: {}",
		project_dir.display()
	);
}

// ═══════════════════════════════════════════════════════════════
// 3. /init creates .simse/ and config can be verified
// ═══════════════════════════════════════════════════════════════

#[test]
fn config_file_round_trip_via_init() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&work_dir).unwrap();

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	type_command(&mut h, "/init");

	h.wait_for_text("Initialized")
		.expect("'/init' should show 'Initialized'");

	// Verify the .simse/ directory was created.
	let project_dir = work_dir.join(".simse");
	assert!(
		project_dir.exists(),
		"Expected .simse/ directory to be created at {}",
		project_dir.display()
	);

	// Verify the directory is a real directory (not a file).
	assert!(
		project_dir.is_dir(),
		"Expected .simse/ to be a directory at {}",
		project_dir.display()
	);
}

// ═══════════════════════════════════════════════════════════════
// 4. Global data_dir and project work_dir are separate
// ═══════════════════════════════════════════════════════════════

#[test]
fn global_vs_project_directories_separate() {
	let tmp = TempDir::new().unwrap();
	let data_dir = tmp.path().join("data");
	let work_dir = tmp.path().join("work");
	std::fs::create_dir_all(&data_dir).unwrap();
	std::fs::create_dir_all(&work_dir).unwrap();

	// Verify the directories are indeed different paths.
	assert_ne!(
		data_dir, work_dir,
		"data_dir and work_dir should be separate paths"
	);

	write_default_config(&data_dir);

	let mut h = spawn_simse_with_cwd(&data_dir, &work_dir);
	wait_for_startup(&h);

	// Create project config via /init.
	type_command(&mut h, "/init");

	h.wait_for_text("Initialized")
		.expect("'/init' should show 'Initialized'");

	// Verify both directories exist and are separate.
	assert!(data_dir.exists(), "data_dir should exist");
	assert!(work_dir.join(".simse").exists(), ".simse/ should exist in work_dir");

	// Verify global config is in data_dir, not in work_dir.
	assert!(
		data_dir.join("config.json").exists(),
		"Global config should be in data_dir"
	);
	// Verify project config is in work_dir/.simse/, not in data_dir.
	assert!(
		work_dir.join(".simse").exists(),
		"Project config should be in work_dir/.simse/"
	);
	assert!(
		!data_dir.join(".simse").exists(),
		"data_dir should NOT have a .simse/ directory"
	);
}
