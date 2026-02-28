# simse-vfs Rust Refactor Design

**Date:** 2026-02-28

## Goal

Rewrite the simse-vfs core logic in Rust as a subprocess server communicating via JSON-RPC 2.0 over NDJSON stdio, matching the simse-engine pattern. The TypeScript package becomes a thin client that preserves the existing `VirtualFS` public API.

## Architecture

```
simse-vfs/
  src/                    ← TypeScript: thin JSON-RPC client
    lib.ts                ← barrel exports (unchanged public API)
    types.ts              ← unchanged type definitions
    errors.ts             ← unchanged error factories
    logger.ts             ← unchanged
    exec.ts               ← unchanged exec interface
    client.ts             ← NEW: JSON-RPC client over stdio
    vfs.ts                ← REWRITE: VirtualFS impl delegates to Rust subprocess
    vfs-disk.ts           ← KEEP: disk commit/load stays in TS (accesses host FS)
    path-utils.ts         ← KEEP: used by TS client for pre-validation
    validators.ts         ← KEEP: used by vfs-disk.ts (host-side validation)
  engine/                 ← NEW: Rust crate (simse-vfs-engine)
    Cargo.toml
    src/
      lib.rs              ← module declarations
      main.rs             ← stdio server entry point
      error.rs            ← VfsError enum
      protocol.rs         ← JSON-RPC request/response types
      server.rs           ← request dispatcher
      transport.rs        ← NDJSON frame reader/writer
      vfs.rs              ← in-memory VFS (nodes, limits, history)
      path.rs             ← path normalization + validation
      glob.rs             ← glob matching, brace expansion, negation
      search.rs           ← substring + regex search with context
      diff.rs             ← Myers diff algorithm
      snapshot.rs         ← snapshot/restore serialization
      transaction.rs      ← atomic transaction execution
  tests/
    vfs.test.ts           ← existing tests (should still pass)
    vfs-disk.test.ts      ← unchanged
```

### Dependency Direction

```
simse-vfs (TS client) → spawns → simse-vfs-engine (Rust binary)
simse → depends on → simse-vfs (via workspace protocol)
```

## Protocol

JSON-RPC 2.0 over NDJSON stdio (same pattern as simse-engine/ACP).

### Request/Response Format

```jsonc
// Request
{"jsonrpc":"2.0","id":1,"method":"vfs/writeFile","params":{"path":"/src/foo.ts","content":"hello","contentType":"text"}}

// Success response
{"jsonrpc":"2.0","id":1,"result":null}

// Error response
{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"File not found","data":{"vfsCode":"VFS_NOT_FOUND","metadata":{"path":"/missing.txt"}}}}

// Notification (file change callback)
{"jsonrpc":"2.0","method":"vfs/event","params":{"type":"write","path":"/src/foo.ts","size":5}}
```

### Methods

Methods map 1:1 to the VirtualFS interface:

| Method | Params | Result |
|--------|--------|--------|
| `vfs/readFile` | `{path}` | `{contentType, text?, data?, size}` |
| `vfs/writeFile` | `{path, content, contentType?, createParents?}` | `null` |
| `vfs/appendFile` | `{path, content}` | `null` |
| `vfs/deleteFile` | `{path}` | `{deleted: bool}` |
| `vfs/mkdir` | `{path, recursive?}` | `null` |
| `vfs/readdir` | `{path, recursive?}` | `[{name, type}]` |
| `vfs/rmdir` | `{path, recursive?}` | `{deleted: bool}` |
| `vfs/stat` | `{path}` | `{path, type, size, createdAt, modifiedAt}` |
| `vfs/exists` | `{path}` | `{exists: bool}` |
| `vfs/rename` | `{oldPath, newPath}` | `null` |
| `vfs/copy` | `{src, dest, overwrite?, recursive?}` | `null` |
| `vfs/glob` | `{pattern}` | `[string]` |
| `vfs/tree` | `{path?}` | `{tree: string}` |
| `vfs/du` | `{path}` | `{size: number}` |
| `vfs/search` | `{query, glob?, maxResults?, mode?, contextBefore?, contextAfter?, countOnly?}` | `[{path, line, column, match, contextBefore?, contextAfter?}]` or `{count: number}` |
| `vfs/history` | `{path}` | `[{version, contentType, text?, base64?, size, timestamp}]` |
| `vfs/diff` | `{oldPath, newPath, context?}` | `{oldPath, newPath, hunks, additions, deletions}` |
| `vfs/diffVersions` | `{path, oldVersion, newVersion?, context?}` | same as diff |
| `vfs/checkout` | `{path, version}` | `null` |
| `vfs/snapshot` | `{}` | `{files, directories}` |
| `vfs/restore` | `{files, directories}` | `null` |
| `vfs/clear` | `{}` | `null` |
| `vfs/transaction` | `{ops: [{type, ...}]}` | `null` |
| `vfs/metrics` | `{}` | `{totalSize, nodeCount, fileCount, directoryCount}` |

### Binary Data

`Uint8Array` content is transmitted as base64 strings. The TS client encodes on write and decodes on read. This matches the existing VFS snapshot format.

### Initialization

When `createVirtualFS()` is called, the TS client:
1. Spawns `simse-vfs-engine` as a child process
2. Sends an `initialize` request with limits and history options
3. Awaits `initialized` response
4. Returns the `VirtualFS` interface backed by JSON-RPC calls

### Event Notifications

File change callbacks (`VFSCallbacks`) are delivered as JSON-RPC notifications from the Rust server:

```jsonc
{"jsonrpc":"2.0","method":"vfs/event","params":{"type":"write","path":"/foo.ts","size":42}}
{"jsonrpc":"2.0","method":"vfs/event","params":{"type":"delete","path":"/old.ts"}}
{"jsonrpc":"2.0","method":"vfs/event","params":{"type":"rename","oldPath":"/a.ts","newPath":"/b.ts"}}
{"jsonrpc":"2.0","method":"vfs/event","params":{"type":"mkdir","path":"/new-dir"}}
```

The TS client routes these to the `VFSCallbacks` registered in options.

## What Stays in TypeScript

- **`vfs-disk.ts`**: Needs `node:fs` access to the host filesystem. The Rust process is sandboxed.
- **`path-utils.ts`**: Used by TS client for pre-validation and by `vfs-disk.ts`.
- **`validators.ts`**: Used by `vfs-disk.ts` for pre-commit host-side validation.
- **`types.ts`, `errors.ts`, `logger.ts`, `exec.ts`**: Pure types/interfaces.

## What Moves to Rust

All core VFS logic currently in `vfs.ts` (~1,700 lines):
- In-memory node tree (HashMap-based)
- Path normalization + validation
- Glob matching (brace expansion, negation, `**` wildcards)
- Search (substring + regex, context lines, count-only)
- Diff (Myers algorithm, hunks, history tracking)
- Snapshot/restore
- Transactions (atomic with rollback)
- Limits enforcement

## Rust Crate Structure

### Dependencies

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
thiserror = "2"
base64 = "0.22"
```

No async runtime needed — the server is synchronous (single-threaded, blocking stdio). VFS operations are all in-memory and fast.

### Error Handling

```rust
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
    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}
```

Each variant maps to existing VFS error codes (`VFS_INVALID_PATH`, `VFS_NOT_FOUND`, etc.).

## Backward Compatibility

The public API (`VirtualFS` interface, all types in `types.ts`) stays identical. `createVirtualFS()` changes internally to spawn the Rust subprocess but the signature and return type are unchanged. All existing tests should pass without modification.

One change: `createVirtualFS()` becomes **async** (returns `Promise<VirtualFS>`) since spawning a subprocess is asynchronous. Alternatively, we can spawn lazily on first call with an internal ready promise.

## Non-Goals

- No async runtime in Rust (tokio/async-std) — synchronous stdio is sufficient
- No disk I/O in Rust — `vfs-disk.ts` stays in TypeScript
- No Firecracker integration yet — that comes later as an `ExecBackend`
- No cross-compilation matrix — build for host platform only initially
