//! Thread-safe VirtualFs wrapper around `simse_vfs_engine::vfs::VirtualFs`.
//!
//! Wraps the engine VFS in `Arc<Mutex<...>>` so it can be shared across async
//! tasks. All methods lock the mutex, call the engine, and release before returning.
//! An optional `EventBus` integration publishes VFS events for observability.

use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::error::SimseError;
use crate::events::EventBus;

// Re-export engine types that consumers need
pub use simse_vfs_engine::diff::{DiffHunk, DiffLine, DiffLineType, DiffOutput};
pub use simse_vfs_engine::path::VfsLimits;
pub use simse_vfs_engine::search::{SearchMatch, SearchMode, SearchOptions, SearchResult};
pub use simse_vfs_engine::vfs::{
	DiffResultOutput, DirEntryResult, HistoryEntryResult, MetricsResult, ReadResult, SearchOpts,
	SearchOutput, SnapshotData, SnapshotDir, SnapshotFile, StatResult, TransactionOp, VfsEvent,
};

// ---------------------------------------------------------------------------
// Event type constants for VFS
// ---------------------------------------------------------------------------

pub mod vfs_event_types {
	pub const VFS_WRITE: &str = "vfs.write";
	pub const VFS_DELETE: &str = "vfs.delete";
	pub const VFS_RENAME: &str = "vfs.rename";
	pub const VFS_MKDIR: &str = "vfs.mkdir";
	pub const VFS_SEARCH: &str = "vfs.search";
	pub const VFS_SNAPSHOT: &str = "vfs.snapshot";
	pub const VFS_RESTORE: &str = "vfs.restore";
	pub const VFS_CLEAR: &str = "vfs.clear";
	pub const VFS_TRANSACTION: &str = "vfs.transaction";
}

// ---------------------------------------------------------------------------
// WriteOptions
// ---------------------------------------------------------------------------

/// Options for writing a file to the VFS.
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
	/// Content type: "text" (default) or "binary" (base64-encoded content).
	pub content_type: Option<String>,
	/// Whether to create parent directories automatically.
	pub create_parents: bool,
}

// ---------------------------------------------------------------------------
// VirtualFs
// ---------------------------------------------------------------------------

/// Thread-safe wrapper around the VFS engine.
///
/// All methods acquire the internal mutex lock, perform the operation, and
/// release the lock. The optional `EventBus` publishes events for write,
/// delete, rename, mkdir, and other operations.
pub struct VirtualFs {
	inner: Arc<Mutex<simse_vfs_engine::vfs::VirtualFs>>,
	event_bus: Option<Arc<EventBus>>,
}

impl VirtualFs {
	/// Create a new VirtualFs with the given limits and max history depth.
	pub fn new(limits: VfsLimits, max_history: usize) -> Self {
		Self {
			inner: Arc::new(Mutex::new(simse_vfs_engine::vfs::VirtualFs::new(
				limits,
				max_history,
			))),
			event_bus: None,
		}
	}

	/// Builder method: attach an event bus for publishing VFS events.
	pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
		self.event_bus = Some(bus);
		self
	}

	/// Get a reference to the event bus if attached.
	pub fn event_bus(&self) -> Option<&Arc<EventBus>> {
		self.event_bus.as_ref()
	}

	// -- helpers ----------------------------------------------------------

	fn lock_inner(
		&self,
	) -> std::sync::MutexGuard<'_, simse_vfs_engine::vfs::VirtualFs> {
		self.inner.lock().unwrap_or_else(|e| e.into_inner())
	}

	fn publish_engine_events(&self, events: Vec<VfsEvent>) {
		if let Some(ref bus) = self.event_bus {
			for event in events {
				match &event {
					VfsEvent::Write {
						path,
						size,
						content_type,
						is_new,
					} => {
						bus.publish(
							vfs_event_types::VFS_WRITE,
							json!({
								"path": path,
								"size": size,
								"contentType": content_type,
								"isNew": is_new,
							}),
						);
					}
					VfsEvent::Delete { path } => {
						bus.publish(
							vfs_event_types::VFS_DELETE,
							json!({ "path": path }),
						);
					}
					VfsEvent::Rename { old_path, new_path } => {
						bus.publish(
							vfs_event_types::VFS_RENAME,
							json!({
								"oldPath": old_path,
								"newPath": new_path,
							}),
						);
					}
					VfsEvent::Mkdir { path } => {
						bus.publish(
							vfs_event_types::VFS_MKDIR,
							json!({ "path": path }),
						);
					}
				}
			}
		}
	}

	// -- File operations --------------------------------------------------

	/// Read a file's content from the VFS.
	pub fn read_file(&self, path: &str) -> Result<ReadResult, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.read_file(path)?)
	}

	/// Write a file with content and optional write options.
	pub fn write_file(
		&self,
		path: &str,
		content: &str,
		options: Option<WriteOptions>,
	) -> Result<(), SimseError> {
		let opts = options.unwrap_or_default();
		let mut vfs = self.lock_inner();
		vfs.write_file(
			path,
			content,
			opts.content_type.as_deref(),
			opts.create_parents,
		)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		Ok(())
	}

	/// Append content to an existing text file.
	pub fn append_file(&self, path: &str, content: &str) -> Result<(), SimseError> {
		let mut vfs = self.lock_inner();
		vfs.append_file(path, content)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		Ok(())
	}

	/// Delete a file. Returns `true` if the file existed and was deleted.
	pub fn delete_file(&self, path: &str) -> Result<bool, SimseError> {
		let mut vfs = self.lock_inner();
		let deleted = vfs.delete_file(path)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		Ok(deleted)
	}

	// -- Directory operations ---------------------------------------------

	/// Create a directory. If `recursive` is true, create all parents.
	pub fn mkdir(&self, path: &str, recursive: bool) -> Result<(), SimseError> {
		let mut vfs = self.lock_inner();
		vfs.mkdir(path, recursive)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		Ok(())
	}

	/// List directory contents. If `recursive` is true, list all descendants.
	pub fn readdir(
		&self,
		path: &str,
		recursive: bool,
	) -> Result<Vec<DirEntryResult>, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.readdir(path, recursive)?)
	}

	/// Remove a directory. If `recursive` is true, remove contents too.
	/// Returns `true` if the directory existed and was removed.
	pub fn rmdir(&self, path: &str, recursive: bool) -> Result<bool, SimseError> {
		let mut vfs = self.lock_inner();
		let removed = vfs.rmdir(path, recursive)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		Ok(removed)
	}

	// -- Navigation -------------------------------------------------------

	/// Get file or directory metadata.
	pub fn stat(&self, path: &str) -> Result<StatResult, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.stat(path)?)
	}

	/// Check if a path exists in the VFS.
	pub fn exists(&self, path: &str) -> Result<bool, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.exists(path)?)
	}

	/// Rename (move) a file or directory.
	pub fn rename(&self, old_path: &str, new_path: &str) -> Result<(), SimseError> {
		let mut vfs = self.lock_inner();
		vfs.rename(old_path, new_path)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		Ok(())
	}

	/// Copy a file or directory.
	pub fn copy(
		&self,
		src: &str,
		dest: &str,
		overwrite: bool,
		recursive: bool,
	) -> Result<(), SimseError> {
		let mut vfs = self.lock_inner();
		vfs.copy(src, dest, overwrite, recursive)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		Ok(())
	}

	// -- Query operations -------------------------------------------------

	/// Match files against glob patterns (supports negation with `!` prefix).
	pub fn glob(&self, patterns: Vec<String>) -> Vec<String> {
		let vfs = self.lock_inner();
		vfs.glob(patterns)
	}

	/// Return a tree-formatted string of directory contents.
	pub fn tree(&self, root: Option<&str>) -> Result<String, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.tree(root)?)
	}

	/// Return disk usage (total bytes) under a path.
	pub fn du(&self, path: &str) -> Result<u64, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.du(path)?)
	}

	/// Search file contents.
	pub fn search(
		&self,
		query: &str,
		options: SearchOpts,
	) -> Result<SearchOutput, SimseError> {
		let vfs = self.lock_inner();
		let result = vfs.search(query, options)?;
		drop(vfs);
		if let Some(ref bus) = self.event_bus {
			bus.publish(
				vfs_event_types::VFS_SEARCH,
				json!({ "query": query }),
			);
		}
		Ok(result)
	}

	// -- History & Diff ---------------------------------------------------

	/// Get version history for a file.
	pub fn history(&self, path: &str) -> Result<Vec<HistoryEntryResult>, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.history(path)?)
	}

	/// Compute a diff between two files.
	pub fn diff(
		&self,
		old_path: &str,
		new_path: &str,
		context: usize,
	) -> Result<DiffResultOutput, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.diff(old_path, new_path, context)?)
	}

	/// Compute a diff between two versions of the same file.
	pub fn diff_versions(
		&self,
		path: &str,
		old_ver: usize,
		new_ver: Option<usize>,
		context: usize,
	) -> Result<DiffResultOutput, SimseError> {
		let vfs = self.lock_inner();
		Ok(vfs.diff_versions(path, old_ver, new_ver, context)?)
	}

	/// Checkout (revert) a file to a specific version.
	pub fn checkout(&self, path: &str, version: usize) -> Result<(), SimseError> {
		let mut vfs = self.lock_inner();
		vfs.checkout(path, version)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		Ok(())
	}

	// -- Snapshot & Restore -----------------------------------------------

	/// Take a snapshot of the entire VFS state.
	pub fn snapshot(&self) -> SnapshotData {
		let vfs = self.lock_inner();
		let snap = vfs.snapshot();
		drop(vfs);
		if let Some(ref bus) = self.event_bus {
			bus.publish(
				vfs_event_types::VFS_SNAPSHOT,
				json!({
					"files": snap.files.len(),
					"directories": snap.directories.len(),
				}),
			);
		}
		snap
	}

	/// Restore the VFS from a snapshot.
	pub fn restore(&self, snapshot: SnapshotData) -> Result<(), SimseError> {
		let mut vfs = self.lock_inner();
		vfs.restore(snapshot)?;
		drop(vfs);
		if let Some(ref bus) = self.event_bus {
			bus.publish(vfs_event_types::VFS_RESTORE, json!({}));
		}
		Ok(())
	}

	/// Clear the entire VFS (keeps only the root directory).
	pub fn clear(&self) {
		let mut vfs = self.lock_inner();
		vfs.clear();
		drop(vfs);
		if let Some(ref bus) = self.event_bus {
			bus.publish(vfs_event_types::VFS_CLEAR, json!({}));
		}
	}

	// -- Transaction ------------------------------------------------------

	/// Execute a sequence of operations atomically. Rolls back on failure.
	pub fn transaction(&self, ops: Vec<TransactionOp>) -> Result<(), SimseError> {
		let op_count = ops.len();
		let mut vfs = self.lock_inner();
		vfs.transaction(ops)?;
		let events = vfs.drain_events();
		drop(vfs);
		self.publish_engine_events(events);
		if let Some(ref bus) = self.event_bus {
			bus.publish(
				vfs_event_types::VFS_TRANSACTION,
				json!({ "operations": op_count }),
			);
		}
		Ok(())
	}

	// -- Metrics & Events -------------------------------------------------

	/// Get VFS metrics (total size, node/file/directory counts).
	pub fn metrics(&self) -> MetricsResult {
		let vfs = self.lock_inner();
		vfs.metrics()
	}

	/// Drain pending engine events.
	pub fn drain_events(&self) -> Vec<VfsEvent> {
		let mut vfs = self.lock_inner();
		vfs.drain_events()
	}
}
