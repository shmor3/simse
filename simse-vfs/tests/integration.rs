// ---------------------------------------------------------------------------
// Integration tests for simse-vfs-engine
//
// Each test spawns the binary, communicates over JSON-RPC 2.0 / NDJSON stdio,
// and verifies responses.
// ---------------------------------------------------------------------------

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Manages a running `simse-vfs-engine` child process and provides methods for
/// sending JSON-RPC requests and reading responses.
struct VfsProcess {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    next_id: AtomicU64,
}

impl VfsProcess {
    /// Spawn a new VFS engine process.
    fn spawn() -> Self {
        let bin = env!("CARGO_BIN_EXE_simse-vfs-engine");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn simse-vfs-engine");

        let stdout = child.stdout.take().expect("no stdout");
        let reader = BufReader::new(stdout);

        Self {
            child,
            reader,
            next_id: AtomicU64::new(1),
        }
    }

    /// Send a JSON-RPC request and return the response `result` or `error` field.
    /// Skips any notification lines (lines without an `id` field).
    fn send(&mut self, method: &str, params: Value) -> RpcResponse {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let stdin = self.child.stdin.as_mut().expect("no stdin");
        let mut line = serde_json::to_string(&request).unwrap();
        line.push('\n');
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.flush().unwrap();

        // Read lines until we find the response matching our id.
        // The server may emit notification lines (no `id`) before the response.
        loop {
            let mut buf = String::new();
            let bytes_read = self
                .reader
                .read_line(&mut buf)
                .expect("failed to read from stdout");
            if bytes_read == 0 {
                panic!(
                    "unexpected EOF from simse-vfs-engine while waiting for response to id={}",
                    id
                );
            }
            let buf = buf.trim();
            if buf.is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(buf)
                .unwrap_or_else(|e| panic!("invalid JSON from engine: {e}\nline: {buf}"));

            // Notifications have no `id` field — skip them.
            if parsed.get("id").is_none() {
                continue;
            }

            let resp_id = parsed["id"].as_u64().expect("response id is not u64");
            assert_eq!(resp_id, id, "response id mismatch");

            if let Some(error) = parsed.get("error") {
                return RpcResponse::Error(error.clone());
            }
            // `result` might be null, which is fine.
            return RpcResponse::Ok(parsed.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// Convenience: send a request, expect success, return the `result` value.
    fn call(&mut self, method: &str, params: Value) -> Value {
        match self.send(method, params) {
            RpcResponse::Ok(v) => v,
            RpcResponse::Error(e) => panic!("expected success, got error: {e}"),
        }
    }

    /// Convenience: send a request, expect an error, return the `error` object.
    fn call_err(&mut self, method: &str, params: Value) -> Value {
        match self.send(method, params) {
            RpcResponse::Error(e) => e,
            RpcResponse::Ok(v) => panic!("expected error, got success: {v}"),
        }
    }

    /// Initialize with default params (no limits, no history config).
    fn initialize(&mut self) -> Value {
        self.call("initialize", json!({"limits": null, "history": null}))
    }

    /// Initialize with custom limits.
    fn initialize_with_limits(&mut self, limits: Value) -> Value {
        self.call("initialize", json!({"limits": limits, "history": null}))
    }

    /// Initialize with disk backend pointing to the given root directory.
    fn initialize_with_disk(&mut self, root_dir: &str) -> Value {
        self.call(
            "initialize",
            json!({
                "limits": null,
                "history": null,
                "disk": {
                    "rootDirectory": root_dir,
                }
            }),
        )
    }
}

impl Drop for VfsProcess {
    fn drop(&mut self) {
        // Close stdin to let the child exit gracefully.
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

#[derive(Debug)]
enum RpcResponse {
    Ok(Value),
    Error(Value),
}

// ---------------------------------------------------------------------------
// Test 1: writeFile → readFile → verify content
// ---------------------------------------------------------------------------

#[test]
fn write_and_read_file() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    // Write a file
    proc.call(
        "vfs/writeFile",
        json!({
            "path": "vfs:///hello.txt",
            "content": "Hello, world!",
        }),
    );

    // Read it back
    let result = proc.call("vfs/readFile", json!({"path": "vfs:///hello.txt"}));

    assert_eq!(result["contentType"], "text");
    assert_eq!(result["text"], "Hello, world!");
    assert_eq!(result["size"], 13);
}

// ---------------------------------------------------------------------------
// Test 2: mkdir → readdir → verify listing
// ---------------------------------------------------------------------------

#[test]
fn mkdir_and_readdir() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    // Create directories
    proc.call(
        "vfs/mkdir",
        json!({"path": "vfs:///src", "recursive": false}),
    );
    proc.call(
        "vfs/mkdir",
        json!({"path": "vfs:///src/utils", "recursive": false}),
    );

    // Write a file in src/
    proc.call(
        "vfs/writeFile",
        json!({
            "path": "vfs:///src/main.ts",
            "content": "console.log('hi');",
        }),
    );

    // readdir on src/
    let result = proc.call(
        "vfs/readdir",
        json!({"path": "vfs:///src", "recursive": false}),
    );

    let entries = result["entries"].as_array().expect("readdir should return entries array");
    assert_eq!(entries.len(), 2);

    // Collect names and types
    let mut names: Vec<&str> = entries.iter().map(|e| e["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, vec!["main.ts", "utils"]);

    // Verify types
    for entry in entries {
        let name = entry["name"].as_str().unwrap();
        match name {
            "main.ts" => assert_eq!(entry["type"], "file"),
            "utils" => assert_eq!(entry["type"], "directory"),
            _ => panic!("unexpected entry: {name}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Test 3: writeFile → deleteFile → exists returns false
// ---------------------------------------------------------------------------

#[test]
fn delete_file_and_exists() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    proc.call(
        "vfs/writeFile",
        json!({
            "path": "vfs:///temp.txt",
            "content": "temporary",
        }),
    );

    // Verify it exists
    let result = proc.call("vfs/exists", json!({"path": "vfs:///temp.txt"}));
    assert_eq!(result["exists"], true);

    // Delete it
    let del = proc.call("vfs/deleteFile", json!({"path": "vfs:///temp.txt"}));
    assert_eq!(del["deleted"], true);

    // Verify it no longer exists
    let result = proc.call("vfs/exists", json!({"path": "vfs:///temp.txt"}));
    assert_eq!(result["exists"], false);
}

// ---------------------------------------------------------------------------
// Test 4: writeFile → rename → readFile at new path
// ---------------------------------------------------------------------------

#[test]
fn rename_file() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    proc.call(
        "vfs/writeFile",
        json!({
            "path": "vfs:///old.txt",
            "content": "rename me",
        }),
    );

    proc.call(
        "vfs/rename",
        json!({
            "oldPath": "vfs:///old.txt",
            "newPath": "vfs:///new.txt",
        }),
    );

    // Old path should not exist
    let result = proc.call("vfs/exists", json!({"path": "vfs:///old.txt"}));
    assert_eq!(result["exists"], false);

    // New path should have the content
    let result = proc.call("vfs/readFile", json!({"path": "vfs:///new.txt"}));
    assert_eq!(result["text"], "rename me");
}

// ---------------------------------------------------------------------------
// Test 5: writeFile → search → verify match
// ---------------------------------------------------------------------------

#[test]
fn search_file_content() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    proc.call(
        "vfs/writeFile",
        json!({
            "path": "vfs:///notes.txt",
            "content": "line one\nfind this needle here\nline three",
        }),
    );

    let result = proc.call(
        "vfs/search",
        json!({
            "query": "needle",
        }),
    );

    let results = result["results"].as_array().expect("search should return results array");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["path"], "vfs:///notes.txt");
    assert_eq!(results[0]["line"], 2);
    assert!(
        results[0]["match"]
            .as_str()
            .unwrap()
            .contains("needle"),
        "match text should contain the search term"
    );
}

// ---------------------------------------------------------------------------
// Test 6: writeFile → glob → verify matches
// ---------------------------------------------------------------------------

#[test]
fn glob_matches() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    // Create several files with different extensions
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///app.ts", "content": "ts"}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///lib.ts", "content": "ts2"}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///readme.md", "content": "md"}),
    );
    proc.call(
        "vfs/mkdir",
        json!({"path": "vfs:///src", "recursive": false}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///src/index.ts", "content": "index"}),
    );

    // Glob for all .ts files
    let result = proc.call("vfs/glob", json!({"pattern": "**/*.ts"}));
    let matches = result["matches"].as_array().expect("glob should return matches array");

    // Should find app.ts, lib.ts, src/index.ts
    assert_eq!(matches.len(), 3, "expected 3 .ts files, got: {matches:?}");

    let paths: Vec<&str> = matches.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(paths.contains(&"vfs:///app.ts"));
    assert!(paths.contains(&"vfs:///lib.ts"));
    assert!(paths.contains(&"vfs:///src/index.ts"));
}

// ---------------------------------------------------------------------------
// Test 7: writeFile v1 → writeFile v2 → diff → verify hunks
// ---------------------------------------------------------------------------

#[test]
fn diff_two_files() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    proc.call(
        "vfs/writeFile",
        json!({
            "path": "vfs:///v1.txt",
            "content": "line one\nline two\nline three\n",
        }),
    );
    proc.call(
        "vfs/writeFile",
        json!({
            "path": "vfs:///v2.txt",
            "content": "line one\nline TWO modified\nline three\nline four\n",
        }),
    );

    let diff = proc.call(
        "vfs/diff",
        json!({
            "oldPath": "vfs:///v1.txt",
            "newPath": "vfs:///v2.txt",
        }),
    );

    assert_eq!(diff["oldPath"], "vfs:///v1.txt");
    assert_eq!(diff["newPath"], "vfs:///v2.txt");

    let hunks = diff["hunks"].as_array().expect("hunks should be array");
    assert!(!hunks.is_empty(), "diff should produce at least one hunk");

    // The diff should have at least 1 addition (the modified line and the new line)
    let additions = diff["additions"].as_u64().unwrap();
    let deletions = diff["deletions"].as_u64().unwrap();
    assert!(additions > 0, "expected additions in diff");
    assert!(deletions > 0, "expected deletions in diff");
}

// ---------------------------------------------------------------------------
// Test 8: snapshot → clear → restore → verify content
// ---------------------------------------------------------------------------

#[test]
fn snapshot_clear_restore() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    // Create some content
    proc.call(
        "vfs/mkdir",
        json!({"path": "vfs:///data", "recursive": false}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///data/config.json", "content": "{\"key\": \"value\"}"}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///root.txt", "content": "root file"}),
    );

    // Take snapshot
    let snapshot = proc.call("vfs/snapshot", json!({}));
    assert!(
        snapshot["files"].as_array().unwrap().len() >= 2,
        "snapshot should contain at least 2 files"
    );
    assert!(
        snapshot["directories"].as_array().unwrap().len() >= 1,
        "snapshot should contain at least 1 directory (besides root)"
    );

    // Clear the VFS
    proc.call("vfs/clear", json!({}));

    // Verify everything is gone
    let result = proc.call("vfs/exists", json!({"path": "vfs:///data/config.json"}));
    assert_eq!(result["exists"], false);
    let result = proc.call("vfs/exists", json!({"path": "vfs:///root.txt"}));
    assert_eq!(result["exists"], false);

    // Restore from snapshot
    proc.call("vfs/restore", json!({"snapshot": snapshot}));

    // Verify content is back
    let result = proc.call("vfs/readFile", json!({"path": "vfs:///data/config.json"}));
    assert_eq!(result["text"], "{\"key\": \"value\"}");

    let result = proc.call("vfs/readFile", json!({"path": "vfs:///root.txt"}));
    assert_eq!(result["text"], "root file");
}

// ---------------------------------------------------------------------------
// Test 9: transaction with error → verify rollback
// ---------------------------------------------------------------------------

#[test]
fn transaction_rollback_on_error() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    // Create a pre-existing file
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///existing.txt", "content": "keep me"}),
    );

    // Submit a transaction that writes a new file and then fails
    // (writing to the root path "vfs:///" should fail)
    let error = proc.call_err(
        "vfs/transaction",
        json!({
            "ops": [
                {"type": "writeFile", "path": "vfs:///new.txt", "content": "new data"},
                {"type": "writeFile", "path": "vfs:///", "content": "bad"}
            ]
        }),
    );

    // Verify we got an error
    assert!(
        error.get("code").is_some(),
        "error should have a code field"
    );

    // The new.txt should NOT exist (rollback)
    let result = proc.call("vfs/exists", json!({"path": "vfs:///new.txt"}));
    assert_eq!(
        result["exists"], false,
        "new.txt should not exist after transaction rollback"
    );

    // The existing file should still be there
    let result = proc.call("vfs/readFile", json!({"path": "vfs:///existing.txt"}));
    assert_eq!(result["text"], "keep me");
}

// ---------------------------------------------------------------------------
// Test 10: initialize with limits → exceed limit → verify error response
// ---------------------------------------------------------------------------

#[test]
fn limits_exceeded_error() {
    let mut proc = VfsProcess::spawn();

    // Initialize with a very small max file size (10 bytes)
    proc.initialize_with_limits(json!({
        "maxFileSize": 10,
    }));

    // Try to write a file that exceeds the limit
    let error = proc.call_err(
        "vfs/writeFile",
        json!({
            "path": "vfs:///big.txt",
            "content": "this content is definitely longer than 10 bytes",
        }),
    );

    // Verify error response
    assert!(error.get("code").is_some(), "error should have a code");
    let code = error["code"].as_i64().unwrap();
    assert_eq!(code, -32000, "should be VFS_ERROR code");

    // Check error data has the VFS-specific code
    let data = error.get("data");
    assert!(data.is_some(), "error should have data field");
    let vfs_code = data.unwrap()["vfsCode"].as_str().unwrap();
    assert_eq!(
        vfs_code, "VFS_LIMIT_EXCEEDED",
        "should be VFS_LIMIT_EXCEEDED error"
    );

    // The file should not exist
    let result = proc.call("vfs/exists", json!({"path": "vfs:///big.txt"}));
    assert_eq!(result["exists"], false);
}

// ---------------------------------------------------------------------------
// Bonus: verify calling a method before initialize returns an error
// ---------------------------------------------------------------------------

#[test]
fn error_before_initialize() {
    let mut proc = VfsProcess::spawn();

    let error = proc.call_err(
        "vfs/readFile",
        json!({"path": "vfs:///anything.txt"}),
    );

    assert!(error.get("code").is_some());
    let msg = error["message"].as_str().unwrap();
    assert!(
        msg.contains("Not initialized"),
        "error message should mention not initialized, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Bonus: verify unknown method returns method-not-found error
// ---------------------------------------------------------------------------

#[test]
fn unknown_method_error() {
    let mut proc = VfsProcess::spawn();
    proc.initialize();

    let error = proc.call_err("vfs/nonExistent", json!({}));

    let code = error["code"].as_i64().unwrap();
    assert_eq!(code, -32601, "should be METHOD_NOT_FOUND code");
}

// ===========================================================================
// Disk backend integration tests
// ===========================================================================

// ---------------------------------------------------------------------------
// Disk test 1: write via file://, read back, verify content
// ---------------------------------------------------------------------------

#[test]
fn disk_write_and_read_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    // Write a file
    proc.call(
        "vfs/writeFile",
        json!({
            "path": "file:///hello.txt",
            "content": "Hello, disk!",
        }),
    );

    // Read it back
    let result = proc.call("vfs/readFile", json!({"path": "file:///hello.txt"}));
    assert_eq!(result["contentType"], "text");
    assert_eq!(result["text"], "Hello, disk!");
    assert_eq!(result["size"], 12);
}

// ---------------------------------------------------------------------------
// Disk test 2: create a file in temp dir before init, read via file://
// ---------------------------------------------------------------------------

#[test]
fn disk_read_real_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    // Create a file on disk before initializing the VFS
    std::fs::write(dir.path().join("pre-existing.txt"), "I was here first").unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    let result = proc.call(
        "vfs/readFile",
        json!({"path": "file:///pre-existing.txt"}),
    );
    assert_eq!(result["contentType"], "text");
    assert_eq!(result["text"], "I was here first");
    assert_eq!(result["size"], 16);
}

// ---------------------------------------------------------------------------
// Disk test 3: write then append, verify combined content
// ---------------------------------------------------------------------------

#[test]
fn disk_append_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///log.txt", "content": "line1\n"}),
    );

    proc.call(
        "vfs/appendFile",
        json!({"path": "file:///log.txt", "content": "line2\n"}),
    );

    let result = proc.call("vfs/readFile", json!({"path": "file:///log.txt"}));
    assert_eq!(result["text"], "line1\nline2\n");
    assert_eq!(result["size"], 12);
}

// ---------------------------------------------------------------------------
// Disk test 4: write, delete, verify gone
// ---------------------------------------------------------------------------

#[test]
fn disk_delete_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///temp.txt", "content": "delete me"}),
    );

    // Verify exists
    let result = proc.call("vfs/exists", json!({"path": "file:///temp.txt"}));
    assert_eq!(result["exists"], true);

    // Delete
    proc.call("vfs/deleteFile", json!({"path": "file:///temp.txt"}));

    // Verify gone
    let result = proc.call("vfs/exists", json!({"path": "file:///temp.txt"}));
    assert_eq!(result["exists"], false);
}

// ---------------------------------------------------------------------------
// Disk test 5: mkdir, write files, readdir, verify listing
// ---------------------------------------------------------------------------

#[test]
fn disk_mkdir_and_readdir() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/mkdir",
        json!({"path": "file:///mydir", "recursive": false}),
    );

    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///mydir/a.txt", "content": "a"}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///mydir/b.txt", "content": "b"}),
    );

    let result = proc.call(
        "vfs/readdir",
        json!({"path": "file:///mydir", "recursive": false}),
    );

    let entries = result["entries"].as_array().expect("readdir should return entries array");
    assert_eq!(entries.len(), 2);

    let mut names: Vec<&str> = entries.iter().map(|e| e["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, vec!["a.txt", "b.txt"]);

    for entry in entries {
        assert_eq!(entry["type"], "file");
    }
}

// ---------------------------------------------------------------------------
// Disk test 6: mkdir, rmdir, verify gone
// ---------------------------------------------------------------------------

#[test]
fn disk_rmdir() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/mkdir",
        json!({"path": "file:///empty_dir", "recursive": false}),
    );

    // Verify exists
    let result = proc.call("vfs/exists", json!({"path": "file:///empty_dir"}));
    assert_eq!(result["exists"], true);

    // Remove it
    proc.call(
        "vfs/rmdir",
        json!({"path": "file:///empty_dir", "recursive": false}),
    );

    // Verify gone
    let result = proc.call("vfs/exists", json!({"path": "file:///empty_dir"}));
    assert_eq!(result["exists"], false);
}

// ---------------------------------------------------------------------------
// Disk test 7: write file, stat, verify metadata; exists returns true
// ---------------------------------------------------------------------------

#[test]
fn disk_stat_and_exists() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///info.txt", "content": "data"}),
    );

    let stat = proc.call("vfs/stat", json!({"path": "file:///info.txt"}));
    assert_eq!(stat["type"], "file");
    assert_eq!(stat["size"], 4);
    assert!(
        stat["modifiedAt"].as_u64().unwrap() > 0,
        "modifiedAt should be > 0"
    );

    let exists = proc.call("vfs/exists", json!({"path": "file:///info.txt"}));
    assert_eq!(exists["exists"], true);

    let not_exists = proc.call("vfs/exists", json!({"path": "file:///nope.txt"}));
    assert_eq!(not_exists["exists"], false);
}

// ---------------------------------------------------------------------------
// Disk test 8: write file, rename, verify old gone + new has content
// ---------------------------------------------------------------------------

#[test]
fn disk_rename() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///old.txt", "content": "rename me"}),
    );

    proc.call(
        "vfs/rename",
        json!({
            "oldPath": "file:///old.txt",
            "newPath": "file:///new.txt",
        }),
    );

    // Old path should not exist
    let result = proc.call("vfs/exists", json!({"path": "file:///old.txt"}));
    assert_eq!(result["exists"], false);

    // New path should have the content
    let result = proc.call("vfs/readFile", json!({"path": "file:///new.txt"}));
    assert_eq!(result["text"], "rename me");
}

// ---------------------------------------------------------------------------
// Disk test 9: write file, copy, verify both exist
// ---------------------------------------------------------------------------

#[test]
fn disk_copy() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///src.txt", "content": "copy me"}),
    );

    proc.call(
        "vfs/copy",
        json!({
            "src": "file:///src.txt",
            "dest": "file:///dest.txt",
        }),
    );

    // Both should exist with same content
    let src = proc.call("vfs/readFile", json!({"path": "file:///src.txt"}));
    assert_eq!(src["text"], "copy me");

    let dest = proc.call("vfs/readFile", json!({"path": "file:///dest.txt"}));
    assert_eq!(dest["text"], "copy me");
}

// ---------------------------------------------------------------------------
// Disk test 10: write multiple files, glob **/*.txt, verify matches
// ---------------------------------------------------------------------------

#[test]
fn disk_glob() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///a.txt", "content": "a"}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///b.rs", "content": "b"}),
    );
    proc.call(
        "vfs/mkdir",
        json!({"path": "file:///sub", "recursive": false}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///sub/c.txt", "content": "c"}),
    );

    // Glob uses the pattern to detect disk scheme — prefix with file://
    let result = proc.call("vfs/glob", json!({"pattern": "file:///**/*.txt"}));
    let matches = result["matches"].as_array().expect("glob should return matches");

    assert_eq!(matches.len(), 2, "expected 2 .txt files, got: {matches:?}");

    let paths: Vec<&str> = matches.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(paths.iter().any(|p| p.contains("a.txt")));
    assert!(paths.iter().any(|p| p.contains("c.txt")));
    // b.rs should NOT match
    assert!(!paths.iter().any(|p| p.contains("b.rs")));
}

// ---------------------------------------------------------------------------
// Disk test 11: create directory structure, verify tree output
// ---------------------------------------------------------------------------

#[test]
fn disk_tree() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/mkdir",
        json!({"path": "file:///project", "recursive": false}),
    );
    proc.call(
        "vfs/mkdir",
        json!({"path": "file:///project/src", "recursive": false}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///project/src/main.rs", "content": "fn main() {}"}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///project/README.md", "content": "# Readme"}),
    );

    let result = proc.call("vfs/tree", json!({"path": "file:///project"}));
    let tree = result["tree"].as_str().expect("tree should be a string");

    assert!(tree.contains("src"), "tree should contain 'src'");
    assert!(tree.contains("main.rs"), "tree should contain 'main.rs'");
    assert!(tree.contains("README.md"), "tree should contain 'README.md'");
}

// ---------------------------------------------------------------------------
// Disk test 12: write files, verify du sums sizes
// ---------------------------------------------------------------------------

#[test]
fn disk_du() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/mkdir",
        json!({"path": "file:///dutest", "recursive": false}),
    );
    // 4 bytes
    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///dutest/a.txt", "content": "aaaa"}),
    );
    // 2 bytes
    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///dutest/b.txt", "content": "bb"}),
    );

    let result = proc.call("vfs/du", json!({"path": "file:///dutest"}));
    assert_eq!(result["size"], 6, "du should sum to 6 bytes");
}

// ---------------------------------------------------------------------------
// Disk test 13: write file with content, search for text, verify match
// ---------------------------------------------------------------------------

#[test]
fn disk_search() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/writeFile",
        json!({
            "path": "file:///notes.txt",
            "content": "line one\nfind this needle here\nline three"
        }),
    );

    // For disk search, the glob param determines the scheme.
    // Without a glob, search defaults to VFS. Use a glob with file:// prefix.
    let result = proc.call(
        "vfs/search",
        json!({
            "query": "needle",
            "glob": "file:///**/*",
        }),
    );

    let results = result["results"]
        .as_array()
        .expect("search should return results array");
    assert_eq!(results.len(), 1);
    assert!(results[0]["path"].as_str().unwrap().contains("notes.txt"));
    assert_eq!(results[0]["line"], 2);
    assert!(
        results[0]["match"]
            .as_str()
            .unwrap()
            .contains("needle"),
        "match text should contain the search term"
    );
}

// ---------------------------------------------------------------------------
// Disk test 14: sandbox violation — path traversal should fail
// ---------------------------------------------------------------------------

#[test]
fn disk_sandbox_violation() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    let error = proc.call_err(
        "vfs/readFile",
        json!({"path": "file:///../../../etc/passwd"}),
    );

    assert!(error.get("code").is_some());
    let data = error.get("data").expect("error should have data");
    let vfs_code = data["vfsCode"].as_str().unwrap();
    // Could be VFS_NOT_FOUND or VFS_INVALID_PATH depending on OS
    assert!(
        vfs_code == "VFS_INVALID_PATH" || vfs_code == "VFS_NOT_FOUND",
        "expected VFS_INVALID_PATH or VFS_NOT_FOUND, got: {vfs_code}"
    );
}

// ---------------------------------------------------------------------------
// Disk test 15: init without disk config, try file:// path → DISK_NOT_CONFIGURED
// ---------------------------------------------------------------------------

#[test]
fn disk_not_configured() {
    let mut proc = VfsProcess::spawn();
    // Initialize without disk config
    proc.initialize();

    let error = proc.call_err(
        "vfs/readFile",
        json!({"path": "file:///anything.txt"}),
    );

    assert!(error.get("code").is_some());
    let data = error.get("data").expect("error should have data");
    let vfs_code = data["vfsCode"].as_str().unwrap();
    assert_eq!(
        vfs_code, "VFS_DISK_NOT_CONFIGURED",
        "should be VFS_DISK_NOT_CONFIGURED, got: {vfs_code}"
    );
}

// ---------------------------------------------------------------------------
// Disk test 16: vfs/history with file:// path → rejected
// ---------------------------------------------------------------------------

#[test]
fn disk_vfs_only_method_rejected() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    proc.call(
        "vfs/writeFile",
        json!({"path": "file:///test.txt", "content": "hello"}),
    );

    // history is now routed to disk backend — should succeed with empty entries
    // (first write doesn't record history, only overwrites do)
    let result = proc.call(
        "vfs/history",
        json!({"path": "file:///test.txt"}),
    );

    let entries = result["entries"].as_array().unwrap();
    assert!(entries.is_empty(), "first write should have no history entries");
}

// ---------------------------------------------------------------------------
// Disk test 17: vfs/rename with mixed vfs:// and file:// → rejected
// ---------------------------------------------------------------------------

#[test]
fn disk_mixed_scheme_rejected() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    // Write a VFS file so it exists
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///old.txt", "content": "data"}),
    );

    let error = proc.call_err(
        "vfs/rename",
        json!({
            "oldPath": "vfs:///old.txt",
            "newPath": "file:///new.txt",
        }),
    );

    assert!(error.get("code").is_some());
    let msg = error["message"].as_str().unwrap();
    assert!(
        msg.contains("mix") || msg.contains("Cannot"),
        "error should indicate mixing schemes is not allowed, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Disk test 18: verify all existing vfs:// operations still work with disk configured
// ---------------------------------------------------------------------------

#[test]
fn disk_vfs_still_works() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let mut proc = VfsProcess::spawn();
    proc.initialize_with_disk(root);

    // Write via vfs://
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///hello.txt", "content": "Hello, VFS!"}),
    );

    // Read via vfs://
    let result = proc.call("vfs/readFile", json!({"path": "vfs:///hello.txt"}));
    assert_eq!(result["text"], "Hello, VFS!");

    // Write a second version so that history records the first
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///hello.txt", "content": "Hello, VFS v2!"}),
    );

    // Verify overwrite
    let result = proc.call("vfs/readFile", json!({"path": "vfs:///hello.txt"}));
    assert_eq!(result["text"], "Hello, VFS v2!");

    // History should work for vfs:// paths (v1 is recorded when overwritten by v2)
    let history = proc.call("vfs/history", json!({"path": "vfs:///hello.txt"}));
    let entries = history["entries"]
        .as_array()
        .expect("history should return entries array");
    assert!(
        !entries.is_empty(),
        "history should have at least one entry after overwrite"
    );

    // mkdir, readdir
    proc.call(
        "vfs/mkdir",
        json!({"path": "vfs:///dir", "recursive": false}),
    );
    proc.call(
        "vfs/writeFile",
        json!({"path": "vfs:///dir/file.txt", "content": "nested"}),
    );
    let readdir = proc.call(
        "vfs/readdir",
        json!({"path": "vfs:///dir", "recursive": false}),
    );
    let entries = readdir["entries"]
        .as_array()
        .expect("readdir should return entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "file.txt");

    // Exists
    let exists = proc.call("vfs/exists", json!({"path": "vfs:///hello.txt"}));
    assert_eq!(exists["exists"], true);
    let exists = proc.call("vfs/exists", json!({"path": "vfs:///nope.txt"}));
    assert_eq!(exists["exists"], false);

    // Snapshot/restore still works
    let snapshot = proc.call("vfs/snapshot", json!({}));
    assert!(
        snapshot["files"].as_array().unwrap().len() >= 2,
        "snapshot should contain files"
    );
}
