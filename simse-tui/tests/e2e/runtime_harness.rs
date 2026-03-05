//! RuntimeTestHarness — file I/O test harness with temporary directories.
//!
//! Uses `tempfile::TempDir` for isolated test directories that are
//! automatically cleaned up when the harness is dropped.

use std::path::PathBuf;
use tempfile::TempDir;

/// Test harness that creates temporary data and work directories for
/// testing real file I/O operations.
pub struct RuntimeTestHarness {
	/// Temporary directory simulating the global data directory (~/.config/simse).
	pub data_dir: TempDir,
	/// Temporary directory simulating the project working directory.
	pub work_dir: TempDir,
}

impl RuntimeTestHarness {
	/// Create a new harness with fresh temporary directories.
	pub fn new() -> Self {
		Self {
			data_dir: TempDir::new().unwrap(),
			work_dir: TempDir::new().unwrap(),
		}
	}

	/// Get the path to the global data directory.
	pub fn data_path(&self) -> PathBuf {
		self.data_dir.path().to_path_buf()
	}

	/// Get the path to the project working directory.
	pub fn work_path(&self) -> PathBuf {
		self.work_dir.path().to_path_buf()
	}

	/// Write a JSON config file to the data directory.
	pub fn write_config(&self, config: &serde_json::Value) {
		let config_path = self.data_dir.path().join("config.json");
		std::fs::write(
			&config_path,
			serde_json::to_string_pretty(config).unwrap(),
		)
		.unwrap();
	}

	/// Create a `.simse/` project directory in the work directory.
	pub fn init_project(&self) {
		let project_dir = self.work_dir.path().join(".simse");
		std::fs::create_dir_all(&project_dir).unwrap();
	}

	/// Check if the global data directory has any contents.
	pub fn global_config_exists(&self) -> bool {
		self.data_dir.path().exists()
			&& self
				.data_dir
				.path()
				.read_dir()
				.map(|mut rd| rd.next().is_some())
				.unwrap_or(false)
	}

	/// Check if the project `.simse/` directory exists.
	pub fn project_config_exists(&self) -> bool {
		self.work_dir.path().join(".simse").exists()
	}
}
