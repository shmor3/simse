use crate::sandbox::error::SandboxError;
use crate::sandbox::ssh::fs::SshFs;
use crate::sandbox::vfs_diff::DiffOutput;
use crate::sandbox::vfs_disk::{DiskFs, DiskSearchOptions, DiskSearchResult};
use crate::sandbox::vfs_types::{DirEntry, HistoryEntry, ReadFileResult, StatResult};

/// Unified filesystem backend — dispatches to local disk or SSH.
pub enum FsImpl {
    Local(DiskFs),
    Ssh(SshFs),
}

impl FsImpl {
    pub async fn read_file(&self, path: &str) -> Result<ReadFileResult, SandboxError> {
        match self {
            Self::Local(fs) => fs.read_file(path),
            Self::Ssh(fs) => fs.read_file(path).await,
        }
    }

    pub async fn write_file(
        &self,
        path: &str,
        content: &str,
        content_type: Option<&str>,
        create_parents: bool,
    ) -> Result<(), SandboxError> {
        match self {
            Self::Local(fs) => fs.write_file(path, content, content_type, create_parents),
            Self::Ssh(fs) => fs.write_file(path, content, content_type, create_parents).await,
        }
    }

    pub async fn append_file(&self, path: &str, content: &str) -> Result<(), SandboxError> {
        match self {
            Self::Local(fs) => fs.append_file(path, content),
            Self::Ssh(fs) => fs.append_file(path, content).await,
        }
    }

    pub async fn delete_file(&self, path: &str) -> Result<(), SandboxError> {
        match self {
            Self::Local(fs) => fs.delete_file(path),
            Self::Ssh(fs) => fs.delete_file(path).await,
        }
    }

    pub async fn mkdir(&self, path: &str, recursive: bool) -> Result<(), SandboxError> {
        match self {
            Self::Local(fs) => fs.mkdir(path, recursive),
            Self::Ssh(fs) => fs.mkdir(path, recursive).await,
        }
    }

    pub async fn readdir(
        &self,
        path: &str,
        recursive: bool,
    ) -> Result<Vec<DirEntry>, SandboxError> {
        match self {
            Self::Local(fs) => fs.readdir(path, recursive),
            Self::Ssh(fs) => fs.readdir(path, recursive).await,
        }
    }

    pub async fn rmdir(&self, path: &str, recursive: bool) -> Result<(), SandboxError> {
        match self {
            Self::Local(fs) => fs.rmdir(path, recursive),
            Self::Ssh(fs) => fs.rmdir(path, recursive).await,
        }
    }

    pub async fn stat(&self, path: &str) -> Result<StatResult, SandboxError> {
        match self {
            Self::Local(fs) => fs.stat(path),
            Self::Ssh(fs) => fs.stat(path).await,
        }
    }

    pub async fn exists(&self, path: &str) -> Result<bool, SandboxError> {
        match self {
            Self::Local(fs) => fs.exists(path),
            Self::Ssh(fs) => fs.exists(path).await,
        }
    }

    pub async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), SandboxError> {
        match self {
            Self::Local(fs) => fs.rename(old_path, new_path),
            Self::Ssh(fs) => fs.rename(old_path, new_path).await,
        }
    }

    pub async fn copy(
        &self,
        src: &str,
        dest: &str,
        overwrite: bool,
        recursive: bool,
    ) -> Result<(), SandboxError> {
        match self {
            Self::Local(fs) => fs.copy(src, dest, overwrite, recursive),
            Self::Ssh(fs) => fs.copy(src, dest, overwrite, recursive).await,
        }
    }

    pub async fn glob(&self, patterns: &[String]) -> Result<Vec<String>, SandboxError> {
        match self {
            Self::Local(fs) => fs.glob(patterns),
            Self::Ssh(fs) => fs.glob(patterns).await,
        }
    }

    pub async fn tree(&self, path: &str) -> Result<String, SandboxError> {
        match self {
            Self::Local(fs) => fs.tree(path),
            Self::Ssh(fs) => fs.tree(path).await,
        }
    }

    pub async fn du(&self, path: &str) -> Result<u64, SandboxError> {
        match self {
            Self::Local(fs) => fs.du(path),
            Self::Ssh(fs) => fs.du(path).await,
        }
    }

    pub async fn search(
        &self,
        path: &str,
        query: &str,
        opts: &DiskSearchOptions,
    ) -> Result<DiskSearchResult, SandboxError> {
        match self {
            Self::Local(fs) => fs.search(path, query, opts),
            Self::Ssh(fs) => fs.search(path, query, opts).await,
        }
    }

    pub async fn history(&self, path: &str) -> Result<Vec<HistoryEntry>, SandboxError> {
        match self {
            Self::Local(fs) => fs.history(path),
            Self::Ssh(fs) => fs.history(path).await,
        }
    }

    pub async fn diff(
        &self,
        old_path: &str,
        new_path: &str,
        context: usize,
    ) -> Result<DiffOutput, SandboxError> {
        match self {
            Self::Local(fs) => fs.diff(old_path, new_path, context),
            Self::Ssh(fs) => fs.diff(old_path, new_path, context).await,
        }
    }

    pub async fn diff_versions(
        &self,
        path: &str,
        old_version: usize,
        new_version: Option<usize>,
        context: usize,
    ) -> Result<DiffOutput, SandboxError> {
        match self {
            Self::Local(fs) => fs.diff_versions(path, old_version, new_version, context),
            Self::Ssh(fs) => fs.diff_versions(path, old_version, new_version, context).await,
        }
    }

    pub async fn checkout(&self, path: &str, version: usize) -> Result<(), SandboxError> {
        match self {
            Self::Local(fs) => fs.checkout(path, version),
            Self::Ssh(fs) => fs.checkout(path, version).await,
        }
    }
}
