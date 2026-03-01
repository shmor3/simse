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
