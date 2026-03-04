use async_trait::async_trait;

use crate::diff::DiffOutput;
use crate::disk::{DiskSearchOptions, DiskSearchResult};
use crate::error::VfsError;
use crate::protocol::{DirEntry, HistoryEntry, ReadFileResult, StatResult};

// ── FsBackend trait ─────────────────────────────────────────────────────────

/// Async filesystem backend trait.
///
/// Mirrors all public methods of `DiskFs` so that operations can be routed
/// through either a local or remote (e.g. SSH) backend.
#[async_trait]
pub trait FsBackend: Send + Sync {
    // ── File operations ─────────────────────────────────────────────────

    /// Read a file. Returns text or base64 depending on content.
    async fn read_file(&self, path: &str) -> Result<ReadFileResult, VfsError>;

    /// Write content to a file.
    async fn write_file(
        &self,
        path: &str,
        content: &str,
        content_type: Option<&str>,
        create_parents: bool,
    ) -> Result<(), VfsError>;

    /// Append text content to an existing file.
    async fn append_file(&self, path: &str, content: &str) -> Result<(), VfsError>;

    /// Delete a file.
    async fn delete_file(&self, path: &str) -> Result<(), VfsError>;

    // ── Directory operations ────────────────────────────────────────────

    /// Create a directory.
    async fn mkdir(&self, path: &str, recursive: bool) -> Result<(), VfsError>;

    /// Read directory entries.
    async fn readdir(&self, path: &str, recursive: bool) -> Result<Vec<DirEntry>, VfsError>;

    /// Remove a directory.
    async fn rmdir(&self, path: &str, recursive: bool) -> Result<(), VfsError>;

    // ── Metadata ────────────────────────────────────────────────────────

    /// Get metadata for a file or directory.
    async fn stat(&self, path: &str) -> Result<StatResult, VfsError>;

    /// Check whether a path exists.
    async fn exists(&self, path: &str) -> Result<bool, VfsError>;

    // ── File management ─────────────────────────────────────────────────

    /// Rename/move a file or directory.
    async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), VfsError>;

    /// Copy a file or directory.
    async fn copy(
        &self,
        src: &str,
        dest: &str,
        overwrite: bool,
        recursive: bool,
    ) -> Result<(), VfsError>;

    // ── Search & traversal ──────────────────────────────────────────────

    /// Match files against glob patterns. Returns `file://`-prefixed paths.
    async fn glob(&self, patterns: &[String]) -> Result<Vec<String>, VfsError>;

    /// Generate a tree representation of a directory.
    async fn tree(&self, path: &str) -> Result<String, VfsError>;

    /// Calculate total disk usage of a path in bytes.
    async fn du(&self, path: &str) -> Result<u64, VfsError>;

    /// Search for text within files under a directory.
    async fn search(
        &self,
        path: &str,
        query: &str,
        opts: &DiskSearchOptions,
    ) -> Result<DiskSearchResult, VfsError>;

    // ── History & versioning ────────────────────────────────────────────

    /// Get version history for a file.
    async fn history(&self, path: &str) -> Result<Vec<HistoryEntry>, VfsError>;

    /// Diff two files. Returns raw diff output.
    async fn diff(
        &self,
        old_path: &str,
        new_path: &str,
        context: usize,
    ) -> Result<DiffOutput, VfsError>;

    /// Diff two versions of the same file.
    async fn diff_versions(
        &self,
        path: &str,
        old_version: usize,
        new_version: Option<usize>,
        context: usize,
    ) -> Result<DiffOutput, VfsError>;

    /// Checkout (revert) a file to a specific version.
    async fn checkout(&self, path: &str, version: usize) -> Result<(), VfsError>;
}
