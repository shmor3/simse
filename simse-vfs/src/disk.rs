use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine;
use regex::Regex;

use crate::error::VfsError;
use crate::glob::{expand_braces, match_parts};
use crate::protocol::{DirEntry, ReadFileResult, SearchResult, StatResult};
use crate::search::{search_text, SearchMode, SearchOptions};

// ── Constants ───────────────────────────────────────────────────────────────

const FILE_SCHEME: &str = "file://";

// ── DiskFs ──────────────────────────────────────────────────────────────────

/// Real filesystem backend with sandboxing.
///
/// All paths must use the `file://` scheme. Path resolution canonicalizes
/// the target and verifies it falls within `root_directory` or one of the
/// `allowed_paths` to prevent directory traversal.
pub struct DiskFs {
    root_directory: PathBuf,
    allowed_paths: Vec<PathBuf>,
}

impl DiskFs {
    /// Create a new `DiskFs` rooted at `root_directory`.
    ///
    /// Canonicalizes `root_directory` and `allowed_paths` at construction
    /// time so that every subsequent sandbox check is a cheap prefix
    /// comparison instead of a syscall.
    pub fn new(root_directory: PathBuf, allowed_paths: Vec<PathBuf>) -> Self {
        let canonical_root =
            fs::canonicalize(&root_directory).unwrap_or(root_directory);
        let canonical_allowed: Vec<PathBuf> = allowed_paths
            .iter()
            .filter_map(|p| fs::canonicalize(p).ok())
            .collect();
        Self {
            root_directory: canonical_root,
            allowed_paths: canonical_allowed,
        }
    }

    // ── Path resolution ─────────────────────────────────────────────────

    /// Resolve a `file://`-prefixed path to an absolute path on disk.
    ///
    /// The path is joined against the root directory and canonicalized.
    /// The canonical result must fall within the root or an allowed path.
    pub fn resolve_path(&self, file_path: &str) -> Result<PathBuf, VfsError> {
        let local = strip_file_scheme(file_path)?;
        let local = local.strip_prefix('/').unwrap_or(local);

        let joined = self.root_directory.join(local);
        let canonical = fs::canonicalize(&joined).map_err(|e| {
            VfsError::NotFound(format!("{}: {}", joined.display(), e))
        })?;

        self.check_sandbox(&canonical, file_path)
    }

    /// Resolve a `file://` path for write operations where the target may
    /// not yet exist. Canonicalizes the parent directory instead.
    pub fn resolve_parent_path(&self, file_path: &str) -> Result<PathBuf, VfsError> {
        let local = strip_file_scheme(file_path)?;
        let local = local.strip_prefix('/').unwrap_or(local);

        let joined = self.root_directory.join(local);

        // If the path already exists, canonicalize it directly.
        if joined.exists() {
            let canonical = fs::canonicalize(&joined).map_err(|e| {
                VfsError::NotFound(format!("{}: {}", joined.display(), e))
            })?;
            return self.check_sandbox(&canonical, file_path);
        }

        // Otherwise canonicalize the parent and append the file name.
        let parent = joined.parent().ok_or_else(|| {
            VfsError::InvalidPath("Cannot determine parent directory".into())
        })?;

        let file_name = joined.file_name().ok_or_else(|| {
            VfsError::InvalidPath("Cannot determine file name".into())
        })?;

        let canonical_parent = fs::canonicalize(parent).map_err(|e| {
            VfsError::NotFound(format!("{}: {}", parent.display(), e))
        })?;

        let canonical = canonical_parent.join(file_name);

        self.check_sandbox(&canonical, file_path)
    }

    /// Verify that a canonical path falls within the sandbox.
    ///
    /// Both `self.root_directory` and `self.allowed_paths` were
    /// canonicalized at construction time, so no extra syscalls here.
    fn check_sandbox(&self, canonical: &Path, original: &str) -> Result<PathBuf, VfsError> {
        if canonical.starts_with(&self.root_directory) {
            return Ok(canonical.to_path_buf());
        }

        for allowed in &self.allowed_paths {
            if canonical.starts_with(allowed) {
                return Ok(canonical.to_path_buf());
            }
        }

        Err(VfsError::InvalidPath(format!(
            "Path '{}' is outside sandbox root",
            original
        )))
    }

    // ── File operations ─────────────────────────────────────────────────

    /// Read a file from disk. Returns text or base64 depending on content.
    pub fn read_file(&self, path: &str) -> Result<ReadFileResult, VfsError> {
        let resolved = self.resolve_path(path)?;

        if !resolved.is_file() {
            return Err(VfsError::NotAFile(path.to_string()));
        }

        let data = fs::read(&resolved).map_err(|e| map_io_error(e, path))?;
        let size = data.len() as u64;

        if is_binary(&data) {
            Ok(ReadFileResult {
                content_type: "binary".to_string(),
                text: None,
                data: Some(base64::engine::general_purpose::STANDARD.encode(&data)),
                size,
            })
        } else {
            let text = String::from_utf8_lossy(&data).into_owned();
            Ok(ReadFileResult {
                content_type: "text".to_string(),
                text: Some(text),
                data: None,
                size,
            })
        }
    }

    /// Write content to a file on disk.
    ///
    /// When `create_parents` is true, validates the sandbox constraint
    /// BEFORE creating any parent directories to prevent TOCTOU races.
    pub fn write_file(
        &self,
        path: &str,
        content: &str,
        content_type: Option<&str>,
        create_parents: bool,
    ) -> Result<(), VfsError> {
        let resolved = if create_parents {
            let local = strip_file_scheme(path)?;
            let local = local.strip_prefix('/').unwrap_or(local);
            let joined = self.root_directory.join(local);

            // Validate sandbox BEFORE creating parent directories.
            if let Some(parent) = joined.parent() {
                let mut check_path = parent;
                while !check_path.exists() {
                    check_path = check_path.parent().ok_or_else(|| {
                        VfsError::InvalidPath("Cannot determine parent directory".into())
                    })?;
                }
                let canonical_ancestor =
                    fs::canonicalize(check_path).map_err(|e| {
                        VfsError::NotFound(format!("{}: {}", check_path.display(), e))
                    })?;
                self.check_sandbox(&canonical_ancestor, path)?;

                // Now safe to create parent directories.
                fs::create_dir_all(parent).map_err(|e| map_io_error(e, path))?;
            }
            self.resolve_parent_path(path)?
        } else {
            self.resolve_parent_path(path)?
        };

        let bytes = if content_type == Some("binary") {
            base64::engine::general_purpose::STANDARD
                .decode(content)
                .map_err(|e| VfsError::InvalidOperation(format!("Invalid base64: {}", e)))?
        } else {
            content.as_bytes().to_vec()
        };

        fs::write(&resolved, &bytes).map_err(|e| map_io_error(e, path))?;
        Ok(())
    }

    /// Append text content to an existing file.
    pub fn append_file(&self, path: &str, content: &str) -> Result<(), VfsError> {
        let resolved = self.resolve_path(path)?;

        if !resolved.is_file() {
            return Err(VfsError::NotAFile(path.to_string()));
        }

        let mut file = OpenOptions::new().append(true).open(&resolved)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    /// Delete a file from disk.
    pub fn delete_file(&self, path: &str) -> Result<(), VfsError> {
        let resolved = self.resolve_path(path)?;

        if !resolved.is_file() {
            return Err(VfsError::NotAFile(path.to_string()));
        }

        fs::remove_file(&resolved).map_err(|e| map_io_error(e, path))?;
        Ok(())
    }

    /// Get metadata for a file or directory.
    pub fn stat(&self, path: &str) -> Result<StatResult, VfsError> {
        let resolved = self.resolve_path(path)?;
        let meta = fs::metadata(&resolved)?;

        let node_type = if meta.is_file() {
            "file"
        } else if meta.is_dir() {
            "directory"
        } else {
            "other"
        };

        let created_at = meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Ok(StatResult {
            path: path.to_string(),
            node_type: node_type.to_string(),
            size: meta.len(),
            created_at,
            modified_at,
        })
    }

    /// Check whether a path exists on disk.
    pub fn exists(&self, path: &str) -> Result<bool, VfsError> {
        match self.resolve_path(path) {
            Ok(resolved) => Ok(resolved.exists()),
            Err(VfsError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Create a directory.
    ///
    /// Validates the sandbox constraint BEFORE creating any directories
    /// by walking up to the nearest existing ancestor and canonicalizing it.
    pub fn mkdir(&self, path: &str, recursive: bool) -> Result<(), VfsError> {
        let local = strip_file_scheme(path)?;
        let local = local.strip_prefix('/').unwrap_or(local);
        let joined = self.root_directory.join(local);

        // Find the deepest existing ancestor to canonicalize for sandbox check.
        let mut check_path = joined.as_path();
        while !check_path.exists() {
            check_path = check_path.parent().ok_or_else(|| {
                VfsError::InvalidPath("Cannot determine parent directory".into())
            })?;
        }
        let canonical_ancestor = fs::canonicalize(check_path).map_err(|e| {
            VfsError::NotFound(format!("{}: {}", check_path.display(), e))
        })?;
        self.check_sandbox(&canonical_ancestor, path)?;

        if recursive {
            fs::create_dir_all(&joined).map_err(|e| map_io_error(e, path))?;
        } else {
            fs::create_dir(&joined).map_err(|e| map_io_error(e, path))?;
        }

        Ok(())
    }

    /// Read directory entries.
    pub fn readdir(&self, path: &str, recursive: bool) -> Result<Vec<DirEntry>, VfsError> {
        let resolved = self.resolve_path(path)?;

        if !resolved.is_dir() {
            return Err(VfsError::NotADirectory(path.to_string()));
        }

        let mut entries = Vec::new();

        if recursive {
            self.readdir_recursive(&resolved, &resolved, &mut entries)?;
        } else {
            for entry in fs::read_dir(&resolved)? {
                let entry = entry?;
                let meta = entry.metadata()?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let node_type = if meta.is_file() {
                    "file"
                } else if meta.is_dir() {
                    "directory"
                } else {
                    "other"
                };
                entries.push(DirEntry {
                    name,
                    node_type: node_type.to_string(),
                });
            }
        }

        Ok(entries)
    }

    /// Recursively collect directory entries with relative paths.
    ///
    /// Symlinks are skipped to prevent sandbox escapes.
    fn readdir_recursive(
        &self,
        base: &Path,
        current: &Path,
        entries: &mut Vec<DirEntry>,
    ) -> Result<(), VfsError> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_symlink() {
                continue; // Skip symlinks to prevent sandbox escapes
            }

            let rel = entry
                .path()
                .strip_prefix(base)
                .unwrap_or(&entry.path())
                .to_string_lossy()
                .replace('\\', "/");

            let node_type = if ft.is_file() {
                "file"
            } else if ft.is_dir() {
                "directory"
            } else {
                "other"
            };

            entries.push(DirEntry {
                name: rel.clone(),
                node_type: node_type.to_string(),
            });

            if ft.is_dir() {
                self.readdir_recursive(base, &entry.path(), entries)?;
            }
        }
        Ok(())
    }

    /// Remove a directory.
    pub fn rmdir(&self, path: &str, recursive: bool) -> Result<(), VfsError> {
        let resolved = self.resolve_path(path)?;

        if !resolved.is_dir() {
            return Err(VfsError::NotADirectory(path.to_string()));
        }

        if recursive {
            fs::remove_dir_all(&resolved).map_err(|e| map_io_error(e, path))?;
        } else {
            fs::remove_dir(&resolved).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    return VfsError::PermissionDenied(format!("{}: {}", path, e));
                }
                if e.kind() == std::io::ErrorKind::Other
                    || e.to_string().contains("not empty")
                    || e.to_string().contains("directory is not empty")
                    // Windows error
                    || e.to_string().contains("The directory is not empty")
                {
                    VfsError::NotEmpty(path.to_string())
                } else {
                    VfsError::Io(e)
                }
            })?;
        }

        Ok(())
    }

    /// Rename/move a file or directory. Both paths must be in the sandbox.
    pub fn rename(&self, old_path: &str, new_path: &str) -> Result<(), VfsError> {
        let resolved_old = self.resolve_path(old_path)?;
        let resolved_new = self.resolve_parent_path(new_path)?;

        fs::rename(&resolved_old, &resolved_new)?;
        Ok(())
    }

    /// Copy a file or directory.
    pub fn copy(
        &self,
        src: &str,
        dest: &str,
        overwrite: bool,
        recursive: bool,
    ) -> Result<(), VfsError> {
        let resolved_src = self.resolve_path(src)?;
        let resolved_dest = self.resolve_parent_path(dest)?;

        if resolved_src.is_file() {
            if resolved_dest.exists() && !overwrite {
                return Err(VfsError::AlreadyExists(dest.to_string()));
            }
            fs::copy(&resolved_src, &resolved_dest)?;
        } else if resolved_src.is_dir() {
            if !recursive {
                return Err(VfsError::InvalidOperation(
                    "Cannot copy directory without recursive flag".into(),
                ));
            }
            copy_dir_recursive(&resolved_src, &resolved_dest, overwrite)?;
        } else {
            return Err(VfsError::NotFound(src.to_string()));
        }

        Ok(())
    }

    /// Match files against glob patterns. Returns `file://`-prefixed paths.
    pub fn glob(&self, patterns: &[String]) -> Result<Vec<String>, VfsError> {
        let mut matched = Vec::new();

        // Collect all files under root (already canonicalized at construction).
        let mut all_files = Vec::new();
        walk_dir(&self.root_directory, &mut all_files)?;

        for file_path in &all_files {
            // Build a file:// URI from the path relative to root.
            let rel = file_path
                .strip_prefix(&self.root_directory)
                .unwrap_or(file_path);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let file_uri = format!("{}/{}", FILE_SCHEME, rel_str.trim_start_matches('/'));

            for pattern in patterns {
                if match_glob_disk(&file_uri, pattern) {
                    matched.push(file_uri.clone());
                    break;
                }
            }
        }

        matched.sort();
        Ok(matched)
    }

    /// Generate a tree representation of a directory.
    pub fn tree(&self, path: &str) -> Result<String, VfsError> {
        let resolved = self.resolve_path(path)?;

        if !resolved.is_dir() {
            return Err(VfsError::NotADirectory(path.to_string()));
        }

        let name = resolved
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());

        let mut output = String::new();
        output.push_str(&name);
        output.push('\n');
        build_tree(&resolved, "", &mut output)?;
        Ok(output)
    }

    /// Calculate total disk usage of a path in bytes.
    pub fn du(&self, path: &str) -> Result<u64, VfsError> {
        let resolved = self.resolve_path(path)?;
        let total = disk_usage(&resolved)?;
        Ok(total)
    }

    /// Search for text within files under a directory.
    pub fn search(
        &self,
        path: &str,
        query: &str,
        opts: &DiskSearchOptions,
    ) -> Result<DiskSearchResult, VfsError> {
        let resolved = self.resolve_path(path)?;

        let search_opts = SearchOptions {
            max_results: opts.max_results,
            mode: match opts.mode {
                DiskSearchMode::Substring => SearchMode::Substring,
                DiskSearchMode::Regex => SearchMode::Regex,
            },
            context_before: opts.context_before,
            context_after: opts.context_after,
            count_only: opts.count_only,
        };

        let compiled_regex = if matches!(opts.mode, DiskSearchMode::Regex) {
            Some(Regex::new(query).map_err(|e| {
                VfsError::InvalidOperation(format!("Invalid regex: {}", e))
            })?)
        } else {
            None
        };

        // Compile optional glob filter.
        let glob_filter = opts.glob.as_deref();

        let mut results = Vec::new();
        let mut count = 0;

        // Walk files.
        let mut all_files = Vec::new();
        walk_dir(&resolved, &mut all_files)?;

        for file_path in &all_files {
            // Build file:// URI.
            let rel = file_path
                .strip_prefix(&self.root_directory)
                .unwrap_or(file_path);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let file_uri = format!("{}/{}", FILE_SCHEME, rel_str.trim_start_matches('/'));

            // Apply glob filter if present.
            if let Some(glob_pat) = glob_filter {
                if !match_glob_disk(&file_uri, glob_pat) {
                    continue;
                }
            }

            // Read file — skip binary files.
            let data = match fs::read(file_path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            if is_binary(&data) {
                continue;
            }

            let text = match String::from_utf8(data) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let hit_limit = search_text(
                &file_uri,
                &text,
                query,
                &search_opts,
                compiled_regex.as_ref(),
                &mut count,
                &mut results,
            );

            if hit_limit {
                break;
            }
        }

        if opts.count_only {
            Ok(DiskSearchResult::Count(count))
        } else {
            let converted: Vec<SearchResult> = results
                .into_iter()
                .map(|m| SearchResult {
                    path: m.path,
                    line: m.line,
                    column: m.column,
                    match_text: m.match_text,
                    context_before: m.context_before,
                    context_after: m.context_after,
                })
                .collect();
            Ok(DiskSearchResult::Matches(converted))
        }
    }
}

// ── Search types ────────────────────────────────────────────────────────────

/// Search mode for disk operations.
#[derive(Debug, Clone, Copy)]
pub enum DiskSearchMode {
    Substring,
    Regex,
}

/// Options for a disk search operation.
#[derive(Debug, Clone)]
pub struct DiskSearchOptions {
    pub max_results: usize,
    pub mode: DiskSearchMode,
    pub context_before: usize,
    pub context_after: usize,
    pub count_only: bool,
    pub glob: Option<String>,
}

impl Default for DiskSearchOptions {
    fn default() -> Self {
        Self {
            max_results: 100,
            mode: DiskSearchMode::Substring,
            context_before: 0,
            context_after: 0,
            count_only: false,
            glob: None,
        }
    }
}

/// Result of a disk search.
#[derive(Debug)]
pub enum DiskSearchResult {
    Matches(Vec<SearchResult>),
    Count(usize),
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Map OS permission-denied errors to `VfsError::PermissionDenied`.
///
/// All other I/O errors pass through as `VfsError::Io`.
fn map_io_error(e: std::io::Error, path: &str) -> VfsError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        VfsError::PermissionDenied(format!("{}: {}", path, e))
    } else {
        VfsError::Io(e)
    }
}

/// Strip the `file://` scheme prefix and return the local part.
fn strip_file_scheme(path: &str) -> Result<&str, VfsError> {
    path.strip_prefix(FILE_SCHEME).ok_or_else(|| {
        VfsError::InvalidPath(format!("Expected {} prefix: {}", FILE_SCHEME, path))
    })
}

/// Check if data is binary by scanning for null bytes in the first 8 KB.
fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(8192);
    data[..check_len].contains(&0)
}

/// Recursively walk a directory and collect all file paths.
///
/// Symlinks are skipped to prevent sandbox escapes — a symlink inside
/// the root could point outside the sandbox, leaking data.
fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), VfsError> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue; // Skip symlinks to prevent sandbox escapes
        }
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }

    Ok(())
}

/// Recursively copy a directory.
///
/// Symlinks are skipped to prevent following links that point outside
/// the sandbox.
fn copy_dir_recursive(src: &Path, dest: &Path, overwrite: bool) -> Result<(), VfsError> {
    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue; // Skip symlinks to prevent sandbox escapes
        }
        let entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if ft.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path, overwrite)?;
        } else {
            if dest_path.exists() && !overwrite {
                return Err(VfsError::AlreadyExists(
                    dest_path.to_string_lossy().into_owned(),
                ));
            }
            fs::copy(&entry_path, &dest_path)?;
        }
    }

    Ok(())
}

/// Build an indented tree string for a directory.
fn build_tree(dir: &Path, prefix: &str, output: &mut String) -> Result<(), VfsError> {
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();

    // Sort entries alphabetically.
    entries.sort_by_key(|e| e.file_name());

    let total = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == total - 1;
        let connector = if is_last { "\u{2514}\u{2500}\u{2500} " } else { "\u{251c}\u{2500}\u{2500} " };
        let child_prefix = if is_last { "    " } else { "\u{2502}   " };

        let name = entry.file_name().to_string_lossy().into_owned();
        output.push_str(prefix);
        output.push_str(connector);
        output.push_str(&name);
        output.push('\n');

        if entry.path().is_dir() {
            let new_prefix = format!("{}{}", prefix, child_prefix);
            build_tree(&entry.path(), &new_prefix, output)?;
        }
    }

    Ok(())
}

/// Recursively calculate disk usage in bytes.
fn disk_usage(path: &Path) -> Result<u64, VfsError> {
    if path.is_file() {
        let meta = fs::metadata(path)?;
        return Ok(meta.len());
    }

    if !path.is_dir() {
        return Ok(0);
    }

    let mut total: u64 = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        total += disk_usage(&entry_path)?;
    }

    Ok(total)
}

/// Match a `file://`-prefixed path against a glob pattern.
///
/// The pattern may or may not have the `file://` prefix. Both the path
/// and pattern are stripped of the scheme before matching with `match_parts`.
fn match_glob_disk(file_path: &str, pattern: &str) -> bool {
    let expanded = expand_braces(pattern);

    let local = file_path
        .strip_prefix(FILE_SCHEME)
        .unwrap_or(file_path);
    let path_parts: Vec<&str> = local.split('/').filter(|s| !s.is_empty()).collect();

    for exp in &expanded {
        let pat_local = exp
            .strip_prefix(FILE_SCHEME)
            .unwrap_or(exp);
        // Also strip a bare leading slash if the pattern has no scheme.
        let pat_local = pat_local.strip_prefix('/').unwrap_or(pat_local);
        let pattern_parts: Vec<&str> = pat_local.split('/').filter(|s| !s.is_empty()).collect();

        if match_parts(&path_parts, 0, &pattern_parts, 0) {
            return true;
        }
    }

    false
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a temp dir and return a DiskFs rooted there.
    fn temp_disk() -> (tempfile::TempDir, DiskFs) {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let disk = DiskFs::new(tmp.path().to_path_buf(), vec![]);
        (tmp, disk)
    }

    fn file_uri(rel: &str) -> String {
        format!("file:///{}", rel.trim_start_matches('/'))
    }

    #[test]
    fn is_binary_detection() {
        assert!(!is_binary(b"hello world"));
        assert!(is_binary(b"hello\0world"));
        assert!(!is_binary(b""));
    }

    #[test]
    fn match_glob_disk_basic() {
        assert!(match_glob_disk("file:///src/main.rs", "**/*.rs"));
        assert!(match_glob_disk("file:///src/main.rs", "file:///src/*.rs"));
        assert!(!match_glob_disk("file:///src/main.rs", "file:///lib/*.rs"));
    }

    #[test]
    fn match_glob_disk_braces() {
        assert!(match_glob_disk(
            "file:///src/main.ts",
            "**/*.{ts,js}"
        ));
        assert!(match_glob_disk(
            "file:///src/main.js",
            "**/*.{ts,js}"
        ));
        assert!(!match_glob_disk(
            "file:///src/main.rs",
            "**/*.{ts,js}"
        ));
    }

    #[test]
    fn write_and_read_text_file() {
        let (_tmp, disk) = temp_disk();
        let path = file_uri("hello.txt");

        disk.write_file(&path, "Hello, world!", None, false)
            .unwrap();

        let result = disk.read_file(&path).unwrap();
        assert_eq!(result.content_type, "text");
        assert_eq!(result.text.as_deref(), Some("Hello, world!"));
        assert!(result.data.is_none());
        assert_eq!(result.size, 13);
    }

    #[test]
    fn write_and_read_binary_file() {
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

    #[test]
    fn write_with_create_parents() {
        let (_tmp, disk) = temp_disk();
        let path = file_uri("a/b/c/deep.txt");

        disk.write_file(&path, "deep content", None, true).unwrap();

        let result = disk.read_file(&path).unwrap();
        assert_eq!(result.text.as_deref(), Some("deep content"));
    }

    #[test]
    fn append_file_works() {
        let (_tmp, disk) = temp_disk();
        let path = file_uri("log.txt");

        disk.write_file(&path, "line1\n", None, false).unwrap();
        disk.append_file(&path, "line2\n").unwrap();

        let result = disk.read_file(&path).unwrap();
        assert_eq!(result.text.as_deref(), Some("line1\nline2\n"));
    }

    #[test]
    fn delete_file_works() {
        let (_tmp, disk) = temp_disk();
        let path = file_uri("temp.txt");

        disk.write_file(&path, "temp", None, false).unwrap();
        assert!(disk.exists(&path).unwrap());

        disk.delete_file(&path).unwrap();
        assert!(!disk.exists(&path).unwrap());
    }

    #[test]
    fn stat_file() {
        let (_tmp, disk) = temp_disk();
        let path = file_uri("info.txt");
        disk.write_file(&path, "data", None, false).unwrap();

        let stat = disk.stat(&path).unwrap();
        assert_eq!(stat.node_type, "file");
        assert_eq!(stat.size, 4);
        assert!(stat.modified_at > 0);
    }

    #[test]
    fn stat_directory() {
        let (_tmp, disk) = temp_disk();
        let path = file_uri("mydir");
        disk.mkdir(&path, false).unwrap();

        let stat = disk.stat(&path).unwrap();
        assert_eq!(stat.node_type, "directory");
    }

    #[test]
    fn exists_returns_false_for_missing() {
        let (_tmp, disk) = temp_disk();
        assert!(!disk.exists(&file_uri("nope.txt")).unwrap());
    }

    #[test]
    fn mkdir_and_readdir() {
        let (_tmp, disk) = temp_disk();
        let dir = file_uri("stuff");
        disk.mkdir(&dir, false).unwrap();

        let f1 = file_uri("stuff/a.txt");
        let f2 = file_uri("stuff/b.txt");
        disk.write_file(&f1, "a", None, false).unwrap();
        disk.write_file(&f2, "b", None, false).unwrap();

        let entries = disk.readdir(&dir, false).unwrap();
        assert_eq!(entries.len(), 2);

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
    }

    #[test]
    fn readdir_recursive() {
        let (_tmp, disk) = temp_disk();
        disk.write_file(&file_uri("r/a.txt"), "a", None, true)
            .unwrap();
        disk.write_file(&file_uri("r/sub/b.txt"), "b", None, true)
            .unwrap();

        let entries = disk.readdir(&file_uri("r"), true).unwrap();
        // Should have: a.txt, sub, sub/b.txt
        assert!(entries.len() >= 3);
    }

    #[test]
    fn rmdir_non_recursive_empty() {
        let (_tmp, disk) = temp_disk();
        let dir = file_uri("empty_dir");
        disk.mkdir(&dir, false).unwrap();
        disk.rmdir(&dir, false).unwrap();
        assert!(!disk.exists(&dir).unwrap());
    }

    #[test]
    fn rmdir_recursive() {
        let (_tmp, disk) = temp_disk();
        disk.write_file(&file_uri("rm_me/sub/file.txt"), "x", None, true)
            .unwrap();
        disk.rmdir(&file_uri("rm_me"), true).unwrap();
        assert!(!disk.exists(&file_uri("rm_me")).unwrap());
    }

    #[test]
    fn rename_file() {
        let (_tmp, disk) = temp_disk();
        let old = file_uri("old.txt");
        let new = file_uri("new.txt");

        disk.write_file(&old, "content", None, false).unwrap();
        disk.rename(&old, &new).unwrap();

        assert!(!disk.exists(&old).unwrap());
        let result = disk.read_file(&new).unwrap();
        assert_eq!(result.text.as_deref(), Some("content"));
    }

    #[test]
    fn copy_file() {
        let (_tmp, disk) = temp_disk();
        let src = file_uri("src.txt");
        let dest = file_uri("dest.txt");

        disk.write_file(&src, "copy me", None, false).unwrap();
        disk.copy(&src, &dest, false, false).unwrap();

        let result = disk.read_file(&dest).unwrap();
        assert_eq!(result.text.as_deref(), Some("copy me"));
        // Source still exists.
        assert!(disk.exists(&src).unwrap());
    }

    #[test]
    fn copy_no_overwrite() {
        let (_tmp, disk) = temp_disk();
        let src = file_uri("s.txt");
        let dest = file_uri("d.txt");

        disk.write_file(&src, "a", None, false).unwrap();
        disk.write_file(&dest, "b", None, false).unwrap();

        let err = disk.copy(&src, &dest, false, false).unwrap_err();
        assert!(matches!(err, VfsError::AlreadyExists(_)));
    }

    #[test]
    fn glob_finds_files() {
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

    #[test]
    fn tree_output() {
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

    #[test]
    fn du_calculates_size() {
        let (_tmp, disk) = temp_disk();
        disk.write_file(&file_uri("du/a.txt"), "aaaa", None, true)
            .unwrap();
        disk.write_file(&file_uri("du/b.txt"), "bb", None, true)
            .unwrap();

        let total = disk.du(&file_uri("du")).unwrap();
        assert_eq!(total, 6); // 4 + 2
    }

    #[test]
    fn search_finds_text() {
        let (_tmp, disk) = temp_disk();
        disk.write_file(&file_uri("s/hello.txt"), "hello world\nfoo bar\nhello again", None, true)
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
            DiskSearchResult::Count(_) => panic!("Expected matches, got count"),
        }
    }

    #[test]
    fn search_count_only() {
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
            DiskSearchResult::Matches(_) => panic!("Expected count, got matches"),
        }
    }

    #[test]
    fn sandbox_prevents_traversal() {
        let (_tmp, disk) = temp_disk();
        // Attempting to traverse outside root should fail.
        let result = disk.resolve_path("file:///../../../etc/passwd");
        assert!(result.is_err());
    }
}
