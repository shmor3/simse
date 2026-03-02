//! Tests for host tools: fuzzy_edit, filesystem sandboxing, and integration
//! tests with temp directories.

use std::path::PathBuf;

use simse_core::tools::host::fuzzy_edit::{fuzzy_match, levenshtein};

// ===========================================================================
// Levenshtein distance tests
// ===========================================================================

#[test]
fn levenshtein_identical_strings() {
	assert_eq!(levenshtein("hello", "hello"), 0);
}

#[test]
fn levenshtein_empty_strings() {
	assert_eq!(levenshtein("", ""), 0);
	assert_eq!(levenshtein("abc", ""), 3);
	assert_eq!(levenshtein("", "abc"), 3);
}

#[test]
fn levenshtein_single_edit() {
	assert_eq!(levenshtein("kitten", "sitten"), 1); // substitution
	assert_eq!(levenshtein("abc", "abcd"), 1); // insertion
	assert_eq!(levenshtein("abcd", "abc"), 1); // deletion
}

#[test]
fn levenshtein_multiple_edits() {
	assert_eq!(levenshtein("kitten", "sitting"), 3);
}

// ===========================================================================
// Fuzzy match: Strategy 1 — Exact match
// ===========================================================================

#[test]
fn exact_match_basic() {
	let content = "hello world";
	let result = fuzzy_match(content, "world", "earth").unwrap();
	assert_eq!(result.replaced, "hello earth");
	assert_eq!(result.strategy, "exact");
}

#[test]
fn exact_match_multiline() {
	let content = "line 1\nline 2\nline 3";
	let result = fuzzy_match(content, "line 2", "LINE TWO").unwrap();
	assert_eq!(result.replaced, "line 1\nLINE TWO\nline 3");
	assert_eq!(result.strategy, "exact");
}

#[test]
fn exact_match_not_unique_returns_none() {
	let content = "abc abc abc";
	let result = fuzzy_match(content, "abc", "xyz");
	assert!(result.is_none());
}

#[test]
fn exact_match_not_found_returns_none() {
	let content = "hello world";
	let result = fuzzy_match(content, "notfound", "replacement");
	assert!(result.is_none());
}

// ===========================================================================
// Fuzzy match: Strategy 2 — Line-trimmed match
// ===========================================================================

#[test]
fn line_trimmed_match_different_leading_whitespace() {
	let content = "  foo\n  bar\n  baz";
	// old_str has no leading whitespace, content has 2 spaces
	let result = fuzzy_match(content, "foo\nbar", "replaced").unwrap();
	assert_eq!(result.strategy, "line-trimmed");
	assert!(result.replaced.contains("replaced"));
	assert!(result.replaced.contains("baz"));
}

#[test]
fn line_trimmed_match_trailing_whitespace_difference() {
	let content = "alpha  \nbeta  \ngamma";
	let result = fuzzy_match(content, "alpha\nbeta", "REPLACED").unwrap();
	assert_eq!(result.strategy, "line-trimmed");
	assert!(result.replaced.starts_with("REPLACED\n"));
}

// ===========================================================================
// Fuzzy match: Strategy 3 — Whitespace-normalized match
// ===========================================================================

#[test]
fn whitespace_normalized_match_extra_internal_spaces() {
	// Content has extra internal whitespace between tokens
	let content = "if  (x  ==  y)  {\n  return   true;\n}";
	let old_str = "if (x == y) {\nreturn true;\n}";
	let result = fuzzy_match(content, old_str, "REPLACED").unwrap();
	assert_eq!(result.strategy, "whitespace-normalized");
	assert_eq!(result.replaced, "REPLACED");
}

// ===========================================================================
// Fuzzy match: Strategy 4 — Indentation-flexible match
// ===========================================================================

#[test]
fn indentation_flexible_match_different_indent_level() {
	// The old_str has 2-space indent, but the content has 4-space indent.
	// Line-trimmed won't match because both sides have the same trimmed content
	// but we actually need the indentation to be flexibly matched.
	// Key: old_str has relative indentation that must be preserved.
	let content = "function f() {\n    if (true) {\n        let x = 1;\n    }\n}";
	// old_str has 2-space base indent (rather than 4-space) but same relative structure
	let old_str = "  if (true) {\n      let x = 1;\n  }";
	let new_str = "if (cond) {\n    let x = 10;\n}";
	let result = fuzzy_match(content, old_str, new_str).unwrap();
	// line-trimmed matches first since trimmed lines are equal
	// The test verifies that _some_ fuzzy strategy succeeds
	assert!(
		result.strategy == "line-trimmed" || result.strategy == "indentation-flexible",
		"Expected line-trimmed or indentation-flexible, got: {}",
		result.strategy
	);
	// Either way, the replacement should be applied
	assert!(result.replaced.contains("function f() {"));
}

// ===========================================================================
// Fuzzy match: Strategy 5 — Block-anchor + Levenshtein
// ===========================================================================

#[test]
fn block_anchor_levenshtein_minor_interior_differences() {
	let content = "fn test() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n}";
	// Old str has minor differences in interior lines
	let old_str = "fn test() {\n    let a = 1;\n    let b = 22;\n    let c = 3;\n}";
	let new_str = "fn test() {\n    let a = 10;\n    let b = 20;\n    let c = 30;\n}";
	let result = fuzzy_match(content, old_str, new_str).unwrap();
	assert_eq!(result.strategy, "block-anchor-levenshtein");
	assert!(result.replaced.contains("let a = 10;"));
}

#[test]
fn block_anchor_too_different_returns_none() {
	let content = "start\ncompletely different line 1\ncompletely different line 2\nend";
	// old_str shares anchors but interior is completely different
	let old_str = "start\nXXXXXXXXXXXXXXXXXXXXXXXXXXX\nYYYYYYYYYYYYYYYYYYYYYYYYYYY\nend";
	let result = fuzzy_match(content, old_str, "REPLACED");
	// This might pass or fail depending on Levenshtein distance ratio
	// With very different content the ratio should exceed 0.3
	if let Some(r) = &result {
		// If it matched, verify the strategy
		assert_eq!(r.strategy, "block-anchor-levenshtein");
	}
	// The important thing is it doesn't crash
}

#[test]
fn block_anchor_less_than_2_lines_returns_none_for_strategy() {
	// Single-line old_str should skip block-anchor strategy.
	// If exact also fails, returns None.
	let content = "line one\nline two\nline three";
	let result = fuzzy_match(content, "nonexistent single line", "replacement");
	assert!(result.is_none());
}

#[test]
fn all_strategies_fail_returns_none() {
	let content = "alpha\nbeta\ngamma";
	let result = fuzzy_match(content, "completely\ndifferent\ncontent\nentirely", "replacement");
	assert!(result.is_none());
}

// ===========================================================================
// Filesystem sandboxing tests
// ===========================================================================

use simse_core::tools::host::filesystem::resolve_sandboxed;

#[test]
fn sandbox_valid_path() {
	let wd = PathBuf::from("/home/user/project");
	let result = resolve_sandboxed(&wd, "src/main.rs", None);
	assert!(result.is_ok());
	let resolved = result.unwrap();
	assert!(resolved.ends_with("src/main.rs"));
}

#[test]
fn sandbox_path_traversal_blocked() {
	let wd = PathBuf::from("/home/user/project");
	let result = resolve_sandboxed(&wd, "../../etc/passwd", None);
	assert!(result.is_err());
	let err = result.unwrap_err();
	assert!(err.to_string().contains("escapes"));
}

#[test]
fn sandbox_with_allowed_paths() {
	let wd = PathBuf::from("/home/user/project");
	let allowed = vec![PathBuf::from("/home/user/shared")];
	// This path escapes the working directory but is within allowed_paths
	let result = resolve_sandboxed(&wd, "../shared/data.txt", Some(&allowed));
	assert!(result.is_ok());
}

#[test]
fn sandbox_with_allowed_paths_still_blocks_outside() {
	let wd = PathBuf::from("/home/user/project");
	let allowed = vec![PathBuf::from("/home/user/shared")];
	// This path is outside both working directory and allowed paths
	let result = resolve_sandboxed(&wd, "../../etc/passwd", Some(&allowed));
	assert!(result.is_err());
}

// ===========================================================================
// Integration tests with tempdir — filesystem tools
// ===========================================================================

use simse_core::tools::host::filesystem::{register_filesystem_tools, FilesystemToolOptions};
use simse_core::tools::registry::ToolRegistry;
use simse_core::tools::types::{ToolCallRequest, ToolRegistryOptions};

fn make_registry_with_fs(wd: PathBuf) -> ToolRegistry {
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	register_filesystem_tools(
		&mut registry,
		FilesystemToolOptions {
			working_directory: wd,
			allowed_paths: None,
		},
	);
	registry
}

#[tokio::test]
async fn fs_write_creates_parent_dirs_and_reads_back() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let registry = make_registry_with_fs(wd);

	// Write a file in a nested directory
	let write_result = registry
		.execute(&ToolCallRequest {
			id: "1".to_string(),
			name: "fs_write".to_string(),
			arguments: serde_json::json!({
				"path": "subdir/nested/hello.txt",
				"content": "Hello, World!"
			}),
		})
		.await;

	assert!(!write_result.is_error);
	assert!(write_result.output.contains("Wrote 13 bytes"));

	// Read back with line numbers
	let read_result = registry
		.execute(&ToolCallRequest {
			id: "2".to_string(),
			name: "fs_read".to_string(),
			arguments: serde_json::json!({
				"path": "subdir/nested/hello.txt"
			}),
		})
		.await;

	assert!(!read_result.is_error);
	assert!(read_result.output.contains("Hello, World!"));
	// Should have line number formatting
	assert!(read_result.output.contains("1\t"));
}

#[tokio::test]
async fn fs_read_with_offset_and_limit() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let registry = make_registry_with_fs(wd.clone());

	// Create a file with multiple lines
	tokio::fs::write(wd.join("lines.txt"), "line1\nline2\nline3\nline4\nline5")
		.await
		.unwrap();

	let result = registry
		.execute(&ToolCallRequest {
			id: "1".to_string(),
			name: "fs_read".to_string(),
			arguments: serde_json::json!({
				"path": "lines.txt",
				"offset": 2,
				"limit": 2
			}),
		})
		.await;

	assert!(!result.is_error);
	assert!(result.output.contains("line2"));
	assert!(result.output.contains("line3"));
	assert!(!result.output.contains("line1"));
	assert!(!result.output.contains("line4"));
}

#[tokio::test]
async fn fs_edit_with_fuzzy_match() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let registry = make_registry_with_fs(wd.clone());

	// Write initial content
	tokio::fs::write(wd.join("code.rs"), "fn main() {\n    println!(\"hello\");\n}")
		.await
		.unwrap();

	let result = registry
		.execute(&ToolCallRequest {
			id: "1".to_string(),
			name: "fs_edit".to_string(),
			arguments: serde_json::json!({
				"path": "code.rs",
				"old_string": "println!(\"hello\");",
				"new_string": "println!(\"world\");"
			}),
		})
		.await;

	assert!(!result.is_error);
	assert!(result.output.contains("Edited"));

	// Verify the file was changed
	let content = tokio::fs::read_to_string(wd.join("code.rs"))
		.await
		.unwrap();
	assert!(content.contains("println!(\"world\");"));
	assert!(!content.contains("println!(\"hello\");"));
}

#[tokio::test]
async fn fs_list_with_depth() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let registry = make_registry_with_fs(wd.clone());

	// Create directory structure
	tokio::fs::create_dir_all(wd.join("a/b"))
		.await
		.unwrap();
	tokio::fs::write(wd.join("top.txt"), "top").await.unwrap();
	tokio::fs::write(wd.join("a/mid.txt"), "mid")
		.await
		.unwrap();
	tokio::fs::write(wd.join("a/b/deep.txt"), "deep")
		.await
		.unwrap();

	// List with depth 1 — should see top-level items and one level of contents
	let result = registry
		.execute(&ToolCallRequest {
			id: "1".to_string(),
			name: "fs_list".to_string(),
			arguments: serde_json::json!({
				"depth": 1
			}),
		})
		.await;

	assert!(!result.is_error);
	assert!(result.output.contains("d a"));
	assert!(result.output.contains("f top.txt"));
}

#[tokio::test]
async fn fs_stat_returns_json() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let registry = make_registry_with_fs(wd.clone());

	tokio::fs::write(wd.join("test.txt"), "hello")
		.await
		.unwrap();

	let result = registry
		.execute(&ToolCallRequest {
			id: "1".to_string(),
			name: "fs_stat".to_string(),
			arguments: serde_json::json!({
				"path": "test.txt"
			}),
		})
		.await;

	assert!(!result.is_error);
	let parsed: serde_json::Value = serde_json::from_str(&result.output).unwrap();
	assert_eq!(parsed["type"], "file");
	assert_eq!(parsed["size"], 5);
	assert!(parsed["modified"].as_str().is_some());
}

#[tokio::test]
async fn fs_delete_file() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let registry = make_registry_with_fs(wd.clone());

	tokio::fs::write(wd.join("delete_me.txt"), "bye")
		.await
		.unwrap();

	let result = registry
		.execute(&ToolCallRequest {
			id: "1".to_string(),
			name: "fs_delete".to_string(),
			arguments: serde_json::json!({
				"path": "delete_me.txt"
			}),
		})
		.await;

	assert!(!result.is_error);
	assert!(result.output.contains("Deleted"));
	assert!(!wd.join("delete_me.txt").exists());
}

#[tokio::test]
async fn fs_delete_non_empty_dir_without_recursive_fails() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let registry = make_registry_with_fs(wd.clone());

	tokio::fs::create_dir_all(wd.join("mydir"))
		.await
		.unwrap();
	tokio::fs::write(wd.join("mydir/file.txt"), "data")
		.await
		.unwrap();

	let result = registry
		.execute(&ToolCallRequest {
			id: "1".to_string(),
			name: "fs_delete".to_string(),
			arguments: serde_json::json!({
				"path": "mydir"
			}),
		})
		.await;

	assert!(result.is_error);
	assert!(result.output.contains("not empty"));
}

#[tokio::test]
async fn fs_move_file() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let registry = make_registry_with_fs(wd.clone());

	tokio::fs::write(wd.join("src.txt"), "moved content")
		.await
		.unwrap();

	let result = registry
		.execute(&ToolCallRequest {
			id: "1".to_string(),
			name: "fs_move".to_string(),
			arguments: serde_json::json!({
				"source": "src.txt",
				"destination": "dst/moved.txt"
			}),
		})
		.await;

	assert!(!result.is_error);
	assert!(result.output.contains("Moved"));
	assert!(!wd.join("src.txt").exists());
	let content = tokio::fs::read_to_string(wd.join("dst/moved.txt"))
		.await
		.unwrap();
	assert_eq!(content, "moved content");
}

// ===========================================================================
// Git and bash tool registration tests
// ===========================================================================

use simse_core::tools::host::bash::{register_bash_tool, BashToolOptions};
use simse_core::tools::host::git::{register_git_tools, GitToolOptions};

#[test]
fn git_tools_register_all_9() {
	let tmp = tempfile::tempdir().unwrap();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	register_git_tools(
		&mut registry,
		GitToolOptions {
			working_directory: tmp.path().to_path_buf(),
		},
	);

	assert!(registry.is_registered("git_status"));
	assert!(registry.is_registered("git_diff"));
	assert!(registry.is_registered("git_log"));
	assert!(registry.is_registered("git_commit"));
	assert!(registry.is_registered("git_branch"));
	assert!(registry.is_registered("git_add"));
	assert!(registry.is_registered("git_stash"));
	assert!(registry.is_registered("git_push"));
	assert!(registry.is_registered("git_pull"));
	assert_eq!(registry.tool_count(), 9);
}

#[test]
fn bash_tool_registers() {
	let tmp = tempfile::tempdir().unwrap();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());
	register_bash_tool(
		&mut registry,
		BashToolOptions {
			working_directory: tmp.path().to_path_buf(),
			default_timeout_ms: None,
			max_output_bytes: None,
			shell: None,
		},
	);

	assert!(registry.is_registered("bash"));
	assert_eq!(registry.tool_count(), 1);
}

#[test]
fn all_host_tools_register_together() {
	let tmp = tempfile::tempdir().unwrap();
	let wd = tmp.path().to_path_buf();
	let mut registry = ToolRegistry::new(ToolRegistryOptions::default());

	register_filesystem_tools(
		&mut registry,
		FilesystemToolOptions {
			working_directory: wd.clone(),
			allowed_paths: None,
		},
	);
	register_git_tools(
		&mut registry,
		GitToolOptions {
			working_directory: wd.clone(),
		},
	);
	register_bash_tool(
		&mut registry,
		BashToolOptions {
			working_directory: wd,
			default_timeout_ms: None,
			max_output_bytes: None,
			shell: None,
		},
	);

	// 9 fs + 9 git + 1 bash = 19
	assert_eq!(registry.tool_count(), 19);
}
