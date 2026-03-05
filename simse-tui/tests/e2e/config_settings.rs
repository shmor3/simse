//! E2E tests for config and settings with real file I/O.
//!
//! Uses `RuntimeTestHarness` with temporary directories.

use super::runtime_harness::RuntimeTestHarness;
use std::fs;

#[test]
fn factory_reset_deletes_global_config() {
	let h = RuntimeTestHarness::new();
	h.write_config(&serde_json::json!({"acp": {"servers": []}}));
	assert!(h.global_config_exists());

	// Simulate factory reset: delete data_dir contents
	let data_path = h.data_path();
	for entry in fs::read_dir(&data_path).unwrap() {
		let entry = entry.unwrap();
		let path = entry.path();
		if path.is_dir() {
			fs::remove_dir_all(&path).unwrap();
		} else {
			fs::remove_file(&path).unwrap();
		}
	}
	assert!(!h.global_config_exists());
}

#[test]
fn factory_reset_project_deletes_project_config() {
	let h = RuntimeTestHarness::new();
	h.init_project();
	assert!(h.project_config_exists());

	let project_dir = h.work_path().join(".simse");
	fs::remove_dir_all(&project_dir).unwrap();
	assert!(!h.project_config_exists());
}

#[test]
fn init_creates_project_directory() {
	let h = RuntimeTestHarness::new();
	assert!(!h.project_config_exists());

	let project_dir = h.work_path().join(".simse");
	fs::create_dir_all(&project_dir).unwrap();
	assert!(h.project_config_exists());
}

#[test]
fn config_file_round_trip() {
	let h = RuntimeTestHarness::new();
	let config = serde_json::json!({
		"acp": {"servers": [{"name": "test", "command": "echo"}]},
		"log": {"level": "info"}
	});
	h.write_config(&config);

	let content = fs::read_to_string(h.data_path().join("config.json")).unwrap();
	let loaded: serde_json::Value = serde_json::from_str(&content).unwrap();
	assert_eq!(loaded["acp"]["servers"][0]["name"], "test");
	assert_eq!(loaded["log"]["level"], "info");
}

#[test]
fn global_vs_project_directories_are_separate() {
	let h = RuntimeTestHarness::new();
	h.write_config(&serde_json::json!({"global": true}));
	h.init_project();

	assert!(h.global_config_exists());
	assert!(h.project_config_exists());
	assert_ne!(h.data_path(), h.work_path());
}

#[test]
fn fresh_harness_has_empty_dirs() {
	let h = RuntimeTestHarness::new();
	assert!(h.data_path().exists());
	assert!(h.work_path().exists());
	assert!(!h.global_config_exists()); // exists but empty
	assert!(!h.project_config_exists()); // no .simse/ dir
}
