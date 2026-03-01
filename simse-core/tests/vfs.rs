//! Tests for the VFS orchestration layer: VirtualFs wrapper, VfsDisk, VfsExec, and validators.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use simse_core::error::SimseError;
use simse_core::events::EventBus;
use simse_core::vfs::disk::{CommitOpType, CommitOptions, LoadOptions, VfsDisk, is_binary_extension};
use simse_core::vfs::exec::{ExecBackend, ExecOptions, ExecResult, VfsExec};
use simse_core::vfs::validators::{
	EmptyFileValidator, JsonSyntaxValidator, MissingTrailingNewlineValidator,
	MixedIndentationValidator, MixedLineEndingsValidator, TrailingWhitespaceValidator,
	ValidationSeverity, VfsValidator, default_validators, validate_snapshot,
};
use simse_core::vfs::vfs::{
	SearchOpts, SnapshotData, SnapshotFile, TransactionOp, VfsLimits, VirtualFs,
	WriteOptions,
};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn new_vfs() -> VirtualFs {
	VirtualFs::new(VfsLimits::default(), 50)
}

fn new_vfs_arc() -> Arc<VirtualFs> {
	Arc::new(new_vfs())
}

// =========================================================================
// VirtualFs basic file operations
// =========================================================================

#[test]
fn vfs_write_and_read_text_file() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///hello.txt", "hello world", None)
		.unwrap();
	let result = vfs.read_file("vfs:///hello.txt").unwrap();
	assert_eq!(result.content_type, "text");
	assert_eq!(result.text.as_deref(), Some("hello world"));
	assert_eq!(result.size, 11);
}

#[test]
fn vfs_write_with_create_parents() {
	let vfs = new_vfs();
	let opts = WriteOptions {
		create_parents: true,
		..Default::default()
	};
	vfs.write_file("vfs:///a/b/c/file.txt", "nested", Some(opts))
		.unwrap();
	let result = vfs.read_file("vfs:///a/b/c/file.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("nested"));
}

#[test]
fn vfs_overwrite_file() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///f.txt", "v1", None).unwrap();
	vfs.write_file("vfs:///f.txt", "v2", None).unwrap();
	let result = vfs.read_file("vfs:///f.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("v2"));
}

#[test]
fn vfs_append_file() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///a.txt", "hello", None).unwrap();
	vfs.append_file("vfs:///a.txt", " world").unwrap();
	let result = vfs.read_file("vfs:///a.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("hello world"));
}

#[test]
fn vfs_delete_file() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///d.txt", "data", None).unwrap();
	assert!(vfs.exists("vfs:///d.txt").unwrap());
	let deleted = vfs.delete_file("vfs:///d.txt").unwrap();
	assert!(deleted);
	assert!(!vfs.exists("vfs:///d.txt").unwrap());
}

#[test]
fn vfs_delete_nonexistent_returns_false() {
	let vfs = new_vfs();
	let deleted = vfs.delete_file("vfs:///nope.txt").unwrap();
	assert!(!deleted);
}

#[test]
fn vfs_exists_root() {
	let vfs = new_vfs();
	assert!(vfs.exists("vfs:///").unwrap());
}

#[test]
fn vfs_stat_file() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///info.txt", "data", None).unwrap();
	let stat = vfs.stat("vfs:///info.txt").unwrap();
	assert_eq!(stat.node_type, "file");
	assert_eq!(stat.size, 4);
	assert!(stat.created_at > 0);
}

#[test]
fn vfs_stat_directory() {
	let vfs = new_vfs();
	let stat = vfs.stat("vfs:///").unwrap();
	assert_eq!(stat.node_type, "directory");
}

// =========================================================================
// VirtualFs directory operations
// =========================================================================

#[test]
fn vfs_mkdir_and_readdir() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///docs", false).unwrap();
	vfs.write_file("vfs:///docs/readme.txt", "hi", None)
		.unwrap();
	let entries = vfs.readdir("vfs:///docs", false).unwrap();
	assert_eq!(entries.len(), 1);
	assert_eq!(entries[0].name, "readme.txt");
	assert_eq!(entries[0].node_type, "file");
}

#[test]
fn vfs_mkdir_recursive() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///a/b/c", true).unwrap();
	assert!(vfs.exists("vfs:///a").unwrap());
	assert!(vfs.exists("vfs:///a/b").unwrap());
	assert!(vfs.exists("vfs:///a/b/c").unwrap());
}

#[test]
fn vfs_readdir_recursive() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///src", false).unwrap();
	vfs.write_file("vfs:///src/main.rs", "fn main() {}", None)
		.unwrap();
	vfs.mkdir("vfs:///src/lib", false).unwrap();
	vfs.write_file("vfs:///src/lib/mod.rs", "pub mod foo;", None)
		.unwrap();
	let entries = vfs.readdir("vfs:///src", true).unwrap();
	assert!(entries.len() >= 3); // lib/, main.rs, lib/mod.rs
}

#[test]
fn vfs_rmdir_empty() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///empty", false).unwrap();
	let removed = vfs.rmdir("vfs:///empty", false).unwrap();
	assert!(removed);
	assert!(!vfs.exists("vfs:///empty").unwrap());
}

#[test]
fn vfs_rmdir_recursive() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///dir", false).unwrap();
	vfs.write_file("vfs:///dir/file.txt", "data", None).unwrap();
	let removed = vfs.rmdir("vfs:///dir", true).unwrap();
	assert!(removed);
	assert!(!vfs.exists("vfs:///dir").unwrap());
}

#[test]
fn vfs_rmdir_nonempty_without_recursive_fails() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///dir", false).unwrap();
	vfs.write_file("vfs:///dir/file.txt", "data", None).unwrap();
	let result = vfs.rmdir("vfs:///dir", false);
	assert!(result.is_err());
}

// =========================================================================
// VirtualFs rename & copy
// =========================================================================

#[test]
fn vfs_rename_file() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///old.txt", "content", None).unwrap();
	vfs.rename("vfs:///old.txt", "vfs:///new.txt").unwrap();
	assert!(!vfs.exists("vfs:///old.txt").unwrap());
	let result = vfs.read_file("vfs:///new.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("content"));
}

#[test]
fn vfs_copy_file() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///src.txt", "original", None).unwrap();
	vfs.copy("vfs:///src.txt", "vfs:///dst.txt", false, false)
		.unwrap();
	let src = vfs.read_file("vfs:///src.txt").unwrap();
	let dst = vfs.read_file("vfs:///dst.txt").unwrap();
	assert_eq!(src.text, dst.text);
}

#[test]
fn vfs_copy_fails_if_dest_exists_and_no_overwrite() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///a.txt", "a", None).unwrap();
	vfs.write_file("vfs:///b.txt", "b", None).unwrap();
	let result = vfs.copy("vfs:///a.txt", "vfs:///b.txt", false, false);
	assert!(result.is_err());
}

// =========================================================================
// VirtualFs search, glob, tree, du
// =========================================================================

#[test]
fn vfs_glob_basic() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///a.txt", "a", None).unwrap();
	vfs.write_file("vfs:///b.rs", "b", None).unwrap();
	let opts = WriteOptions {
		create_parents: true,
		..Default::default()
	};
	vfs.write_file("vfs:///src/c.rs", "c", Some(opts)).unwrap();

	let results = vfs.glob(vec!["**/*.rs".to_string()]);
	assert!(results.len() >= 2);
}

#[test]
fn vfs_glob_negation() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///a.txt", "a", None).unwrap();
	vfs.write_file("vfs:///b.txt", "b", None).unwrap();
	vfs.write_file("vfs:///c.rs", "c", None).unwrap();

	let results = vfs.glob(vec!["**/*".to_string(), "!**/*.rs".to_string()]);
	for r in &results {
		assert!(!r.ends_with(".rs"));
	}
}

#[test]
fn vfs_tree() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///src", false).unwrap();
	vfs.write_file("vfs:///src/main.rs", "fn main(){}", None)
		.unwrap();
	let tree = vfs.tree(None).unwrap();
	assert!(tree.contains("src/"));
	assert!(tree.contains("main.rs"));
}

#[test]
fn vfs_du() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///dir", false).unwrap();
	vfs.write_file("vfs:///dir/a.txt", "hello", None).unwrap(); // 5 bytes
	vfs.write_file("vfs:///dir/b.txt", "world!", None).unwrap(); // 6 bytes
	let usage = vfs.du("vfs:///dir").unwrap();
	assert_eq!(usage, 11);
}

#[test]
fn vfs_search_substring() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///code.rs", "fn hello() {}\nfn world() {}", None)
		.unwrap();
	let opts = SearchOpts {
		mode: "substring".to_string(),
		..Default::default()
	};
	let result = vfs.search("hello", opts).unwrap();
	match result {
		simse_core::vfs::vfs::SearchOutput::Results(matches) => {
			assert!(!matches.is_empty());
		}
		_ => panic!("Expected Results"),
	}
}

#[test]
fn vfs_search_count_only() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///f.txt", "aaa\nbbb\naaa", None)
		.unwrap();
	let opts = SearchOpts {
		count_only: true,
		..Default::default()
	};
	let result = vfs.search("aaa", opts).unwrap();
	match result {
		simse_core::vfs::vfs::SearchOutput::Count(c) => {
			assert_eq!(c, 2);
		}
		_ => panic!("Expected Count"),
	}
}

// =========================================================================
// VirtualFs history, diff, diff_versions, checkout
// =========================================================================

#[test]
fn vfs_history_tracks_versions() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///h.txt", "v1", None).unwrap();
	vfs.write_file("vfs:///h.txt", "v2", None).unwrap();
	vfs.write_file("vfs:///h.txt", "v3", None).unwrap();
	let hist = vfs.history("vfs:///h.txt").unwrap();
	assert_eq!(hist.len(), 2); // v1 and v2 in history (v3 is current)
}

#[test]
fn vfs_diff_between_files() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///old.txt", "line1\nline2\nline3", None)
		.unwrap();
	vfs.write_file("vfs:///new.txt", "line1\nchanged\nline3", None)
		.unwrap();
	let diff = vfs.diff("vfs:///old.txt", "vfs:///new.txt", 3).unwrap();
	assert!(diff.additions > 0 || diff.deletions > 0);
}

#[test]
fn vfs_diff_versions() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///v.txt", "version1", None).unwrap();
	vfs.write_file("vfs:///v.txt", "version2", None).unwrap();
	let diff = vfs.diff_versions("vfs:///v.txt", 1, None, 3).unwrap();
	assert!(diff.additions > 0 || diff.deletions > 0);
}

#[test]
fn vfs_checkout_restores_version() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///co.txt", "original", None).unwrap();
	vfs.write_file("vfs:///co.txt", "updated", None).unwrap();
	vfs.checkout("vfs:///co.txt", 1).unwrap();
	let result = vfs.read_file("vfs:///co.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("original"));
}

// =========================================================================
// VirtualFs snapshot, restore, clear
// =========================================================================

#[test]
fn vfs_snapshot_and_restore() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///data", false).unwrap();
	vfs.write_file("vfs:///data/file.txt", "snapshot me", None)
		.unwrap();
	let snap = vfs.snapshot();
	assert!(!snap.files.is_empty());

	vfs.clear();
	assert!(!vfs.exists("vfs:///data").unwrap());

	vfs.restore(snap).unwrap();
	let result = vfs.read_file("vfs:///data/file.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("snapshot me"));
}

#[test]
fn vfs_clear_resets_to_root() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///x.txt", "data", None).unwrap();
	vfs.clear();
	let metrics = vfs.metrics();
	assert_eq!(metrics.file_count, 0);
	assert_eq!(metrics.directory_count, 1); // root
	assert!(vfs.exists("vfs:///").unwrap());
}

// =========================================================================
// VirtualFs transaction
// =========================================================================

#[test]
fn vfs_transaction_success() {
	let vfs = new_vfs();
	vfs.transaction(vec![
		TransactionOp::Mkdir {
			path: "vfs:///txn".to_string(),
		},
		TransactionOp::WriteFile {
			path: "vfs:///txn/f.txt".to_string(),
			content: "txn data".to_string(),
		},
	])
	.unwrap();

	let result = vfs.read_file("vfs:///txn/f.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("txn data"));
}

#[test]
fn vfs_transaction_rollback_on_failure() {
	let vfs = new_vfs();
	vfs.write_file("vfs:///existing.txt", "keep me", None)
		.unwrap();

	// This transaction should fail on the second op (write to root)
	let result = vfs.transaction(vec![
		TransactionOp::WriteFile {
			path: "vfs:///new.txt".to_string(),
			content: "new".to_string(),
		},
		TransactionOp::WriteFile {
			path: "vfs:///".to_string(), // writing to root fails
			content: "bad".to_string(),
		},
	]);
	assert!(result.is_err());

	// Original file should still exist
	let r = vfs.read_file("vfs:///existing.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("keep me"));
	// New file should have been rolled back
	assert!(!vfs.exists("vfs:///new.txt").unwrap());
}

// =========================================================================
// VirtualFs metrics & events
// =========================================================================

#[test]
fn vfs_metrics() {
	let vfs = new_vfs();
	vfs.mkdir("vfs:///src", false).unwrap();
	vfs.write_file("vfs:///src/main.rs", "fn main(){}", None)
		.unwrap();
	let m = vfs.metrics();
	assert_eq!(m.file_count, 1);
	assert_eq!(m.directory_count, 2); // root + src
	assert_eq!(m.total_size, 11);
}

#[test]
fn vfs_drain_events_empty_after_write() {
	// The wrapper drains events internally on each operation to publish them
	// to the event bus, so drain_events() returns empty after a write_file().
	let vfs = new_vfs();
	vfs.write_file("vfs:///ev.txt", "data", None).unwrap();
	let events = vfs.drain_events();
	assert!(events.is_empty());
}

#[test]
fn vfs_engine_events_published_via_bus() {
	// Verify that engine events are published to the event bus instead of
	// being accumulated in drain_events().
	let bus = Arc::new(EventBus::new());
	let vfs = VirtualFs::new(VfsLimits::default(), 50).with_event_bus(bus.clone());

	let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
	let count_clone = count.clone();
	let _unsub = bus.subscribe("vfs.write", move |_| {
		count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
	});

	vfs.write_file("vfs:///a.txt", "1", None).unwrap();
	vfs.write_file("vfs:///b.txt", "2", None).unwrap();

	assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
	// drain_events should still be empty
	assert!(vfs.drain_events().is_empty());
}

#[test]
fn vfs_event_bus_integration() {
	let bus = Arc::new(EventBus::new());
	let vfs = VirtualFs::new(VfsLimits::default(), 50).with_event_bus(bus.clone());

	let received = Arc::new(std::sync::Mutex::new(Vec::new()));
	let received_clone = received.clone();
	let _unsub = bus.subscribe("vfs.write", move |payload| {
		received_clone
			.lock()
			.unwrap()
			.push(payload.clone());
	});

	vfs.write_file("vfs:///event.txt", "hello", None).unwrap();

	let events = received.lock().unwrap();
	assert_eq!(events.len(), 1);
	assert_eq!(events[0]["path"], "vfs:///event.txt");
}

#[test]
fn vfs_event_bus_delete() {
	let bus = Arc::new(EventBus::new());
	let vfs = VirtualFs::new(VfsLimits::default(), 50).with_event_bus(bus.clone());

	let received = Arc::new(std::sync::Mutex::new(Vec::new()));
	let received_clone = received.clone();
	let _unsub = bus.subscribe("vfs.delete", move |payload| {
		received_clone
			.lock()
			.unwrap()
			.push(payload.clone());
	});

	vfs.write_file("vfs:///del.txt", "data", None).unwrap();
	vfs.delete_file("vfs:///del.txt").unwrap();

	let events = received.lock().unwrap();
	assert_eq!(events.len(), 1);
}

#[test]
fn vfs_event_bus_rename() {
	let bus = Arc::new(EventBus::new());
	let vfs = VirtualFs::new(VfsLimits::default(), 50).with_event_bus(bus.clone());

	let received = Arc::new(std::sync::Mutex::new(Vec::new()));
	let received_clone = received.clone();
	let _unsub = bus.subscribe("vfs.rename", move |payload| {
		received_clone
			.lock()
			.unwrap()
			.push(payload.clone());
	});

	vfs.write_file("vfs:///old.txt", "data", None).unwrap();
	vfs.rename("vfs:///old.txt", "vfs:///new.txt").unwrap();

	let events = received.lock().unwrap();
	assert_eq!(events.len(), 1);
	assert_eq!(events[0]["oldPath"], "vfs:///old.txt");
	assert_eq!(events[0]["newPath"], "vfs:///new.txt");
}

#[test]
fn vfs_event_bus_mkdir() {
	let bus = Arc::new(EventBus::new());
	let vfs = VirtualFs::new(VfsLimits::default(), 50).with_event_bus(bus.clone());

	let received = Arc::new(std::sync::Mutex::new(Vec::new()));
	let received_clone = received.clone();
	let _unsub = bus.subscribe("vfs.mkdir", move |payload| {
		received_clone
			.lock()
			.unwrap()
			.push(payload.clone());
	});

	vfs.mkdir("vfs:///newdir", false).unwrap();

	let events = received.lock().unwrap();
	assert_eq!(events.len(), 1);
}

// =========================================================================
// VfsDisk — commit and load
// =========================================================================

#[test]
fn vfs_disk_commit_writes_files() {
	let vfs = new_vfs_arc();
	vfs.mkdir("vfs:///src", false).unwrap();
	vfs.write_file("vfs:///src/main.rs", "fn main() {}", None)
		.unwrap();
	vfs.write_file("vfs:///readme.txt", "Hello", None).unwrap();

	let tmp = tempfile::tempdir().unwrap();
	let disk = VfsDisk::new(vfs, tmp.path().to_path_buf());

	let result = disk
		.commit(
			tmp.path(),
			CommitOptions {
				overwrite: true,
				..Default::default()
			},
			None,
		)
		.unwrap();

	assert!(result.files_written >= 2);
	assert!(tmp.path().join("src/main.rs").exists());
	assert!(tmp.path().join("readme.txt").exists());

	let content = std::fs::read_to_string(tmp.path().join("readme.txt")).unwrap();
	assert_eq!(content, "Hello");
}

#[test]
fn vfs_disk_commit_dry_run() {
	let vfs = new_vfs_arc();
	vfs.write_file("vfs:///f.txt", "data", None).unwrap();

	let tmp = tempfile::tempdir().unwrap();
	let disk = VfsDisk::new(vfs, tmp.path().to_path_buf());

	let result = disk
		.commit(
			tmp.path(),
			CommitOptions {
				dry_run: true,
				..Default::default()
			},
			None,
		)
		.unwrap();

	assert!(result.files_written >= 1);
	// Dry run: file should NOT exist on disk
	assert!(!tmp.path().join("f.txt").exists());
}

#[test]
fn vfs_disk_commit_filter() {
	let vfs = new_vfs_arc();
	vfs.write_file("vfs:///include.txt", "yes", None).unwrap();
	vfs.write_file("vfs:///exclude.log", "no", None).unwrap();

	let tmp = tempfile::tempdir().unwrap();
	let disk = VfsDisk::new(vfs, tmp.path().to_path_buf());

	let result = disk
		.commit(
			tmp.path(),
			CommitOptions {
				overwrite: true,
				filter: Some(Box::new(|path: &str| path.ends_with(".txt"))),
				..Default::default()
			},
			None,
		)
		.unwrap();

	assert!(tmp.path().join("include.txt").exists());
	assert!(!tmp.path().join("exclude.log").exists());

	// Check that the skipped file appears in operations
	let skipped: Vec<_> = result
		.operations
		.iter()
		.filter(|op| op.op_type == CommitOpType::Skip)
		.collect();
	assert!(!skipped.is_empty());
}

#[test]
fn vfs_disk_load_from_directory() {
	let tmp = tempfile::tempdir().unwrap();
	std::fs::write(tmp.path().join("hello.txt"), "world").unwrap();
	std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
	std::fs::write(tmp.path().join("sub/nested.txt"), "deep").unwrap();

	let vfs = new_vfs_arc();
	let disk = VfsDisk::new(vfs.clone(), tmp.path().to_path_buf());

	let result = disk
		.load(tmp.path(), LoadOptions::default())
		.unwrap();

	assert!(result.files_written >= 2);
	let r = vfs.read_file("vfs:///hello.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("world"));
	let r2 = vfs.read_file("vfs:///sub/nested.txt").unwrap();
	assert_eq!(r2.text.as_deref(), Some("deep"));
}

#[test]
fn vfs_disk_load_max_file_size() {
	let tmp = tempfile::tempdir().unwrap();
	std::fs::write(tmp.path().join("small.txt"), "hi").unwrap();
	std::fs::write(tmp.path().join("big.txt"), "a".repeat(1000)).unwrap();

	let vfs = new_vfs_arc();
	let disk = VfsDisk::new(vfs.clone(), tmp.path().to_path_buf());

	let result = disk
		.load(
			tmp.path(),
			LoadOptions {
				max_file_size: Some(100),
				..Default::default()
			},
		)
		.unwrap();

	assert!(vfs.exists("vfs:///small.txt").unwrap());
	// big.txt should have been skipped
	assert!(!vfs.exists("vfs:///big.txt").unwrap());

	let skipped: Vec<_> = result
		.operations
		.iter()
		.filter(|op| op.op_type == CommitOpType::Skip)
		.collect();
	assert!(!skipped.is_empty());
}

#[test]
fn vfs_disk_load_filter() {
	let tmp = tempfile::tempdir().unwrap();
	std::fs::write(tmp.path().join("yes.txt"), "include").unwrap();
	std::fs::write(tmp.path().join("no.log"), "exclude").unwrap();

	let vfs = new_vfs_arc();
	let disk = VfsDisk::new(vfs.clone(), tmp.path().to_path_buf());

	disk.load(
		tmp.path(),
		LoadOptions {
			filter: Some(Box::new(|path: &str| path.ends_with(".txt"))),
			..Default::default()
		},
	)
	.unwrap();

	assert!(vfs.exists("vfs:///yes.txt").unwrap());
	assert!(!vfs.exists("vfs:///no.log").unwrap());
}

// =========================================================================
// Binary extension detection
// =========================================================================

#[test]
fn binary_extension_known_types() {
	assert!(is_binary_extension("png"));
	assert!(is_binary_extension("PNG")); // case insensitive
	assert!(is_binary_extension("jpg"));
	assert!(is_binary_extension("wasm"));
	assert!(is_binary_extension("exe"));
	assert!(is_binary_extension("dll"));
	assert!(is_binary_extension("zip"));
	assert!(is_binary_extension("pdf"));
	assert!(is_binary_extension("woff2"));
	assert!(is_binary_extension("mp4"));
}

#[test]
fn binary_extension_text_types() {
	assert!(!is_binary_extension("txt"));
	assert!(!is_binary_extension("rs"));
	assert!(!is_binary_extension("ts"));
	assert!(!is_binary_extension("md"));
	assert!(!is_binary_extension("json"));
	assert!(!is_binary_extension("html"));
}

// =========================================================================
// Validators
// =========================================================================

#[test]
fn validator_json_syntax_valid() {
	let v = JsonSyntaxValidator;
	let issues = v.validate("test.json", r#"{"key": "value"}"#);
	assert!(issues.is_empty());
}

#[test]
fn validator_json_syntax_invalid() {
	let v = JsonSyntaxValidator;
	let issues = v.validate("test.json", r#"{"key": bad}"#);
	assert_eq!(issues.len(), 1);
	assert_eq!(issues[0].severity, ValidationSeverity::Error);
	assert_eq!(issues[0].code, "json_syntax");
}

#[test]
fn validator_json_syntax_empty() {
	let v = JsonSyntaxValidator;
	let issues = v.validate("test.json", "");
	assert_eq!(issues.len(), 1);
	assert_eq!(issues[0].severity, ValidationSeverity::Error);
}

#[test]
fn validator_json_extensions_filter() {
	let v = JsonSyntaxValidator;
	assert_eq!(v.extensions(), Some(&["json"][..]));
}

#[test]
fn validator_trailing_whitespace_detects() {
	let v = TrailingWhitespaceValidator;
	let issues = v.validate("f.txt", "hello   \nworld\ntrailing ");
	assert_eq!(issues.len(), 2); // lines 1 and 3
	assert_eq!(issues[0].line, Some(1));
	assert_eq!(issues[1].line, Some(3));
}

#[test]
fn validator_trailing_whitespace_clean() {
	let v = TrailingWhitespaceValidator;
	let issues = v.validate("f.txt", "hello\nworld");
	assert!(issues.is_empty());
}

#[test]
fn validator_mixed_indentation_detects() {
	let v = MixedIndentationValidator;
	let issues = v.validate("f.rs", "\tindented\n  spaced");
	assert_eq!(issues.len(), 1);
	assert_eq!(issues[0].severity, ValidationSeverity::Error);
	assert_eq!(issues[0].code, "mixed_indentation");
}

#[test]
fn validator_mixed_indentation_tabs_only() {
	let v = MixedIndentationValidator;
	let issues = v.validate("f.rs", "\tline1\n\tline2");
	assert!(issues.is_empty());
}

#[test]
fn validator_mixed_indentation_spaces_only() {
	let v = MixedIndentationValidator;
	let issues = v.validate("f.rs", "  line1\n  line2");
	assert!(issues.is_empty());
}

#[test]
fn validator_empty_file() {
	let v = EmptyFileValidator;
	let issues = v.validate("empty.txt", "");
	assert_eq!(issues.len(), 1);
	assert_eq!(issues[0].severity, ValidationSeverity::Warning);
	assert_eq!(issues[0].code, "empty_file");
}

#[test]
fn validator_empty_file_whitespace_only() {
	let v = EmptyFileValidator;
	let issues = v.validate("spaces.txt", "   \n  ");
	assert_eq!(issues.len(), 1);
}

#[test]
fn validator_empty_file_not_empty() {
	let v = EmptyFileValidator;
	let issues = v.validate("ok.txt", "content");
	assert!(issues.is_empty());
}

#[test]
fn validator_mixed_line_endings() {
	let v = MixedLineEndingsValidator;
	let issues = v.validate("mix.txt", "line1\r\nline2\nline3");
	assert_eq!(issues.len(), 1);
	assert_eq!(issues[0].severity, ValidationSeverity::Warning);
	assert_eq!(issues[0].code, "mixed_line_endings");
}

#[test]
fn validator_lf_only() {
	let v = MixedLineEndingsValidator;
	let issues = v.validate("lf.txt", "line1\nline2\nline3");
	assert!(issues.is_empty());
}

#[test]
fn validator_crlf_only() {
	let v = MixedLineEndingsValidator;
	let issues = v.validate("crlf.txt", "line1\r\nline2\r\nline3");
	assert!(issues.is_empty());
}

#[test]
fn validator_missing_trailing_newline() {
	let v = MissingTrailingNewlineValidator;
	let issues = v.validate("f.txt", "content");
	assert_eq!(issues.len(), 1);
	assert_eq!(issues[0].code, "missing_trailing_newline");
}

#[test]
fn validator_has_trailing_newline() {
	let v = MissingTrailingNewlineValidator;
	let issues = v.validate("f.txt", "content\n");
	assert!(issues.is_empty());
}

#[test]
fn validator_missing_newline_empty_file() {
	let v = MissingTrailingNewlineValidator;
	let issues = v.validate("f.txt", "");
	assert!(issues.is_empty()); // Empty files handled by EmptyFileValidator
}

// =========================================================================
// validate_snapshot integration
// =========================================================================

#[test]
fn validate_snapshot_integration() {
	let snap = SnapshotData {
		files: vec![
			SnapshotFile {
				path: "vfs:///good.json".to_string(),
				content_type: "text".to_string(),
				text: Some(r#"{"valid": true}"#.to_string()),
				base64: None,
				created_at: 0,
				modified_at: 0,
			},
			SnapshotFile {
				path: "vfs:///bad.json".to_string(),
				content_type: "text".to_string(),
				text: Some("{invalid}".to_string()),
				base64: None,
				created_at: 0,
				modified_at: 0,
			},
			SnapshotFile {
				path: "vfs:///trailing.txt".to_string(),
				content_type: "text".to_string(),
				text: Some("hello   ".to_string()),
				base64: None,
				created_at: 0,
				modified_at: 0,
			},
		],
		directories: vec![],
	};

	let validators = default_validators();
	let result = validate_snapshot(&snap, &validators);

	assert!(!result.passed); // json error fails it
	assert!(result.errors > 0);
	assert!(result.warnings > 0);
}

#[test]
fn validate_snapshot_skips_binary() {
	let snap = SnapshotData {
		files: vec![SnapshotFile {
			path: "vfs:///image.png".to_string(),
			content_type: "binary".to_string(),
			text: None,
			base64: Some("AAAA".to_string()),
			created_at: 0,
			modified_at: 0,
		}],
		directories: vec![],
	};

	let validators = default_validators();
	let result = validate_snapshot(&snap, &validators);
	assert!(result.passed);
	assert_eq!(result.errors, 0);
	assert_eq!(result.warnings, 0);
}

#[test]
fn validate_snapshot_all_clean() {
	let snap = SnapshotData {
		files: vec![SnapshotFile {
			path: "vfs:///clean.txt".to_string(),
			content_type: "text".to_string(),
			text: Some("clean content\n".to_string()),
			base64: None,
			created_at: 0,
			modified_at: 0,
		}],
		directories: vec![],
	};

	let validators = default_validators();
	let result = validate_snapshot(&snap, &validators);
	assert!(result.passed);
}

#[test]
fn default_validators_count() {
	let v = default_validators();
	assert_eq!(v.len(), 6);
}

// =========================================================================
// VfsExec — mock backend
// =========================================================================

struct MockExecBackend {
	stdout: String,
	exit_code: i32,
}

#[async_trait]
impl ExecBackend for MockExecBackend {
	async fn run(
		&self,
		_command: &str,
		_args: &[String],
		_options: Option<&ExecOptions>,
	) -> Result<ExecResult, SimseError> {
		Ok(ExecResult {
			stdout: self.stdout.clone(),
			stderr: String::new(),
			exit_code: self.exit_code,
			files_changed: Vec::new(),
		})
	}

	async fn dispose(&self) -> Result<(), SimseError> {
		Ok(())
	}
}

#[tokio::test]
async fn vfs_exec_runs_command() {
	let backend = Arc::new(MockExecBackend {
		stdout: "hello world".to_string(),
		exit_code: 0,
	});
	let exec = VfsExec::new(backend);
	let result = exec.run("echo", &["hello".to_string()], None).await.unwrap();
	assert_eq!(result.stdout, "hello world");
	assert_eq!(result.exit_code, 0);
}

#[tokio::test]
async fn vfs_exec_with_options() {
	let backend = Arc::new(MockExecBackend {
		stdout: "output".to_string(),
		exit_code: 42,
	});
	let exec = VfsExec::new(backend);
	let opts = ExecOptions {
		cwd: Some("/tmp".to_string()),
		env: Some(HashMap::from([("KEY".to_string(), "val".to_string())])),
		timeout_ms: Some(5000),
		stdin: None,
	};
	let result = exec
		.run("cmd", &[], Some(&opts))
		.await
		.unwrap();
	assert_eq!(result.exit_code, 42);
}

#[tokio::test]
async fn vfs_exec_dispose() {
	let backend = Arc::new(MockExecBackend {
		stdout: String::new(),
		exit_code: 0,
	});
	let exec = VfsExec::new(backend);
	exec.dispose().await.unwrap();
}

// =========================================================================
// Error propagation
// =========================================================================

#[test]
fn vfs_read_nonexistent_returns_error() {
	let vfs = new_vfs();
	let result = vfs.read_file("vfs:///nonexistent.txt");
	assert!(result.is_err());
}

#[test]
fn vfs_write_to_root_fails() {
	let vfs = new_vfs();
	let result = vfs.write_file("vfs:///", "data", None);
	assert!(result.is_err());
}

#[test]
fn vfs_invalid_path_errors() {
	let vfs = new_vfs();
	let result = vfs.read_file("/no-scheme.txt");
	assert!(result.is_err());
}

// =========================================================================
// VfsDisk commit with validation
// =========================================================================

#[test]
fn vfs_disk_commit_with_validation() {
	let vfs = new_vfs_arc();
	vfs.write_file("vfs:///bad.json", "{invalid}", None)
		.unwrap();
	vfs.write_file("vfs:///good.txt", "hello\n", None).unwrap();

	let tmp = tempfile::tempdir().unwrap();
	let disk = VfsDisk::new(vfs, tmp.path().to_path_buf());
	let validators = default_validators();

	let result = disk
		.commit(
			tmp.path(),
			CommitOptions {
				overwrite: true,
				validate: true,
				..Default::default()
			},
			Some(&validators),
		)
		.unwrap();

	assert!(result.validation.is_some());
	let validation = result.validation.unwrap();
	assert!(!validation.passed); // JSON syntax error
	assert!(validation.errors > 0);
}

// =========================================================================
// VfsDisk commit skip on existing without overwrite
// =========================================================================

#[test]
fn vfs_disk_commit_no_overwrite_skips_existing() {
	let vfs = new_vfs_arc();
	vfs.write_file("vfs:///exists.txt", "new content", None)
		.unwrap();

	let tmp = tempfile::tempdir().unwrap();
	std::fs::write(tmp.path().join("exists.txt"), "old content").unwrap();

	let disk = VfsDisk::new(vfs, tmp.path().to_path_buf());
	let result = disk
		.commit(tmp.path(), CommitOptions::default(), None)
		.unwrap();

	// Should have skipped the file
	let skipped: Vec<_> = result
		.operations
		.iter()
		.filter(|op| op.op_type == CommitOpType::Skip)
		.collect();
	assert!(!skipped.is_empty());

	// Old content should remain unchanged
	let content = std::fs::read_to_string(tmp.path().join("exists.txt")).unwrap();
	assert_eq!(content, "old content");
}

// =========================================================================
// VfsDisk load no overwrite skips existing
// =========================================================================

#[test]
fn vfs_disk_load_no_overwrite_skips_existing() {
	let tmp = tempfile::tempdir().unwrap();
	std::fs::write(tmp.path().join("file.txt"), "disk data").unwrap();

	let vfs = new_vfs_arc();
	// Pre-populate VFS
	vfs.write_file("vfs:///file.txt", "vfs data", None).unwrap();

	let disk = VfsDisk::new(vfs.clone(), tmp.path().to_path_buf());
	let result = disk.load(tmp.path(), LoadOptions::default()).unwrap();

	// File should have been skipped
	let skipped: Vec<_> = result
		.operations
		.iter()
		.filter(|op| op.op_type == CommitOpType::Skip)
		.collect();
	assert!(!skipped.is_empty());

	// VFS content should remain unchanged
	let r = vfs.read_file("vfs:///file.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("vfs data"));
}
