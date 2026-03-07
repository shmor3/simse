// ---------------------------------------------------------------------------
// Integration tests for VirtualFs and DiskFs
//
// Direct Rust API tests (no JSON-RPC). Ported from simse-vfs/tests/integration.rs.
//
// FP pattern: VirtualFs mutation methods take `self` by value and return
// `Self` (or `(Self, T)`). Read-only methods take `&self`.
// ---------------------------------------------------------------------------

use base64::Engine as _;

use simse_sandbox_engine::error::SandboxError;
use simse_sandbox_engine::vfs_disk::{DiskFs, DiskSearchMode, DiskSearchOptions, DiskSearchResult};
use simse_sandbox_engine::vfs_path::VfsLimits;
use simse_sandbox_engine::vfs_store::{SearchOpts, SearchOutput, TransactionOp, VirtualFs};

// ===========================================================================
// VirtualFs tests
// ===========================================================================

fn new_vfs() -> VirtualFs {
	VirtualFs::new(VfsLimits::default(), 50)
}

// ── write and read file ─────────────────────────────────────────────────

#[test]
fn vfs_write_and_read_file() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///hello.txt", "Hello, world!", None, false)
		.unwrap();

	let result = vfs.read_file("vfs:///hello.txt").unwrap();
	assert_eq!(result.content_type, "text");
	assert_eq!(result.text.as_deref(), Some("Hello, world!"));
	assert_eq!(result.size, 13);
}

// ── mkdir and readdir ───────────────────────────────────────────────────

#[test]
fn vfs_mkdir_and_readdir() {
	let vfs = new_vfs();

	let vfs = vfs.mkdir("vfs:///src", false).unwrap();
	let vfs = vfs.mkdir("vfs:///src/utils", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///src/main.ts", "console.log('hi');", None, false)
		.unwrap();

	let entries = vfs.readdir("vfs:///src", false).unwrap();
	assert_eq!(entries.len(), 2);

	let mut names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
	names.sort();
	assert_eq!(names, vec!["main.ts", "utils"]);

	for entry in &entries {
		match entry.name.as_str() {
			"main.ts" => assert_eq!(entry.node_type, "file"),
			"utils" => assert_eq!(entry.node_type, "directory"),
			other => panic!("unexpected entry: {other}"),
		}
	}
}

// ── delete file and exists ──────────────────────────────────────────────

#[test]
fn vfs_delete_file_and_exists() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///temp.txt", "temporary", None, false)
		.unwrap();

	assert!(vfs.exists("vfs:///temp.txt").unwrap());

	let (vfs, deleted) = vfs.delete_file("vfs:///temp.txt").unwrap();
	assert!(deleted);
	assert!(!vfs.exists("vfs:///temp.txt").unwrap());
}

// ── rename file ─────────────────────────────────────────────────────────

#[test]
fn vfs_rename_file() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///old.txt", "content", None, false)
		.unwrap();

	let vfs = vfs.rename("vfs:///old.txt", "vfs:///new.txt").unwrap();

	assert!(!vfs.exists("vfs:///old.txt").unwrap());
	let result = vfs.read_file("vfs:///new.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("content"));
}

// ── search content ──────────────────────────────────────────────────────

#[test]
fn vfs_search_content() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file(
			"vfs:///notes.txt",
			"hello world\nfoo bar\nhello again",
			None,
			false,
		)
		.unwrap();

	let result = vfs
		.search(
			"hello",
			SearchOpts {
				mode: "substring".to_string(),
				..SearchOpts::default()
			},
		)
		.unwrap();

	match result {
		SearchOutput::Results(matches) => {
			assert_eq!(matches.len(), 2);
			for m in &matches {
				assert!(m.match_text.contains("hello"));
			}
		}
		SearchOutput::Count(_) => panic!("expected results, got count"),
	}
}

// ── glob matches ────────────────────────────────────────────────────────

#[test]
fn vfs_glob_matches() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///src/main.ts", "ts", None, true)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///src/lib.ts", "ts", None, true)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///README.md", "md", None, false)
		.unwrap();

	let results = vfs.glob(vec!["vfs:///**/*.ts".to_string()]);
	assert_eq!(results.len(), 2);
	for r in &results {
		assert!(r.ends_with(".ts"));
	}

	// Ensure .md file is not matched
	assert!(!results.iter().any(|r| r.ends_with(".md")));
}

// ── diff two files ──────────────────────────────────────────────────────

#[test]
fn vfs_diff_two_files() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///a.txt", "hello\nworld", None, false)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///b.txt", "hello\nearth", None, false)
		.unwrap();

	let diff = vfs.diff("vfs:///a.txt", "vfs:///b.txt", 3).unwrap();
	assert_eq!(diff.additions, 1);
	assert_eq!(diff.deletions, 1);
	assert!(!diff.hunks.is_empty());
}

// ── snapshot, clear, restore ────────────────────────────────────────────

#[test]
fn vfs_snapshot_clear_restore() {
	let vfs = new_vfs();
	let vfs = vfs.mkdir("vfs:///data", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///data/file.txt", "important", None, false)
		.unwrap();

	let snapshot = vfs.snapshot();

	// Verify state is populated
	assert!(vfs.exists("vfs:///data").unwrap());
	assert!(vfs.exists("vfs:///data/file.txt").unwrap());

	// Clear wipes everything
	let vfs = vfs.clear();
	assert!(!vfs.exists("vfs:///data").unwrap());
	assert!(!vfs.exists("vfs:///data/file.txt").unwrap());
	assert!(vfs.exists("vfs:///").unwrap()); // root always exists

	// Restore brings it back
	let vfs = vfs.restore(snapshot).unwrap();
	assert!(vfs.exists("vfs:///data").unwrap());
	let result = vfs.read_file("vfs:///data/file.txt").unwrap();
	assert_eq!(result.text.as_deref(), Some("important"));
}

// ── transaction rollback on error ───────────────────────────────────────

#[test]
fn vfs_transaction_rollback_on_error() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///existing.txt", "keep me", None, false)
		.unwrap();

	let result = vfs.transaction(vec![
		TransactionOp::WriteFile {
			path: "vfs:///new.txt".to_string(),
			content: "new data".to_string(),
		},
		// This should fail — writing to root as a file
		TransactionOp::WriteFile {
			path: "vfs:///".to_string(),
			content: "bad".to_string(),
		},
	]);

	let vfs = match result {
		Ok(_) => panic!("expected transaction to fail"),
		Err((vfs, _err)) => vfs,
	};
	// new.txt should not exist after rollback
	assert!(!vfs.exists("vfs:///new.txt").unwrap());
	// existing.txt should still be intact
	let r = vfs.read_file("vfs:///existing.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("keep me"));
}

#[test]
fn vfs_transaction_commits_on_success() {
	let vfs = new_vfs();
	let vfs = vfs
		.transaction(vec![
			TransactionOp::Mkdir {
				path: "vfs:///project".to_string(),
			},
			TransactionOp::WriteFile {
				path: "vfs:///project/main.rs".to_string(),
				content: "fn main() {}".to_string(),
			},
		])
		.map_err(|(_vfs, e)| e)
		.unwrap();

	assert!(vfs.exists("vfs:///project").unwrap());
	let r = vfs.read_file("vfs:///project/main.rs").unwrap();
	assert_eq!(r.text.as_deref(), Some("fn main() {}"));
}

// ── limits exceeded ─────────────────────────────────────────────────────

#[test]
fn vfs_file_size_limit_exceeded() {
	let limits = VfsLimits {
		max_file_size: 10,
		..VfsLimits::default()
	};
	let vfs = VirtualFs::new(limits, 50);

	let err = vfs
		.write_file("vfs:///big.txt", "12345678901", None, false)
		.unwrap_err();
	assert!(matches!(err, SandboxError::VfsLimitExceeded(_)));
}

#[test]
fn vfs_total_size_limit_exceeded() {
	let limits = VfsLimits {
		max_total_size: 10,
		..VfsLimits::default()
	};
	let vfs = VirtualFs::new(limits, 50);

	let vfs = vfs
		.write_file("vfs:///a.txt", "12345", None, false)
		.unwrap();
	let err = vfs
		.write_file("vfs:///b.txt", "123456", None, false)
		.unwrap_err();
	assert!(matches!(err, SandboxError::VfsLimitExceeded(_)));
}

#[test]
fn vfs_node_count_limit_exceeded() {
	let limits = VfsLimits {
		max_node_count: 3, // root + 2 nodes max
		..VfsLimits::default()
	};
	let vfs = VirtualFs::new(limits, 50);

	let vfs = vfs
		.write_file("vfs:///a.txt", "a", None, false)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///b.txt", "b", None, false)
		.unwrap();
	let err = vfs
		.write_file("vfs:///c.txt", "c", None, false)
		.unwrap_err();
	assert!(matches!(err, SandboxError::VfsLimitExceeded(_)));
}

// ── additional VirtualFs tests ──────────────────────────────────────────

#[test]
fn vfs_stat_file_and_directory() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///f.txt", "hello", None, false)
		.unwrap();
	let vfs = vfs.mkdir("vfs:///dir", false).unwrap();

	let stat_f = vfs.stat("vfs:///f.txt").unwrap();
	assert_eq!(stat_f.node_type, "file");
	assert_eq!(stat_f.size, 5);
	assert!(stat_f.created_at > 0);

	let stat_d = vfs.stat("vfs:///dir").unwrap();
	assert_eq!(stat_d.node_type, "directory");
}

#[test]
fn vfs_stat_not_found() {
	let vfs = new_vfs();
	let err = vfs.stat("vfs:///nope").unwrap_err();
	assert!(matches!(err, SandboxError::VfsNotFound(_)));
}

#[test]
fn vfs_history_tracks_versions() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///f.txt", "v1", None, false)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///f.txt", "v2", None, false)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///f.txt", "v3", None, false)
		.unwrap();

	let history = vfs.history("vfs:///f.txt").unwrap();
	// v1 and v2 are in history (v3 is current)
	assert_eq!(history.len(), 2);
	assert_eq!(history[0].text.as_deref(), Some("v1"));
	assert_eq!(history[1].text.as_deref(), Some("v2"));
}

#[test]
fn vfs_diff_versions() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///f.txt", "line1\nline2", None, false)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///f.txt", "line1\nline2\nline3", None, false)
		.unwrap();

	// Diff version 1 (history) against current (version 2)
	let diff = vfs.diff_versions("vfs:///f.txt", 1, None, 3).unwrap();
	assert_eq!(diff.additions, 1);
	assert_eq!(diff.deletions, 0);
}

#[test]
fn vfs_checkout_to_previous_version() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///f.txt", "version one", None, false)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///f.txt", "version two", None, false)
		.unwrap();

	let vfs = vfs.checkout("vfs:///f.txt", 1).unwrap();
	let r = vfs.read_file("vfs:///f.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("version one"));
}

#[test]
fn vfs_tree_output() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///a.txt", "a", None, false)
		.unwrap();
	let vfs = vfs.mkdir("vfs:///dir", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///dir/b.txt", "bb", None, false)
		.unwrap();

	let tree = vfs.tree(None).unwrap();
	assert!(tree.contains("a.txt"));
	assert!(tree.contains("dir/"));
	assert!(tree.contains("b.txt"));
}

#[test]
fn vfs_du_file_and_directory() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///f.txt", "hello", None, false)
		.unwrap();
	assert_eq!(vfs.du("vfs:///f.txt").unwrap(), 5);

	let vfs = vfs.mkdir("vfs:///d", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///d/a.txt", "aaa", None, false)
		.unwrap();
	let vfs = vfs
		.write_file("vfs:///d/b.txt", "bb", None, false)
		.unwrap();
	assert_eq!(vfs.du("vfs:///d").unwrap(), 5);
}

#[test]
fn vfs_metrics_track_counts() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///a.txt", "aaa", None, false)
		.unwrap();
	let vfs = vfs.mkdir("vfs:///dir", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///dir/b.txt", "bb", None, false)
		.unwrap();

	let m = vfs.metrics();
	assert_eq!(m.total_size, 5); // 3 + 2
	assert_eq!(m.file_count, 2);
	assert_eq!(m.directory_count, 2); // root + dir
	assert_eq!(m.node_count, 4); // root + dir + a.txt + b.txt
}

#[test]
fn vfs_events_are_emitted() {
	use simse_sandbox_engine::vfs_store::VfsEvent;

	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///f.txt", "x", None, false)
		.unwrap();
	let vfs = vfs.mkdir("vfs:///dir", false).unwrap();
	let (vfs, _deleted) = vfs.delete_file("vfs:///f.txt").unwrap();

	let (vfs, events) = vfs.drain_events();
	assert_eq!(events.len(), 3);
	assert!(matches!(&events[0], VfsEvent::Write { .. }));
	assert!(matches!(&events[1], VfsEvent::Mkdir { .. }));
	assert!(matches!(&events[2], VfsEvent::Delete { .. }));

	// Second drain should be empty
	let (_vfs, events2) = vfs.drain_events();
	assert!(events2.is_empty());
}

#[test]
fn vfs_append_file() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///log.txt", "line1\n", None, false)
		.unwrap();
	let vfs = vfs.append_file("vfs:///log.txt", "line2\n").unwrap();

	let r = vfs.read_file("vfs:///log.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("line1\nline2\n"));
}

#[test]
fn vfs_write_with_create_parents() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///a/b/c/deep.txt", "deep content", None, true)
		.unwrap();

	assert!(vfs.exists("vfs:///a").unwrap());
	assert!(vfs.exists("vfs:///a/b").unwrap());
	assert!(vfs.exists("vfs:///a/b/c").unwrap());

	let r = vfs.read_file("vfs:///a/b/c/deep.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("deep content"));
}

#[test]
fn vfs_rmdir_empty() {
	let vfs = new_vfs();
	let vfs = vfs.mkdir("vfs:///dir", false).unwrap();
	let (vfs, removed) = vfs.rmdir("vfs:///dir", false).unwrap();
	assert!(removed);
	assert!(!vfs.exists("vfs:///dir").unwrap());
}

#[test]
fn vfs_rmdir_nonempty_fails_without_recursive() {
	let vfs = new_vfs();
	let vfs = vfs.mkdir("vfs:///dir", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///dir/f.txt", "x", None, false)
		.unwrap();

	let err = vfs.rmdir("vfs:///dir", false).unwrap_err();
	assert!(matches!(err, SandboxError::VfsNotEmpty(_)));
}

#[test]
fn vfs_rmdir_recursive() {
	let vfs = new_vfs();
	let vfs = vfs.mkdir("vfs:///dir", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///dir/f.txt", "x", None, false)
		.unwrap();
	let (vfs, removed) = vfs.rmdir("vfs:///dir", true).unwrap();
	assert!(removed);
	assert!(!vfs.exists("vfs:///dir").unwrap());
	assert!(!vfs.exists("vfs:///dir/f.txt").unwrap());
}

#[test]
fn vfs_rename_directory_with_descendants() {
	let vfs = new_vfs();
	let vfs = vfs.mkdir("vfs:///src", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///src/a.txt", "a", None, false)
		.unwrap();

	let vfs = vfs.rename("vfs:///src", "vfs:///dst").unwrap();

	assert!(!vfs.exists("vfs:///src").unwrap());
	assert!(vfs.exists("vfs:///dst").unwrap());
	assert!(vfs.exists("vfs:///dst/a.txt").unwrap());
}

#[test]
fn vfs_copy_file() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///src.txt", "data", None, false)
		.unwrap();
	let vfs = vfs
		.copy("vfs:///src.txt", "vfs:///dst.txt", false, false)
		.unwrap();

	let r = vfs.read_file("vfs:///dst.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("data"));
	// Source still exists
	assert!(vfs.exists("vfs:///src.txt").unwrap());
}

#[test]
fn vfs_copy_directory_recursive() {
	let vfs = new_vfs();
	let vfs = vfs.mkdir("vfs:///src", false).unwrap();
	let vfs = vfs
		.write_file("vfs:///src/a.txt", "a", None, false)
		.unwrap();
	let vfs = vfs
		.copy("vfs:///src", "vfs:///dst", false, true)
		.unwrap();

	assert!(vfs.exists("vfs:///dst").unwrap());
	let r = vfs.read_file("vfs:///dst/a.txt").unwrap();
	assert_eq!(r.text.as_deref(), Some("a"));
}

#[test]
fn vfs_search_with_regex() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file(
			"vfs:///code.rs",
			"fn main() {}\nfn helper() {}",
			None,
			false,
		)
		.unwrap();

	let result = vfs
		.search(
			r"fn \w+\(\)",
			SearchOpts {
				mode: "regex".to_string(),
				..SearchOpts::default()
			},
		)
		.unwrap();

	match result {
		SearchOutput::Results(matches) => {
			assert_eq!(matches.len(), 2);
		}
		SearchOutput::Count(_) => panic!("expected results, got count"),
	}
}

#[test]
fn vfs_search_count_only() {
	let vfs = new_vfs();
	let vfs = vfs
		.write_file("vfs:///f.txt", "aaa\nbbb\naaa", None, false)
		.unwrap();

	let result = vfs
		.search(
			"aaa",
			SearchOpts {
				count_only: true,
				..SearchOpts::default()
			},
		)
		.unwrap();

	match result {
		SearchOutput::Count(c) => assert_eq!(c, 2),
		SearchOutput::Results(_) => panic!("expected count, got results"),
	}
}

#[test]
fn vfs_binary_write_and_read() {
	let vfs = new_vfs();
	let data = vec![0u8, 1, 2, 255];
	let b64 = base64::engine::general_purpose::STANDARD.encode(&data);

	let vfs = vfs
		.write_file("vfs:///bin.dat", &b64, Some("binary"), false)
		.unwrap();

	let r = vfs.read_file("vfs:///bin.dat").unwrap();
	assert_eq!(r.content_type, "binary");
	let decoded = base64::engine::general_purpose::STANDARD
		.decode(r.data_base64.unwrap())
		.unwrap();
	assert_eq!(decoded, data);
}

// ===========================================================================
// DiskFs tests
// ===========================================================================

fn temp_disk() -> (tempfile::TempDir, DiskFs) {
	let tmp = tempfile::tempdir().expect("failed to create tempdir");
	let disk = DiskFs::new(tmp.path().to_path_buf(), vec![], 50);
	(tmp, disk)
}

fn file_uri(rel: &str) -> String {
	format!("file:///{}", rel.trim_start_matches('/'))
}

// ── write and read file (text + binary detection) ───────────────────────

#[test]
fn disk_write_and_read_text_file() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("hello.txt");

	disk.write_file(&path, "Hello, world!", None, false)
		.unwrap();

	let result = disk.read_file(&path).unwrap();
	assert_eq!(result.content_type, "text");
	assert_eq!(result.text.as_deref(), Some("Hello, world!"));
	assert_eq!(result.size, 13);
}

#[test]
fn disk_write_and_read_binary_file() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("data.bin");

	let raw = b"\x00\x01\x02\x03";
	let b64 = base64::engine::general_purpose::STANDARD.encode(raw);
	disk.write_file(&path, &b64, Some("binary"), false)
		.unwrap();

	let result = disk.read_file(&path).unwrap();
	assert_eq!(result.content_type, "binary");
	assert!(result.text.is_none());
	assert_eq!(result.data.as_deref(), Some(b64.as_str()));
}

// ── append file ─────────────────────────────────────────────────────────

#[test]
fn disk_append_file() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("log.txt");

	disk.write_file(&path, "line1\n", None, false).unwrap();
	disk.append_file(&path, "line2\n").unwrap();

	let result = disk.read_file(&path).unwrap();
	assert_eq!(result.text.as_deref(), Some("line1\nline2\n"));
}

// ── delete file ─────────────────────────────────────────────────────────

#[test]
fn disk_delete_file() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("temp.txt");

	disk.write_file(&path, "temp", None, false).unwrap();
	assert!(disk.exists(&path).unwrap());

	disk.delete_file(&path).unwrap();
	assert!(!disk.exists(&path).unwrap());
}

// ── mkdir and readdir (recursive) ───────────────────────────────────────

#[test]
fn disk_mkdir_and_readdir() {
	let (_tmp, disk) = temp_disk();
	let dir = file_uri("stuff");
	disk.mkdir(&dir, false).unwrap();

	disk.write_file(&file_uri("stuff/a.txt"), "a", None, false)
		.unwrap();
	disk.write_file(&file_uri("stuff/b.txt"), "b", None, false)
		.unwrap();

	let entries = disk.readdir(&dir, false).unwrap();
	assert_eq!(entries.len(), 2);

	let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
	assert!(names.contains(&"a.txt"));
	assert!(names.contains(&"b.txt"));
}

#[test]
fn disk_readdir_recursive() {
	let (_tmp, disk) = temp_disk();
	disk.write_file(&file_uri("r/a.txt"), "a", None, true)
		.unwrap();
	disk.write_file(&file_uri("r/sub/b.txt"), "b", None, true)
		.unwrap();

	let entries = disk.readdir(&file_uri("r"), true).unwrap();
	// Should have at least: a.txt, sub, sub/b.txt
	assert!(entries.len() >= 3);
}

// ── rmdir ───────────────────────────────────────────────────────────────

#[test]
fn disk_rmdir_empty() {
	let (_tmp, disk) = temp_disk();
	let dir = file_uri("empty_dir");
	disk.mkdir(&dir, false).unwrap();
	disk.rmdir(&dir, false).unwrap();
	assert!(!disk.exists(&dir).unwrap());
}

#[test]
fn disk_rmdir_recursive() {
	let (_tmp, disk) = temp_disk();
	disk.write_file(&file_uri("rm_me/sub/file.txt"), "x", None, true)
		.unwrap();
	disk.rmdir(&file_uri("rm_me"), true).unwrap();
	assert!(!disk.exists(&file_uri("rm_me")).unwrap());
}

// ── stat and exists ─────────────────────────────────────────────────────

#[test]
fn disk_stat_file() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("info.txt");
	disk.write_file(&path, "data", None, false).unwrap();

	let stat = disk.stat(&path).unwrap();
	assert_eq!(stat.node_type, "file");
	assert_eq!(stat.size, 4);
	assert!(stat.modified_at > 0);
}

#[test]
fn disk_stat_directory() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("mydir");
	disk.mkdir(&path, false).unwrap();

	let stat = disk.stat(&path).unwrap();
	assert_eq!(stat.node_type, "directory");
}

#[test]
fn disk_exists_returns_false_for_missing() {
	let (_tmp, disk) = temp_disk();
	assert!(!disk.exists(&file_uri("nope.txt")).unwrap());
}

// ── rename ──────────────────────────────────────────────────────────────

#[test]
fn disk_rename_file() {
	let (_tmp, disk) = temp_disk();
	let old = file_uri("old.txt");
	let new = file_uri("new.txt");

	disk.write_file(&old, "content", None, false).unwrap();
	disk.rename(&old, &new).unwrap();

	assert!(!disk.exists(&old).unwrap());
	let result = disk.read_file(&new).unwrap();
	assert_eq!(result.text.as_deref(), Some("content"));
}

// ── copy ────────────────────────────────────────────────────────────────

#[test]
fn disk_copy_file() {
	let (_tmp, disk) = temp_disk();
	let src = file_uri("src.txt");
	let dest = file_uri("dest.txt");

	disk.write_file(&src, "copy me", None, false).unwrap();
	disk.copy(&src, &dest, false, false).unwrap();

	let result = disk.read_file(&dest).unwrap();
	assert_eq!(result.text.as_deref(), Some("copy me"));
	// Source still exists
	assert!(disk.exists(&src).unwrap());
}

#[test]
fn disk_copy_no_overwrite() {
	let (_tmp, disk) = temp_disk();
	let src = file_uri("s.txt");
	let dest = file_uri("d.txt");

	disk.write_file(&src, "a", None, false).unwrap();
	disk.write_file(&dest, "b", None, false).unwrap();

	let err = disk.copy(&src, &dest, false, false).unwrap_err();
	assert!(matches!(err, SandboxError::VfsAlreadyExists(_)));
}

// ── glob ────────────────────────────────────────────────────────────────

#[test]
fn disk_glob_finds_files() {
	let (_tmp, disk) = temp_disk();
	disk.write_file(&file_uri("src/main.rs"), "fn main", None, true)
		.unwrap();
	disk.write_file(&file_uri("src/lib.rs"), "pub mod", None, true)
		.unwrap();
	disk.write_file(&file_uri("README.md"), "# Readme", None, false)
		.unwrap();

	let matches = disk.glob(&["**/*.rs".to_string()]).unwrap();
	assert_eq!(matches.len(), 2);
	for m in &matches {
		assert!(m.ends_with(".rs"));
	}
}

// ── tree ────────────────────────────────────────────────────────────────

#[test]
fn disk_tree_output() {
	let (_tmp, disk) = temp_disk();
	disk.write_file(&file_uri("t/a.txt"), "a", None, true)
		.unwrap();
	disk.write_file(&file_uri("t/sub/b.txt"), "b", None, true)
		.unwrap();

	let tree = disk.tree(&file_uri("t")).unwrap();
	assert!(tree.contains("a.txt"));
	assert!(tree.contains("sub"));
	assert!(tree.contains("b.txt"));
}

// ── du ──────────────────────────────────────────────────────────────────

#[test]
fn disk_du_calculates_size() {
	let (_tmp, disk) = temp_disk();
	disk.write_file(&file_uri("du/a.txt"), "aaaa", None, true)
		.unwrap();
	disk.write_file(&file_uri("du/b.txt"), "bb", None, true)
		.unwrap();

	let total = disk.du(&file_uri("du")).unwrap();
	assert_eq!(total, 6); // 4 + 2
}

// ── search ──────────────────────────────────────────────────────────────

#[test]
fn disk_search_finds_text() {
	let (_tmp, disk) = temp_disk();
	disk.write_file(
		&file_uri("s/hello.txt"),
		"hello world\nfoo bar\nhello again",
		None,
		true,
	)
	.unwrap();
	disk.write_file(&file_uri("s/other.txt"), "no match here", None, true)
		.unwrap();

	let opts = DiskSearchOptions::default();
	let result = disk.search(&file_uri("s"), "hello", &opts).unwrap();

	match result {
		DiskSearchResult::Matches(matches) => {
			assert_eq!(matches.len(), 2);
			assert!(matches.iter().all(|m| m.path.contains("hello.txt")));
		}
		DiskSearchResult::Count(_) => panic!("expected matches, got count"),
	}
}

#[test]
fn disk_search_count_only() {
	let (_tmp, disk) = temp_disk();
	disk.write_file(&file_uri("c/f.txt"), "aaa\nbbb\naaa", None, true)
		.unwrap();

	let opts = DiskSearchOptions {
		count_only: true,
		..Default::default()
	};
	let result = disk.search(&file_uri("c"), "aaa", &opts).unwrap();

	match result {
		DiskSearchResult::Count(c) => assert_eq!(c, 2),
		DiskSearchResult::Matches(_) => panic!("expected count, got matches"),
	}
}

// ── sandbox violation (path outside root) ───────────────────────────────

#[test]
fn disk_sandbox_prevents_traversal() {
	let (_tmp, disk) = temp_disk();
	let result = disk.exists("file:///../../../etc/passwd");
	// Either an error or false is acceptable — the key thing is it does not succeed
	match result {
		Ok(false) => {} // path resolved but doesn't exist in sandbox
		Err(_) => {}    // sandbox violation caught
		Ok(true) => panic!("sandbox should have prevented access outside root"),
	}
}

#[test]
fn disk_sandbox_prevents_read_outside_root() {
	let (_tmp, disk) = temp_disk();
	let result = disk.read_file("file:///../../../etc/passwd");
	assert!(result.is_err());
}

// ── history tracks writes ───────────────────────────────────────────────

#[test]
fn disk_history_tracks_writes() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("versioned.txt");

	disk.write_file(&path, "v1", None, false).unwrap();
	disk.write_file(&path, "v2", None, false).unwrap();
	disk.write_file(&path, "v3", None, false).unwrap();

	let history = disk.history(&path).unwrap();
	// v1 and v2 should be in history (v3 is current on disk)
	assert_eq!(history.len(), 2);
	assert_eq!(history[0].text.as_deref(), Some("v1"));
	assert_eq!(history[1].text.as_deref(), Some("v2"));
}

// ── diff two files ──────────────────────────────────────────────────────

#[test]
fn disk_diff_two_files() {
	let (_tmp, disk) = temp_disk();
	let a = file_uri("a.txt");
	let b = file_uri("b.txt");

	disk.write_file(&a, "hello\nworld", None, false).unwrap();
	disk.write_file(&b, "hello\nearth", None, false).unwrap();

	let diff = disk.diff(&a, &b, 3).unwrap();
	assert_eq!(diff.additions, 1);
	assert_eq!(diff.deletions, 1);
	assert!(!diff.hunks.is_empty());
}

// ── diff versions ───────────────────────────────────────────────────────

#[test]
fn disk_diff_versions() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("diffme.txt");

	disk.write_file(&path, "line1\nline2", None, false).unwrap();
	disk.write_file(&path, "line1\nline2\nline3", None, false)
		.unwrap();

	// Diff version 1 (from history) against current file
	let diff = disk.diff_versions(&path, 1, None, 3).unwrap();
	assert_eq!(diff.additions, 1);
	assert_eq!(diff.deletions, 0);
}

// ── checkout to previous version ────────────────────────────────────────

#[test]
fn disk_checkout_to_previous_version() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("checkout.txt");

	disk.write_file(&path, "version one", None, false).unwrap();
	disk.write_file(&path, "version two", None, false).unwrap();

	// History has version 1 = "version one"
	// Current file is "version two"
	disk.checkout(&path, 1).unwrap();

	let r = disk.read_file(&path).unwrap();
	assert_eq!(r.text.as_deref(), Some("version one"));
}

// ── write with create_parents ───────────────────────────────────────────

#[test]
fn disk_write_with_create_parents() {
	let (_tmp, disk) = temp_disk();
	let path = file_uri("a/b/c/deep.txt");

	disk.write_file(&path, "deep content", None, true).unwrap();

	let result = disk.read_file(&path).unwrap();
	assert_eq!(result.text.as_deref(), Some("deep content"));
}

// ── search with regex ───────────────────────────────────────────────────

#[test]
fn disk_search_with_regex() {
	let (_tmp, disk) = temp_disk();
	disk.write_file(
		&file_uri("code/main.rs"),
		"fn main() {}\nfn helper() {}",
		None,
		true,
	)
	.unwrap();

	let opts = DiskSearchOptions {
		mode: DiskSearchMode::Regex,
		..Default::default()
	};
	let result = disk
		.search(&file_uri("code"), r"fn \w+\(\)", &opts)
		.unwrap();

	match result {
		DiskSearchResult::Matches(matches) => {
			assert_eq!(matches.len(), 2);
		}
		DiskSearchResult::Count(_) => panic!("expected matches, got count"),
	}
}
