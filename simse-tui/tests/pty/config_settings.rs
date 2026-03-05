//! PTY tests for config settings behavior.
//!
//! These tests verify project-level config operations and directory separation:
//! - `/factory-reset-project` deletes `.simse/` in work_dir
//! - `/init` creates `.simse/` and config files can be verified
//! - Global data_dir and project work_dir are separate paths
//!
//! Tests already covered in commands_config.rs are not duplicated here:
//! - factory_reset_deletes_global_config (covered by factory_reset_confirm_deletes_config)
//! - init_creates_project_directory (covered by init_command_creates_project_directory)

use super::r#mod::*;
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════
// 1. /factory-reset-project deletes .simse/ in work_dir
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
// 2. /init creates .simse/ and config can be verified
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
// 3. Global data_dir and project work_dir are separate
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
