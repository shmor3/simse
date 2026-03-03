# VFS Disk History & Diff Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add per-file change history, diff, and checkout to the `file://` disk backend using a content-addressed shadow store in `.simse/`.

**Architecture:** New `DiskHistory` struct in `disk.rs` manages a `.simse/objects/` + `.simse/manifests/` shadow store inside the root directory. DiskFs gains 4 new methods (history, diff, diff_versions, checkout) and its write/append methods record old content before mutation. Server routing upgrades 4 VFS-only methods to dual-backend. Existing operations (glob, search, readdir, tree, du) exclude `.simse/` from results.

**Tech Stack:** Rust, std::fs, sha2 crate for SHA256, existing diff.rs module

**Design doc:** `docs/plans/2026-03-03-vfs-disk-history-design.md`

---

### Task 1: Add sha2 dependency and create DiskHistory struct

**Files:**
- Modify: `simse-vfs/Cargo.toml`
- Modify: `simse-vfs/src/disk.rs`

**Step 1: Add sha2 to Cargo.toml**

Add to `[dependencies]`:
```toml
sha2 = "0.10"
```

**Step 2: Add DiskHistory struct and methods to disk.rs**

Add at the top of disk.rs:
```rust
use sha2::{Sha256, Digest};
```

Add the `DiskHistory` struct and its implementation **below the DiskFs impl block** (before the helper functions section). This is a standalone struct — DiskFs will reference it later.

```rust
// ── Shadow History Store ────────────────────────────────────────────────

const SIMSE_DIR: &str = ".simse";

/// Manifest entry for a single file version.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionEntry {
    version: usize,
    hash: String,
    size: u64,
    content_type: String,
    timestamp: u64,
}

/// Manifest for a tracked file (stored as JSON in .simse/manifests/).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileManifest {
    path: String,
    versions: Vec<VersionEntry>,
}

/// Content-addressed shadow history store.
///
/// Stores file versions in `.simse/objects/` using SHA256 hashes.
/// Tracks version metadata in `.simse/manifests/` as JSON files.
pub struct DiskHistory {
    objects_dir: PathBuf,
    manifests_dir: PathBuf,
    max_entries: usize,
}
```

**Step 3: Implement DiskHistory methods**

```rust
impl DiskHistory {
    /// Create a new DiskHistory. Directories are created lazily on first write.
    pub fn new(root: &Path, max_entries: usize) -> Self {
        let simse_dir = root.join(SIMSE_DIR);
        Self {
            objects_dir: simse_dir.join("objects"),
            manifests_dir: simse_dir.join("manifests"),
            max_entries,
        }
    }

    /// Ensure the objects and manifests directories exist.
    fn ensure_dirs(&self) -> Result<(), VfsError> {
        fs::create_dir_all(&self.objects_dir)?;
        fs::create_dir_all(&self.manifests_dir)?;
        Ok(())
    }

    /// Compute SHA256 hex digest of data.
    fn hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Store content in the object store. Returns the hash.
    pub fn store_content(&self, data: &[u8]) -> Result<String, VfsError> {
        self.ensure_dirs()?;
        let hash = Self::hash(data);
        let dir = self.objects_dir.join(&hash[..2]);
        fs::create_dir_all(&dir)?;
        let object_path = dir.join(&hash[2..]);
        if !object_path.exists() {
            fs::write(&object_path, data)?;
        }
        Ok(hash)
    }

    /// Load content from the object store by hash.
    pub fn load_content(&self, hash: &str) -> Result<Vec<u8>, VfsError> {
        if hash.len() < 3 {
            return Err(VfsError::InvalidOperation("Invalid hash".into()));
        }
        let object_path = self.objects_dir.join(&hash[..2]).join(&hash[2..]);
        fs::read(&object_path).map_err(|_| {
            VfsError::NotFound(format!("History object not found: {}", hash))
        })
    }

    /// Get the manifest file path for a given file:// path.
    fn manifest_path(&self, file_path: &str) -> PathBuf {
        // URL-encode the relative path (strip file:// and leading /)
        let relative = file_path
            .strip_prefix(FILE_SCHEME)
            .unwrap_or(file_path)
            .trim_start_matches('/');
        let encoded = relative.replace('/', "%2F");
        self.manifests_dir.join(format!("{}.json", encoded))
    }

    /// Read the manifest for a file. Returns empty manifest if none exists.
    fn read_manifest(&self, file_path: &str) -> Result<FileManifest, VfsError> {
        let path = self.manifest_path(file_path);
        if !path.exists() {
            return Ok(FileManifest {
                path: file_path.to_string(),
                versions: Vec::new(),
            });
        }
        let data = fs::read_to_string(&path)?;
        serde_json::from_str(&data).map_err(VfsError::from)
    }

    /// Write a manifest to disk.
    fn write_manifest(&self, manifest: &FileManifest) -> Result<(), VfsError> {
        self.ensure_dirs()?;
        let path = self.manifest_path(&manifest.path);
        let json = serde_json::to_string_pretty(manifest)?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Record a new version of a file. Call this BEFORE modifying the file.
    ///
    /// `file_path` is the `file://` URI.
    /// `data` is the current raw file content.
    /// `content_type` is "text" or "binary".
    /// `size` is the byte length.
    pub fn record_version(
        &self,
        file_path: &str,
        data: &[u8],
        content_type: &str,
        size: u64,
    ) -> Result<(), VfsError> {
        let hash = self.store_content(data)?;
        let mut manifest = self.read_manifest(file_path)?;

        let version = manifest.versions.last().map(|v| v.version + 1).unwrap_or(1);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        manifest.versions.push(VersionEntry {
            version,
            hash,
            size,
            content_type: content_type.to_string(),
            timestamp,
        });

        // Prune oldest if over limit
        if manifest.versions.len() > self.max_entries {
            let drain = manifest.versions.len() - self.max_entries;
            manifest.versions.drain(..drain);
        }

        self.write_manifest(&manifest)
    }

    /// Get version history for a file.
    pub fn get_history(&self, file_path: &str) -> Result<Vec<VersionEntry>, VfsError> {
        let manifest = self.read_manifest(file_path)?;
        Ok(manifest.versions)
    }

    /// Load the content of a specific version.
    pub fn get_version_content(
        &self,
        file_path: &str,
        version: usize,
    ) -> Result<(String, Vec<u8>), VfsError> {
        let manifest = self.read_manifest(file_path)?;
        let entry = manifest.versions.iter().find(|v| v.version == version).ok_or_else(|| {
            VfsError::NotFound(format!("Version {} not found for {}", version, file_path))
        })?;
        let data = self.load_content(&entry.hash)?;
        Ok((entry.content_type.clone(), data))
    }
}
```

**Step 3: Verify compilation**

Run: `cd simse-vfs && cargo check 2>&1`
Expected: Compiles (DiskHistory exists but isn't used by DiskFs yet)

**Step 4: Commit**

```bash
git add simse-vfs/Cargo.toml simse-vfs/src/disk.rs
git commit -m "feat(simse-vfs): add DiskHistory content-addressed shadow store"
```

---

### Task 2: Wire DiskHistory into DiskFs and add history recording

**Files:**
- Modify: `simse-vfs/src/disk.rs`

**Step 1: Add history field to DiskFs**

Change the DiskFs struct:
```rust
pub struct DiskFs {
    root_directory: PathBuf,
    allowed_paths: Vec<PathBuf>,
    history: Option<DiskHistory>,
}
```

**Step 2: Update DiskFs::new() to accept max_history**

Change the constructor signature and create DiskHistory:
```rust
pub fn new(root_directory: PathBuf, allowed_paths: Vec<PathBuf>, max_history: usize) -> Self {
    let canonical_root =
        fs::canonicalize(&root_directory).unwrap_or(root_directory);
    let canonical_allowed: Vec<PathBuf> = allowed_paths
        .iter()
        .filter_map(|p| fs::canonicalize(p).ok())
        .collect();
    let history = DiskHistory::new(&canonical_root, max_history);
    Self {
        root_directory: canonical_root,
        allowed_paths: canonical_allowed,
        history: Some(history),
    }
}
```

**Step 3: Add history recording to write_file**

In `write_file`, after resolving the path but BEFORE writing, if the file already exists, record its current content:

```rust
// Record history before overwriting (if file exists)
if resolved.exists() && resolved.is_file() {
    if let Some(ref history) = self.history {
        if let Ok(old_data) = fs::read(&resolved) {
            let ct = if is_binary(&old_data) { "binary" } else { "text" };
            let size = old_data.len() as u64;
            let _ = history.record_version(path, &old_data, ct, size);
        }
    }
}
```

Insert this right before `let bytes = if content_type == Some("binary") ...` (both in the `create_parents` branch and the `else` branch — or factor it out after the `let resolved = ...` assignment).

For the `create_parents` branch, the resolved path is computed at the end — so the history recording should happen after the `let resolved = ...` block and before the actual `fs::write`.

**Step 4: Add history recording to append_file**

In `append_file`, BEFORE appending, record current content:

```rust
// Record history before appending
if let Some(ref history) = self.history {
    if let Ok(old_data) = fs::read(&resolved) {
        let ct = if is_binary(&old_data) { "binary" } else { "text" };
        let size = old_data.len() as u64;
        let _ = history.record_version(path, &old_data, ct, size);
    }
}
```

Insert this after `if !resolved.is_file()` check and before `let mut file = OpenOptions::new()...`.

**Step 5: Exclude .simse from walk_dir**

In the `walk_dir` helper function, add a check to skip the `.simse` directory:

```rust
fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), VfsError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        // Skip .simse shadow directory
        if path.file_name().map(|n| n == SIMSE_DIR).unwrap_or(false) {
            continue;
        }
        if path.is_dir() {
            walk_dir(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}
```

**Step 6: Exclude .simse from readdir (non-recursive)**

In the `readdir` method, inside the `else` branch for non-recursive listing, add a filter:

```rust
for entry in fs::read_dir(&resolved)? {
    let entry = entry?;
    let name = entry.file_name().to_string_lossy().into_owned();
    // Skip .simse shadow directory
    if name == SIMSE_DIR {
        continue;
    }
    // ... rest unchanged
}
```

**Step 7: Exclude .simse from readdir_recursive**

In `readdir_recursive`, add the same skip check after the symlink check:

```rust
// Skip .simse shadow directory
if path.file_name().map(|n| n == SIMSE_DIR).unwrap_or(false) {
    continue;
}
```

**Step 8: Exclude .simse from build_tree**

In the `build_tree` helper function, add the same skip:

```rust
// Skip .simse shadow directory
if entry_path.file_name().map(|n| n == SIMSE_DIR).unwrap_or(false) {
    continue;
}
```

**Step 9: Verify compilation**

Run: `cd simse-vfs && cargo check 2>&1`

Note: The `DiskFs::new()` signature changed (added `max_history` parameter). If server.rs calls `DiskFs::new()` with 2 args, it will fail. You need to also update server.rs `handle_initialize` to pass `max_history`:

In `handle_initialize`, change:
```rust
self.disk = init_params.disk.map(|dc| {
    DiskFs::new(
        std::path::PathBuf::from(&dc.root_directory),
        dc.allowed_paths
            .unwrap_or_default()
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect(),
        max_history,  // pass the same max_history used for VFS
    )
});
```

Where `max_history` is the value already computed earlier in the function.

**Step 10: Verify all tests pass**

Run: `cd simse-vfs && cargo test 2>&1`
Expected: All existing tests pass (disk tests create tempdir, so .simse is created there)

**Step 11: Commit**

```bash
git add simse-vfs/src/disk.rs simse-vfs/src/server.rs
git commit -m "feat(simse-vfs): wire DiskHistory into DiskFs with history recording on write/append"
```

---

### Task 3: Add history, diff, diff_versions, and checkout methods to DiskFs

**Files:**
- Modify: `simse-vfs/src/disk.rs`

**Step 1: Add diff import**

At the top of disk.rs, add:
```rust
use crate::diff::compute_diff;
use crate::protocol::HistoryEntry;
```

**Step 2: Add history method**

```rust
/// Get version history for a file.
pub fn history(&self, path: &str) -> Result<Vec<HistoryEntry>, VfsError> {
    let _ = self.resolve_path(path)?; // validate path exists and is in sandbox
    let history = self.history.as_ref().ok_or(VfsError::InvalidOperation(
        "History not configured".into(),
    ))?;
    let versions = history.get_history(path)?;
    Ok(versions
        .into_iter()
        .map(|v| {
            // Load content for each version to include in response
            let (content_type, data) = history
                .get_version_content(path, v.version)
                .unwrap_or_else(|_| ("text".to_string(), Vec::new()));
            let (text, base64_data) = if content_type == "binary" {
                (None, Some(base64::engine::general_purpose::STANDARD.encode(&data)))
            } else {
                (Some(String::from_utf8_lossy(&data).into_owned()), None)
            };
            HistoryEntry {
                version: v.version,
                content_type,
                text,
                base64: base64_data,
                size: v.size,
                timestamp: v.timestamp,
            }
        })
        .collect())
}
```

**Step 3: Add diff method**

Diff compares two file:// files on disk. Reads both, runs `compute_diff`.

```rust
/// Diff two files on disk.
pub fn diff(
    &self,
    old_path: &str,
    new_path: &str,
    context: usize,
) -> Result<DiffResultOutput, VfsError> {
    let old_resolved = self.resolve_path(old_path)?;
    let new_resolved = self.resolve_path(new_path)?;

    let old_data = fs::read(&old_resolved).map_err(|e| map_io_error(e, old_path))?;
    let new_data = fs::read(&new_resolved).map_err(|e| map_io_error(e, new_path))?;

    let old_text = String::from_utf8_lossy(&old_data);
    let new_text = String::from_utf8_lossy(&new_data);

    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    compute_diff(&old_lines, &new_lines, context, u32::MAX).map_err(|e| {
        VfsError::InvalidOperation(format!("Diff failed: {}", e))
    })
}
```

Note: `DiffResultOutput` is the name used in server.rs for the type from `crate::vfs`. Actually, looking at the imports in server.rs it uses `DiffResultOutput` from `crate::vfs`. But `compute_diff` in `diff.rs` returns `DiffOutput`. Check what `DiffOutput` is:

The return type of `compute_diff` is `Result<DiffOutput, String>` where `DiffOutput` is defined in `diff.rs`. It contains `old_path`, `new_path`, `hunks`, `additions`, `deletions`. We need to set old_path and new_path ourselves since `compute_diff` doesn't set them.

Actually, looking at the VFS code more carefully, `vfs.rs` has its own `DiffResultOutput` wrapper. Let me check what `compute_diff` actually returns and what the server handlers expect.

The server uses `DiffResultOutput` from `crate::vfs` in `convert_diff_output`. For the disk handlers, we should return the same type structure. Let me check `diff.rs` `DiffOutput`:

```rust
pub struct DiffOutput {
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub deletions: usize,
}
```

It does NOT include `old_path` / `new_path`. The VFS adds those in its own wrapper `DiffResultOutput`. For the disk backend, the server handler will need to add paths too.

So the DiskFs `diff` method should return the `DiffOutput` from `crate::diff` directly, and the server handler adds the paths.

Revised:
```rust
use crate::diff::{compute_diff, DiffOutput};

/// Diff two files on disk. Returns raw diff output (paths added by server handler).
pub fn diff(
    &self,
    old_path: &str,
    new_path: &str,
    context: usize,
) -> Result<DiffOutput, VfsError> {
    let old_resolved = self.resolve_path(old_path)?;
    let new_resolved = self.resolve_path(new_path)?;

    let old_data = fs::read(&old_resolved).map_err(|e| map_io_error(e, old_path))?;
    let new_data = fs::read(&new_resolved).map_err(|e| map_io_error(e, new_path))?;

    let old_text = String::from_utf8_lossy(&old_data);
    let new_text = String::from_utf8_lossy(&new_data);

    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    compute_diff(&old_lines, &new_lines, context, u32::MAX).map_err(|e| {
        VfsError::InvalidOperation(format!("Diff failed: {}", e))
    })
}
```

**Step 4: Add diff_versions method**

```rust
/// Diff two versions of the same file.
///
/// If `new_version` is None, diffs against the current file content.
pub fn diff_versions(
    &self,
    path: &str,
    old_version: usize,
    new_version: Option<usize>,
    context: usize,
) -> Result<DiffOutput, VfsError> {
    let resolved = self.resolve_path(path)?;
    let history = self.history.as_ref().ok_or(VfsError::InvalidOperation(
        "History not configured".into(),
    ))?;

    // Load old version from history
    let (_old_ct, old_data) = history.get_version_content(path, old_version)?;
    let old_text = String::from_utf8_lossy(&old_data);

    // Load new version: from history if specified, otherwise current file
    let new_text_owned;
    let new_text: std::borrow::Cow<str> = match new_version {
        Some(ver) => {
            let (_new_ct, new_data) = history.get_version_content(path, ver)?;
            new_text_owned = String::from_utf8_lossy(&new_data).into_owned();
            std::borrow::Cow::Owned(new_text_owned)
        }
        None => {
            let data = fs::read(&resolved).map_err(|e| map_io_error(e, path))?;
            new_text_owned = String::from_utf8_lossy(&data).into_owned();
            std::borrow::Cow::Owned(new_text_owned)
        }
    };

    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    compute_diff(&old_lines, &new_lines, context, u32::MAX).map_err(|e| {
        VfsError::InvalidOperation(format!("Diff failed: {}", e))
    })
}
```

**Step 5: Add checkout method**

```rust
/// Checkout (revert) a file to a specific version.
///
/// Records the current content as a new version before reverting.
pub fn checkout(&self, path: &str, version: usize) -> Result<(), VfsError> {
    let resolved = self.resolve_path(path)?;
    let history = self.history.as_ref().ok_or(VfsError::InvalidOperation(
        "History not configured".into(),
    ))?;

    // Record current content before reverting
    if resolved.is_file() {
        let current_data = fs::read(&resolved).map_err(|e| map_io_error(e, path))?;
        let ct = if is_binary(&current_data) { "binary" } else { "text" };
        let size = current_data.len() as u64;
        history.record_version(path, &current_data, ct, size)?;
    }

    // Load the requested version and write it
    let (_ct, data) = history.get_version_content(path, version)?;
    fs::write(&resolved, &data).map_err(|e| map_io_error(e, path))?;

    Ok(())
}
```

**Step 6: Verify compilation**

Run: `cd simse-vfs && cargo check 2>&1`

**Step 7: Commit**

```bash
git add simse-vfs/src/disk.rs
git commit -m "feat(simse-vfs): add history, diff, diff_versions, and checkout to DiskFs"
```

---

### Task 4: Update server routing for dual-backend history/diff/checkout

**Files:**
- Modify: `simse-vfs/src/server.rs`

**Step 1: Update imports**

Add to the disk imports in server.rs (near the top where `DiskFs`, `DiskSearchMode`, etc. are imported):
```rust
// Already imported: DiskFs, DiskSearchMode, DiskSearchOptions, DiskSearchResult
// No new imports needed from disk module — the methods return protocol types directly
```

Add diff import if not already present:
```rust
use crate::diff::DiffOutput;
```

**Step 2: Update vfs/history dispatch**

Replace the VFS-only guard with dual-backend routing:

```rust
"vfs/history" => match detect_scheme(&req.params) {
    Scheme::Vfs => self.with_vfs(|vfs| handle_history(vfs, req.params)),
    Scheme::Disk => self.with_disk(|disk| handle_disk_history(disk, req.params)),
},
```

**Step 3: Update vfs/diff dispatch**

```rust
"vfs/diff" => match detect_scheme_dual(&req.params, "oldPath", "newPath") {
    Ok(Scheme::Vfs) => self.with_vfs(|vfs| handle_diff(vfs, req.params)),
    Ok(Scheme::Disk) => self.with_disk(|disk| handle_disk_diff(disk, req.params)),
    Err(e) => Err(e),
},
```

**Step 4: Update vfs/diffVersions dispatch**

```rust
"vfs/diffVersions" => match detect_scheme(&req.params) {
    Scheme::Vfs => self.with_vfs(|vfs| handle_diff_versions(vfs, req.params)),
    Scheme::Disk => self.with_disk(|disk| handle_disk_diff_versions(disk, req.params)),
},
```

**Step 5: Update vfs/checkout dispatch**

```rust
"vfs/checkout" => match detect_scheme(&req.params) {
    Scheme::Vfs => self.with_vfs_mut(|vfs| handle_checkout(vfs, req.params)),
    Scheme::Disk => self.with_disk(|disk| handle_disk_checkout(disk, req.params)),
},
```

Note: `vfs/checkout` for disk uses `with_disk` (not `with_disk_mut`) because DiskFs methods take `&self` — the filesystem itself is the mutable state.

**Step 6: Add disk handler functions**

Add these free-standing handlers alongside the existing disk handlers:

```rust
fn handle_disk_history(
    disk: &DiskFs,
    params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
    let p: PathParams = parse_params(params)?;
    let entries = disk.history(&p.path)?;
    Ok(serde_json::json!({ "entries": entries }))
}

fn handle_disk_diff(
    disk: &DiskFs,
    params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
    let p: DiffParams = parse_params(params)?;
    let d = disk.diff(&p.old_path, &p.new_path, p.context.unwrap_or(3))?;
    Ok(serde_json::to_value(convert_diff_output_from_raw(
        &p.old_path, &p.new_path, d,
    ))?)
}

fn handle_disk_diff_versions(
    disk: &DiskFs,
    params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
    let p: DiffVersionsParams = parse_params(params)?;
    let d = disk.diff_versions(&p.path, p.old_version, p.new_version, p.context.unwrap_or(3))?;
    let old_label = format!("{}@v{}", p.path, p.old_version);
    let new_label = match p.new_version {
        Some(v) => format!("{}@v{}", p.path, v),
        None => format!("{} (current)", p.path),
    };
    Ok(serde_json::to_value(convert_diff_output_from_raw(
        &old_label, &new_label, d,
    ))?)
}

fn handle_disk_checkout(
    disk: &DiskFs,
    params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
    let p: CheckoutParams = parse_params(params)?;
    disk.checkout(&p.path, p.version)?;
    Ok(serde_json::json!({ "ok": true }))
}
```

**Step 7: Add convert_diff_output_from_raw helper**

This converts the raw `DiffOutput` from `diff.rs` into the protocol `DiffResult`:

```rust
fn convert_diff_output_from_raw(old_path: &str, new_path: &str, d: DiffOutput) -> DiffResult {
    DiffResult {
        old_path: old_path.to_string(),
        new_path: new_path.to_string(),
        additions: d.additions,
        deletions: d.deletions,
        hunks: d
            .hunks
            .into_iter()
            .map(|h| DiffHunk {
                old_start: h.old_start,
                old_count: h.old_count,
                new_start: h.new_start,
                new_count: h.new_count,
                lines: h
                    .lines
                    .into_iter()
                    .map(|l| DiffLineResult {
                        line_type: l.line_type.as_str().to_string(),
                        text: l.text,
                        old_line: l.old_line,
                        new_line: l.new_line,
                    })
                    .collect(),
            })
            .collect(),
    }
}
```

Note: The existing `convert_diff_output` converts from `DiffResultOutput` (vfs.rs type). This new function converts from `DiffOutput` (diff.rs type). Different source types, same target.

**Step 8: Add disk event for checkout**

In the dispatch, after `vfs/checkout` Disk case succeeds, emit an event:

The simplest way: add a `DiskEvent` for checkout in the disk event handling section. After the match for `"vfs/checkout"`, push a checkout event:

```rust
// In the dispatch method, where disk events are collected:
"vfs/checkout" => match detect_scheme(&req.params) {
    Scheme::Vfs => self.with_vfs_mut(|vfs| handle_checkout(vfs, req.params.clone())),
    Scheme::Disk => {
        let p: Result<CheckoutParams, _> = serde_json::from_value(req.params.clone());
        let result = self.with_disk(|disk| handle_disk_checkout(disk, req.params));
        if result.is_ok() {
            if let Ok(p) = p {
                disk_events.push(DiskEvent {
                    params: serde_json::json!({
                        "type": "write",
                        "path": p.path,
                        "source": "disk"
                    }),
                });
            }
        }
        result
    }
},
```

**Step 9: Verify compilation and all tests pass**

Run: `cd simse-vfs && cargo check && cargo test 2>&1`

**Step 10: Commit**

```bash
git add simse-vfs/src/server.rs
git commit -m "feat(simse-vfs): route history/diff/diffVersions/checkout to disk backend"
```

---

### Task 5: Write integration tests for disk history/diff/checkout

**Files:**
- Modify: `simse-vfs/tests/integration.rs`

**Step 1: Add integration tests**

Add these tests:

1. `disk_history_tracks_writes` — Write v1, write v2, call `vfs/history`, verify 1 entry (old content captured before second write)

2. `disk_history_tracks_appends` — Write, append, call `vfs/history`, verify 1 entry (pre-append content)

3. `disk_diff_two_files` — Write two files with different content, call `vfs/diff`, verify hunks/additions/deletions

4. `disk_diff_versions` — Write v1, write v2, write v3, call `vfs/diffVersions` with v1 vs current, verify diff output

5. `disk_checkout` — Write v1, write v2, checkout v1, read back, verify v1 content restored

6. `disk_checkout_records_history` — Write v1, write v2, checkout v1, verify history has 3 entries (original, before v2 write, before checkout revert)

7. `disk_history_empty_for_new_file` — Write new file (no prior version), call `vfs/history`, verify empty entries

8. `disk_simse_dir_hidden` — Write files, verify `.simse` doesn't appear in `vfs/readdir`, `vfs/glob`, `vfs/tree`

Each test creates its own `tempdir()` and initializes with disk config, following the existing test pattern.

**Step 2: Run tests**

Run: `cd simse-vfs && cargo test 2>&1`
Expected: All tests pass (old + new)

**Step 3: Commit**

```bash
git add simse-vfs/tests/integration.rs
git commit -m "test(simse-vfs): add integration tests for disk history, diff, and checkout"
```

---

### Task 6: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Update simse-vfs description**

In the repository layout, the simse-vfs line already says `vfs:// in-memory + file:// disk`. No change needed there.

In the simse-vfs module layout, it already lists `disk.rs`. Add a note about the history capability:

```
simse-vfs/                  # Pure Rust crate — virtual filesystem
  src/
    vfs.rs                  # Core VFS implementation (vfs:// in-memory backend)
    disk.rs                 # DiskFs: real filesystem operations (file:// backend, shadow history)
    diff.rs                 # Diff generation (Myers algorithm, shared by both backends)
    glob.rs                 # Glob pattern matching
    search.rs               # File search implementation
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update simse-vfs disk backend description with history support"
```

---

## Summary

| Task | Description | Priority |
|------|-------------|----------|
| 1 | DiskHistory struct with content-addressed store | Must |
| 2 | Wire into DiskFs, history recording on write/append, .simse exclusions | Must |
| 3 | Add history, diff, diff_versions, checkout methods to DiskFs | Must |
| 4 | Server routing updates (4 methods become dual-backend) | Must |
| 5 | Integration tests (8 tests) | Must |
| 6 | Update CLAUDE.md | Must |
