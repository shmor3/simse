# simse-vfs Disk History & Diff Design

**Date:** 2026-03-03
**Status:** Approved

## Overview

Add per-file change history, diff, and checkout to the disk backend (`file://` paths) using a content-addressed shadow store in `.simse/` within the root directory. This brings the 4 remaining VFS-only methods (`history`, `diff`, `diffVersions`, `checkout`) to the disk backend.

## Architecture

### Shadow History Store

When DiskFs is initialized, it creates a `.simse/` directory inside `root_directory`:

```
<root_directory>/
  .simse/
    objects/              # Content-addressed blobs (SHA256)
      ab/cd1234ef...      # sha256[:2] / sha256[2:]
    manifests/            # One JSON per tracked file
      src%2Fmain.rs.json  # URL-encoded relative path → manifest
```

The `.simse/` directory is excluded from `glob`, `search`, `readdir`, `tree`, and `du` operations — it's internal bookkeeping, not user content.

### Content-Addressed Object Store

File content is stored by SHA256 hash. Two-character prefix directory for filesystem friendliness:

```
objects/ab/cdef0123456789...  # raw file bytes
```

Identical content deduplicates automatically. Objects are never deleted (orphan collection is not implemented — YAGNI).

### Manifest Files

One manifest per tracked file. Path is URL-encoded to flatten the namespace:

```json
{
  "path": "file:///src/main.rs",
  "versions": [
    {
      "version": 1,
      "hash": "abcdef0123456789...",
      "size": 1234,
      "contentType": "text",
      "timestamp": 1709500000000
    }
  ]
}
```

Version numbers are 1-indexed and monotonically increasing, matching VFS behavior. Max entries per file controlled by `history.maxEntriesPerFile` (default 50). Oldest entries pruned when limit exceeded.

## DiskHistory Module

New struct managing the shadow store:

```rust
pub struct DiskHistory {
    objects_dir: PathBuf,     // .simse/objects/
    manifests_dir: PathBuf,   // .simse/manifests/
    max_entries: usize,       // default 50
}
```

### Methods

| Method | Description |
|--------|-------------|
| `new(root, max_entries)` | Create dirs if needed, return DiskHistory |
| `store_content(data: &[u8]) -> String` | SHA256 hash, write to objects/, return hash |
| `load_content(hash: &str) -> Vec<u8>` | Read blob from objects/ |
| `record_version(path, data, content_type, size)` | Append version to manifest, prune if over limit |
| `get_history(path) -> Vec<VersionEntry>` | Read manifest, return version list |
| `get_version_content(path, version) -> (String, Vec<u8>)` | Look up hash, return (content_type, data) |
| `manifest_path(path) -> PathBuf` | URL-encode path, return manifests/ path |

## DiskFs Changes

### New Field

```rust
pub struct DiskFs {
    root_directory: PathBuf,
    allowed_paths: Vec<PathBuf>,
    history: Option<DiskHistory>,  // NEW
}
```

`history` is `Some` when the initialize config includes history settings (always, when disk is configured). `None` only if explicitly disabled.

### New Methods (4)

| Method | Description |
|--------|-------------|
| `history(path) -> Vec<HistoryEntry>` | Delegate to DiskHistory, convert to protocol type |
| `diff(old_path, new_path, context) -> DiffResult` | Read both files, call `crate::diff::compute_diff` |
| `diff_versions(path, old_ver, new_ver, context) -> DiffResult` | Load version content from history, diff |
| `checkout(path, version)` | Record current as new version, replace with old version |

### Modified Methods (2)

| Method | Change |
|--------|--------|
| `write_file` | Before overwriting existing file, call `history.record_version()` with current content |
| `append_file` | Before appending to existing file, call `history.record_version()` with current content |

### Exclusions

The `.simse/` directory must be excluded from:
- `glob` — skip `.simse` directory during walk
- `search` — skip `.simse` directory during walk
- `readdir` — filter out `.simse` entry
- `tree` — skip `.simse` directory
- `du` — skip `.simse` directory

The simplest approach: check if a path component is `.simse` in the `walk_dir` helper and the `readdir` methods.

## Server Routing Changes

The 4 methods move from VFS-only to dual-backend:

| Method | Before | After |
|--------|--------|-------|
| `vfs/history` | VFS-only, file:// rejected | Dual: VFS or Disk based on scheme |
| `vfs/diff` | VFS-only, file:// rejected | Dual: VFS or Disk (dual-path scheme check) |
| `vfs/diffVersions` | VFS-only, file:// rejected | Dual: VFS or Disk based on scheme |
| `vfs/checkout` | VFS-only, file:// rejected | Dual: VFS or Disk based on scheme |

No new handler functions needed for `vfs/diff` on disk — it's just reading two files and diffing, which the existing diff algorithm handles.

## Protocol Changes

None. The existing param/result types work for both backends:
- `PathParams` for history
- `DiffParams` for diff (oldPath, newPath, context)
- `DiffVersionsParams` for diffVersions (path, oldVersion, newVersion, context)
- `CheckoutParams` for checkout (path, version)
- `DiffResult`, `HistoryEntry` for responses

## Configuration

The `InitializeParams.history.maxEntriesPerFile` setting applies to both VFS and disk backends. No new config fields needed.

DiskHistory is automatically created when disk config is provided. The `.simse/` directory is created lazily on first write that triggers history recording.

## What Does NOT Change

- `vfs.rs` — completely untouched
- `diff.rs` — unchanged (reused by DiskFs)
- `protocol.rs` — unchanged
- `error.rs` — unchanged
- `glob.rs`, `search.rs`, `path.rs` — unchanged
- All existing vfs:// behavior — 100% backwards compatible
- `vfs/snapshot`, `vfs/restore`, `vfs/clear`, `vfs/transaction`, `vfs/metrics` — remain VFS-only
