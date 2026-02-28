# simse-vfs Rust Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite the simse-vfs core in Rust as a JSON-RPC 2.0 subprocess server, with a thin TypeScript client preserving the existing `VirtualFS` API.

**Architecture:** A Rust binary (`simse-vfs-engine`) handles all VFS operations in-memory and communicates via NDJSON stdio. The TS package (`simse-vfs/`) keeps its public types/interfaces unchanged and replaces the implementation in `vfs.ts` with a JSON-RPC client that spawns/talks to the Rust process. Files that access the host filesystem (`vfs-disk.ts`, `validators.ts`) stay in TypeScript.

**Tech Stack:** Rust 2021 edition, serde/serde_json, regex, thiserror, base64, tracing. TypeScript side uses `node:child_process` `spawn()` (NOT exec — spawn avoids shell injection by design).

---

### Task 1: Scaffold the Rust crate

**Files:**
- Create: `simse-vfs/engine/Cargo.toml`
- Create: `simse-vfs/engine/src/lib.rs`
- Create: `simse-vfs/engine/src/main.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "simse-vfs-engine"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "In-memory VFS server over JSON-RPC 2.0 / NDJSON stdio"

[lib]
name = "simse_vfs_engine"
path = "src/lib.rs"

[[bin]]
name = "simse-vfs-engine"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
thiserror = "2"
base64 = "0.22"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
```

**Step 2: Create stub lib.rs**

```rust
pub mod error;
pub mod transport;
pub mod protocol;
pub mod path;
pub mod vfs;
pub mod glob;
pub mod search;
pub mod diff;
pub mod server;
```

**Step 3: Create stub main.rs**

```rust
use simse_vfs_engine::server::VfsServer;
use simse_vfs_engine::transport::NdjsonTransport;

fn main() {
    // Logging to stderr (stdout is protocol-only)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let transport = NdjsonTransport::new();
    let mut server = VfsServer::new(transport);

    tracing::info!("simse-vfs-engine ready");

    if let Err(e) = server.run() {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

**Step 4: Commit**

```bash
git add simse-vfs/engine/
git commit -m "feat(vfs-engine): scaffold Rust crate"
```

---

### Task 2: Error types and transport

**Files:**
- Create: `simse-vfs/engine/src/error.rs`
- Create: `simse-vfs/engine/src/transport.rs`

**Step 1: Create error.rs**

Port of the VFS error codes from `simse-vfs/src/errors.ts`. Each variant maps to a VFS_ code.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VfsError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Not a file: {0}")]
    NotAFile(String),

    #[error("Not a directory: {0}")]
    NotADirectory(String),

    #[error("Not empty: {0}")]
    NotEmpty(String),

    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl VfsError {
    pub fn code(&self) -> &str {
        match self {
            Self::InvalidPath(_) => "VFS_INVALID_PATH",
            Self::NotFound(_) => "VFS_NOT_FOUND",
            Self::AlreadyExists(_) => "VFS_ALREADY_EXISTS",
            Self::NotAFile(_) => "VFS_NOT_A_FILE",
            Self::NotADirectory(_) => "VFS_NOT_A_DIRECTORY",
            Self::NotEmpty(_) => "VFS_NOT_EMPTY",
            Self::LimitExceeded(_) => "VFS_LIMIT_EXCEEDED",
            Self::InvalidOperation(_) => "VFS_INVALID_OPERATION",
            Self::Io(_) => "VFS_IO_ERROR",
            Self::Json(_) => "VFS_JSON_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "vfsCode": self.code(),
            "message": self.to_string(),
        })
    }
}
```

**Step 2: Create transport.rs**

Same pattern as `simse-engine/src/transport.rs` — NDJSON over stdout.

```rust
use std::io::{self, Write};
use serde::Serialize;

#[derive(Serialize)]
struct JsonRpcResponse<'a> {
    jsonrpc: &'a str,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcErrorBody>,
}

#[derive(Serialize)]
struct JsonRpcErrorBody {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct JsonRpcNotification<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

pub struct NdjsonTransport;

impl Default for NdjsonTransport {
    fn default() -> Self { Self::new() }
}

impl NdjsonTransport {
    pub fn new() -> Self { Self }

    pub fn write_response(&self, id: u64, result: serde_json::Value) {
        self.write_line(&JsonRpcResponse {
            jsonrpc: "2.0", id, result: Some(result), error: None,
        });
    }

    pub fn write_error(&self, id: u64, code: i32, message: impl Into<String>, data: Option<serde_json::Value>) {
        self.write_line(&JsonRpcResponse {
            jsonrpc: "2.0", id, result: None,
            error: Some(JsonRpcErrorBody { code, message: message.into(), data }),
        });
    }

    pub fn write_notification(&self, method: &str, params: serde_json::Value) {
        self.write_line(&JsonRpcNotification {
            jsonrpc: "2.0", method, params: Some(params),
        });
    }

    fn write_line(&self, value: &impl Serialize) {
        let mut stdout = io::stdout().lock();
        if let Err(e) = serde_json::to_writer(&mut stdout, value) {
            tracing::error!("Failed to serialize: {}", e);
            return;
        }
        let _ = writeln!(stdout);
        let _ = stdout.flush();
    }
}
```

**Step 3: Commit**

```bash
git add simse-vfs/engine/src/error.rs simse-vfs/engine/src/transport.rs
git commit -m "feat(vfs-engine): add error types and NDJSON transport"
```

---

### Task 3: Protocol types

**Files:**
- Create: `simse-vfs/engine/src/protocol.rs`

JSON-RPC incoming request type + all VFS-specific param/result serde types. All use `#[serde(rename_all = "camelCase")]` to match the TypeScript API.

Covers: `JsonRpcRequest`, `InitializeParams`, `LimitsParams`, `HistoryParams`, all file/dir/glob/search/diff/snapshot/transaction param structs, and all serializable result structs (`StatResult`, `DirEntry`, `ReadFileResult`, `SearchResult`, `DiffResult`, `DiffHunk`, `DiffLineResult`, `HistoryEntry`, `MetricsResult`, `SnapshotData`, `SnapshotFile`, `SnapshotDir`).

See the design doc for the complete method table: `docs/plans/2026-02-28-simse-vfs-rust-design.md`.

**Step 1: Implement protocol.rs** — create all param/result structs with serde derives.

**Step 2: Commit**

```bash
git add simse-vfs/engine/src/protocol.rs
git commit -m "feat(vfs-engine): add JSON-RPC protocol types"
```

---

### Task 4: Path normalization and validation

**Files:**
- Create: `simse-vfs/engine/src/path.rs`

Port of `simse-vfs/src/path-utils.ts` (115 lines). Must produce identical output for same inputs.

Functions: `normalize_path`, `parent_path`, `base_name`, `ancestor_paths`, `path_depth`, `to_local_path`, `validate_path`, `validate_segment`.

Also defines `VfsLimits` struct with defaults matching the TS version (maxFileSize: 10MB, maxTotalSize: 100MB, maxPathDepth: 32, etc.).

Include `#[cfg(test)]` unit tests verifying parity with the TS path-utils.

**Step 1: Implement path.rs with tests**

**Step 2: Run tests**

Run: `cd D:/GitHub/simse/simse-vfs/engine && cargo test -- path`
Expected: All path tests pass

**Step 3: Commit**

```bash
git add simse-vfs/engine/src/path.rs
git commit -m "feat(vfs-engine): add path normalization and validation"
```

---

### Task 5: Glob matching (brace expansion + negation)

**Files:**
- Create: `simse-vfs/engine/src/glob.rs`

Port of glob logic from `vfs.ts`: `expand_braces`, `match_segment` (wildcard `*`/`?`), `match_parts` (`**` globstar), `match_glob` (full path, with brace expansion).

Include `#[cfg(test)]` tests for brace expansion, basic glob, globstar, and brace+glob combos.

**Step 1: Implement glob.rs with tests**

**Step 2: Run tests**

Run: `cd D:/GitHub/simse/simse-vfs/engine && cargo test -- glob`

**Step 3: Commit**

```bash
git add simse-vfs/engine/src/glob.rs
git commit -m "feat(vfs-engine): add glob matching with brace expansion"
```

---

### Task 6: Search module

**Files:**
- Create: `simse-vfs/engine/src/search.rs`

Supports substring and regex modes, context lines (before/after), count-only mode. The `search_text` function operates on a single file's text and accumulates results.

**Step 1: Implement search.rs**

**Step 2: Commit**

```bash
git add simse-vfs/engine/src/search.rs
git commit -m "feat(vfs-engine): add search module with regex and context"
```

---

### Task 7: Diff module (Myers algorithm)

**Files:**
- Create: `simse-vfs/engine/src/diff.rs`

Port of the Myers diff from `vfs.ts` (lines 966-1060). Includes hunk building with configurable context lines.

`compute_diff(old_lines, new_lines, context, max_lines) -> Result<DiffOutput, String>`

Include `#[cfg(test)]` tests: empty diff, pure additions, pure deletions, mixed changes.

**Step 1: Implement diff.rs with tests**

**Step 2: Run tests**

Run: `cd D:/GitHub/simse/simse-vfs/engine && cargo test -- diff`

**Step 3: Commit**

```bash
git add simse-vfs/engine/src/diff.rs
git commit -m "feat(vfs-engine): add Myers diff algorithm"
```

---

### Task 8: In-memory VFS core

**Files:**
- Create: `simse-vfs/engine/src/vfs.rs`

The largest module. Port of the full VFS from `vfs.ts` (~1,750 lines). Uses `HashMap<String, InternalNode>` for the node tree.

Key struct: `VirtualFs` with methods:
- `new(limits, max_history)` — init root node
- File ops: `read_file`, `write_file`, `append_file`, `delete_file`
- Dir ops: `mkdir`, `readdir`, `rmdir`
- Navigation: `stat`, `exists`, `rename`, `copy`
- Query: `glob` (with negation), `tree`, `du`, `search`
- History: `history`, `diff`, `diff_versions`, `checkout`
- Snapshot: `snapshot`, `restore`, `clear`
- Transaction: `transaction` (snapshot-based rollback)
- Events: `drain_events` (returns pending VfsEvent list)
- Metrics: `metrics` (totalSize, nodeCount, fileCount, dirCount)

Each method must match the TS behavior exactly. The existing TypeScript tests are the specification.

**Step 1: Implement vfs.rs** — this is the core of the refactor

**Step 2: Verify compilation**

Run: `cd D:/GitHub/simse/simse-vfs/engine && cargo check`

**Step 3: Commit**

```bash
git add simse-vfs/engine/src/vfs.rs
git commit -m "feat(vfs-engine): add in-memory VFS core"
```

---

### Task 9: Server dispatcher

**Files:**
- Create: `simse-vfs/engine/src/server.rs`

Reads JSON-RPC from stdin, dispatches to VFS, writes responses. Handles `initialize` to create the VFS instance. After each mutating call, drains events and sends as notifications.

Pattern: `VfsServer` struct with `run()` loop and `dispatch()` method. Helper closures `with_vfs`/`with_vfs_mut` for safe VFS access.

One handler function per JSON-RPC method (~25 handlers). Each is ~10-20 lines: parse params → call vfs → serialize result.

**Step 1: Implement server.rs**

**Step 2: Build**

Run: `cd D:/GitHub/simse/simse-vfs/engine && cargo build`
Expected: Clean build

**Step 3: Commit**

```bash
git add simse-vfs/engine/src/server.rs
git commit -m "feat(vfs-engine): add JSON-RPC server dispatcher"
```

---

### Task 10: Rust integration tests

**Files:**
- Create: `simse-vfs/engine/tests/integration.rs`

End-to-end tests that spawn the binary, send JSON-RPC over stdio, verify responses. Test flows:
1. Initialize → writeFile → readFile → verify content
2. Initialize → mkdir → readdir → verify listing
3. Initialize → writeFile → deleteFile → exists returns false
4. Initialize → writeFile → rename → readFile at new path
5. Initialize → writeFile → search → verify match
6. Initialize → writeFile → glob → verify matches
7. Initialize → writeFile v1 → writeFile v2 → diff → verify hunks
8. Initialize → snapshot → clear → restore → verify content
9. Initialize → transaction with error → verify rollback
10. Initialize with limits → exceed limit → verify error response

**Step 1: Implement integration tests**

**Step 2: Run**

Run: `cd D:/GitHub/simse/simse-vfs/engine && cargo test --test integration`

**Step 3: Commit**

```bash
git add simse-vfs/engine/tests/
git commit -m "test(vfs-engine): add integration tests"
```

---

### Task 11: TypeScript JSON-RPC client

**Files:**
- Create: `simse-vfs/src/client.ts`

Thin client using `node:child_process` `spawn()` (safe — no shell). Manages pending request map, parses NDJSON responses, routes event notifications to callbacks, maps errors to `VFSError`.

**Important:** Uses `spawn()` NOT `exec()`. Spawn bypasses the shell entirely — each arg is passed directly to the binary. This prevents command injection by design.

```typescript
import { spawn } from 'node:child_process';
```

Exports: `VFSClient` interface (request, dispose), `createVFSClient` factory.

**Step 1: Implement client.ts**

**Step 2: Verify typecheck**

Run: `cd D:/GitHub/simse && bun run typecheck`

**Step 3: Commit**

```bash
git add simse-vfs/src/client.ts
git commit -m "feat(vfs): add JSON-RPC client for Rust subprocess"
```

---

### Task 12: Rewrite vfs.ts as client wrapper

**Files:**
- Modify: `simse-vfs/src/vfs.ts`
- Modify: `simse-vfs/src/lib.ts`

Replace the 1,750-line in-memory implementation with a thin wrapper delegating to the Rust subprocess.

**Breaking change:** All VirtualFS methods become **async** (return Promises) because IPC is inherently asynchronous. The `VirtualFS` interface is updated accordingly. `createVirtualFS` becomes `async`.

Added: `dispose()` method on VirtualFS to clean up the subprocess.

Added: `enginePath?: string` option in `VirtualFSOptions` for specifying the binary location.

Keep `VirtualFSOptions` and type definitions unchanged.

**Step 1: Rewrite vfs.ts**

**Step 2: Update lib.ts exports** — add `VFSClient`, `VFSClientOptions`, `VFSClientEvent`

**Step 3: Commit**

```bash
git add simse-vfs/src/vfs.ts simse-vfs/src/lib.ts
git commit -m "feat(vfs): rewrite createVirtualFS as Rust subprocess client"
```

---

### Task 13: Update consumers for async VFS

**Files:**
- Modify: `src/ai/tools/builtin-tools.ts`
- Modify: `src/ai/mcp/mcp-server.ts`
- Modify: any other files importing from simse-vfs that call VFS methods

Search all call sites (`vfs.readFile`, `vfs.writeFile`, `vfs.mkdir`, etc.) and add `await`. Ensure containing functions are `async`.

**Step 1: Find all VFS call sites in src/**

**Step 2: Update each to async**

**Step 3: Verify typecheck**

Run: `cd D:/GitHub/simse && bun run typecheck`
Expected: Clean

**Step 4: Commit**

```bash
git add src/
git commit -m "refactor: update VFS consumers for async subprocess API"
```

---

### Task 14: Update existing tests

**Files:**
- Modify: `simse-vfs/tests/vfs.test.ts`

Update 194+ tests: `createVirtualFS` is now async, all VFS method calls need `await`. Set `enginePath` in test helper to point to the built Rust binary.

**Step 1: Build Rust binary**

Run: `cd D:/GitHub/simse/simse-vfs/engine && cargo build --release`

**Step 2: Update test helper and all test cases to async**

**Step 3: Run tests**

Run: `cd D:/GitHub/simse && bun test ./simse-vfs/tests/vfs.test.ts`
Expected: All tests pass

**Step 4: Commit**

```bash
git add simse-vfs/tests/
git commit -m "test(vfs): update tests for async Rust subprocess VFS"
```

---

### Task 15: Build scripts and final verification

**Files:**
- Modify: `simse-vfs/package.json`

**Step 1: Add engine build scripts**

```json
{
    "scripts": {
        "build:engine": "cd engine && cargo build --release",
        "build:engine:debug": "cd engine && cargo build",
        "test:engine": "cd engine && cargo test"
    }
}
```

**Step 2: Full verification pipeline**

Run sequentially:
```bash
cd D:/GitHub/simse/simse-vfs && bun run build:engine
cd D:/GitHub/simse && bun run typecheck
cd D:/GitHub/simse && bun run lint
cd D:/GitHub/simse && bun test ./simse-vfs/tests/vfs.test.ts
cd D:/GitHub/simse/simse-vfs/engine && cargo test
cd D:/GitHub/simse && bun test
```
Expected: All pass

**Step 3: Commit**

```bash
git add simse-vfs/package.json
git commit -m "build(vfs): add Rust engine build scripts"
```
