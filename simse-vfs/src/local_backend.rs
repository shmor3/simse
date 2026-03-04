use async_trait::async_trait;

use crate::backend::FsBackend;
use crate::diff::DiffOutput;
use crate::disk::{DiskFs, DiskSearchOptions, DiskSearchResult};
use crate::error::VfsError;
use crate::protocol::{DirEntry, HistoryEntry, ReadFileResult, StatResult};

// ── LocalFsBackend ──────────────────────────────────────────────────────────

/// Local filesystem backend wrapping `DiskFs`.
///
/// Each async method simply delegates to the underlying synchronous `DiskFs`
/// implementation. This is the default backend used when no remote (SSH)
/// backend is configured.
pub struct LocalFsBackend {
    disk: DiskFs,
}

impl LocalFsBackend {
    /// Create a new `LocalFsBackend` wrapping the given `DiskFs`.
    pub fn new(disk: DiskFs) -> Self {
        Self { disk }
    }

    /// Access the underlying `DiskFs`.
    pub fn disk(&self) -> &DiskFs {
        &self.disk
    }
}

#[async_trait]
impl FsBackend for LocalFsBackend {
    async fn read_file(&self, path: &str) -> Result<ReadFileResult, VfsError> {
        self.disk.read_file(path)
    }

    async fn write_file(
        &self,
        path: &str,
        content: &str,
        content_type: Option<&str>,
        create_parents: bool,
    ) -> Result<(), VfsError> {
        self.disk.write_file(path, content, content_type, create_parents)
    }

    async fn append_file(&self, path: &str, content: &str) -> Result<(), VfsError> {
        self.disk.append_file(path, content)
    }

    async fn delete_file(&self, path: &str) -> Result<(), VfsError> {
        self.disk.delete_file(path)
    }

    async fn mkdir(&self, path: &str, recursive: bool) -> Result<(), VfsError> {
        self.disk.mkdir(path, recursive)
    }

    async fn readdir(&self, path: &str, recursive: bool) -> Result<Vec<DirEntry>, VfsError> {
        self.disk.readdir(path, recursive)
    }

    async fn rmdir(&self, path: &str, recursive: bool) -> Result<(), VfsError> {
        self.disk.rmdir(path, recursive)
    }

    async fn stat(&self, path: &str) -> Result<StatResult, VfsError> {
        self.disk.stat(path)
    }

    async fn exists(&self, path: &str) -> Result<bool, VfsError> {
        self.disk.exists(path)
    }

    async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), VfsError> {
        self.disk.rename(old_path, new_path)
    }

    async fn copy(
        &self,
        src: &str,
        dest: &str,
        overwrite: bool,
        recursive: bool,
    ) -> Result<(), VfsError> {
        self.disk.copy(src, dest, overwrite, recursive)
    }

    async fn glob(&self, patterns: &[String]) -> Result<Vec<String>, VfsError> {
        self.disk.glob(patterns)
    }

    async fn tree(&self, path: &str) -> Result<String, VfsError> {
        self.disk.tree(path)
    }

    async fn du(&self, path: &str) -> Result<u64, VfsError> {
        self.disk.du(path)
    }

    async fn search(
        &self,
        path: &str,
        query: &str,
        opts: &DiskSearchOptions,
    ) -> Result<DiskSearchResult, VfsError> {
        self.disk.search(path, query, opts)
    }

    async fn history(&self, path: &str) -> Result<Vec<HistoryEntry>, VfsError> {
        self.disk.history(path)
    }

    async fn diff(
        &self,
        old_path: &str,
        new_path: &str,
        context: usize,
    ) -> Result<DiffOutput, VfsError> {
        self.disk.diff(old_path, new_path, context)
    }

    async fn diff_versions(
        &self,
        path: &str,
        old_version: usize,
        new_version: Option<usize>,
        context: usize,
    ) -> Result<DiffOutput, VfsError> {
        self.disk.diff_versions(path, old_version, new_version, context)
    }

    async fn checkout(&self, path: &str, version: usize) -> Result<(), VfsError> {
        self.disk.checkout(path, version)
    }
}
