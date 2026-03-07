use std::sync::Arc;

use base64::Engine;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::StatusCode;

use crate::sandbox::error::SandboxError;
use crate::sandbox::vfs_diff::DiffOutput;
use crate::sandbox::vfs_disk::{DiskSearchMode, DiskSearchOptions, DiskSearchResult};
use crate::sandbox::vfs_types::{
    DirEntry, HistoryEntry, ReadFileResult, SearchResult, StatResult,
};

use super::channel::read_channel_output;
use super::pool::SshPool;

// ── Constants ───────────────────────────────────────────────────────────────

/// Default timeout for exec commands (30 seconds).
const EXEC_TIMEOUT_MS: u64 = 30_000;

/// Maximum output bytes from exec commands (10 MB).
const EXEC_MAX_BYTES: usize = 10 * 1024 * 1024;

// ── SshFs ───────────────────────────────────────────────────────────────────

/// Remote filesystem backend operating over SSH.
///
/// SFTP is used for file I/O operations (read, write, mkdir, stat, etc.)
/// and exec channels are used for operations that SFTP cannot handle
/// (glob, search, tree, du). All paths are scoped under `root` on the
/// remote machine.
pub struct SshFs {
    pool: Arc<SshPool>,
    root: String,
}

impl SshFs {
    /// Create a new `SshFs` with the given pool and remote root directory.
    pub fn new(pool: Arc<SshPool>, root: String) -> Self {
        // Ensure root ends with '/' for clean path joining
        let root = if root.ends_with('/') {
            root
        } else {
            format!("{root}/")
        };
        Self { pool, root }
    }

    /// Resolve a path relative to the remote root directory.
    ///
    /// Strips leading slashes from the input and prepends the root.
    fn resolve(&self, path: &str) -> String {
        let clean = path.trim_start_matches('/');
        if clean.is_empty() {
            // Root directory itself (remove trailing slash for SFTP ops)
            self.root.trim_end_matches('/').to_string()
        } else {
            format!("{}{}", self.root, clean)
        }
    }

    /// Get an SFTP session from the pool, mapping errors to SandboxError.
    async fn sftp(&self) -> Result<SftpSession, SandboxError> {
        self.pool
            .get_sftp_session()
            .await
            .map_err(|e| SandboxError::VfsIo(e.to_string()))
    }

    /// Execute a remote command and return stdout.
    async fn run_remote_cmd(&self, command: &str) -> Result<String, SandboxError> {
        let mut channel = self.pool.get_exec_channel().await.map_err(|e| {
            SandboxError::VfsIo(e.to_string())
        })?;

        channel.exec(true, command.as_bytes()).await.map_err(|e| {
            SandboxError::VfsIo(e.to_string())
        })?;

        let output = read_channel_output(&mut channel, EXEC_TIMEOUT_MS, EXEC_MAX_BYTES)
            .await
            .map_err(|e| {
                SandboxError::VfsIo(e.to_string())
            })?;

        if output.exit_code.unwrap_or(1) != 0 {
            let stderr = output.stderr.trim().to_string();
            if !stderr.is_empty() {
                return Err(SandboxError::VfsIo(stderr));
            }
        }

        Ok(output.stdout)
    }

    /// Create parent directories for a given remote path via SFTP.
    async fn ensure_parent_dirs(&self, sftp: &SftpSession, remote_path: &str) -> Result<(), SandboxError> {
        if let Some(parent) = remote_path.rsplit_once('/').map(|(p, _)| p) {
            if !parent.is_empty() {
                self.mkdir_recursive(sftp, parent).await?;
            }
        }
        Ok(())
    }

    /// Recursively create directories via SFTP.
    ///
    /// Uses `Box::pin` to allow async recursion.
    fn mkdir_recursive<'a>(
        &'a self,
        sftp: &'a SftpSession,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), SandboxError>> + Send + 'a>> {
        Box::pin(async move {
            // Check if directory already exists
            match sftp.metadata(path.to_string()).await {
                Ok(attrs) if attrs.is_dir() => return Ok(()),
                _ => {}
            }

            // Create parent first
            if let Some(parent) = path.rsplit_once('/').map(|(p, _)| p) {
                if !parent.is_empty() {
                    self.mkdir_recursive(sftp, parent).await?;
                }
            }

            // Create this directory, ignoring "already exists"
            match sftp.create_dir(path.to_string()).await {
                Ok(()) => Ok(()),
                Err(russh_sftp::client::error::Error::Status(status))
                    if status.status_code == StatusCode::Failure =>
                {
                    // May already exist after recursive creation
                    match sftp.metadata(path.to_string()).await {
                        Ok(attrs) if attrs.is_dir() => Ok(()),
                        _ => Err(SandboxError::VfsIo(
                            format!("failed to create directory: {path}"),
                        )),
                    }
                }
                Err(e) => Err(sftp_err(e)),
            }
        })
    }

    /// Recursively remove a directory and all its contents via SFTP.
    ///
    /// Uses `Box::pin` to allow async recursion.
    fn rmdir_recursive<'a>(
        &'a self,
        sftp: &'a SftpSession,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), SandboxError>> + Send + 'a>> {
        Box::pin(async move {
            let entries = sftp.read_dir(path.to_string()).await.map_err(sftp_err)?;

            for entry in entries {
                let name = entry.file_name();
                let child = format!("{path}/{name}");

                if entry.file_type().is_dir() {
                    self.rmdir_recursive(sftp, &child).await?;
                } else {
                    sftp.remove_file(child.clone()).await.map_err(sftp_err)?;
                }
            }

            sftp.remove_dir(path.to_string()).await.map_err(sftp_err)
        })
    }

    /// Recursively list directory entries via SFTP.
    ///
    /// Uses `Box::pin` to allow async recursion.
    fn readdir_recursive<'a>(
        &'a self,
        sftp: &'a SftpSession,
        base: &'a str,
        prefix: &'a str,
        out: &'a mut Vec<DirEntry>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), SandboxError>> + Send + 'a>> {
        Box::pin(async move {
            let entries = sftp.read_dir(base.to_string()).await.map_err(sftp_err)?;

            for entry in entries {
                let name = entry.file_name();
                let full_name = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}/{name}")
                };
                let is_dir = entry.file_type().is_dir();
                let node_type = if is_dir { "directory" } else { "file" };

                out.push(DirEntry {
                    name: full_name.clone(),
                    node_type: node_type.to_string(),
                });

                if is_dir {
                    let child_path = format!("{base}/{name}");
                    self.readdir_recursive(sftp, &child_path, &full_name, out)
                        .await?;
                }
            }

            Ok(())
        })
    }

    // ── File operations ─────────────────────────────────────────────────

    pub async fn read_file(&self, path: &str) -> Result<ReadFileResult, SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;

        let data = sftp.read(remote).await.map_err(sftp_err)?;
        let size = data.len() as u64;
        let (content_type, is_text) = detect_content_type(&data);

        if is_text {
            Ok(ReadFileResult {
                content_type: content_type.to_string(),
                text: Some(String::from_utf8_lossy(&data).into_owned()),
                data: None,
                size,
            })
        } else {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            Ok(ReadFileResult {
                content_type: content_type.to_string(),
                text: None,
                data: Some(encoded),
                size,
            })
        }
    }

    pub async fn write_file(
        &self,
        path: &str,
        content: &str,
        _content_type: Option<&str>,
        create_parents: bool,
    ) -> Result<(), SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;

        if create_parents {
            self.ensure_parent_dirs(&sftp, &remote).await?;
        }

        // Create (truncate) and write
        let mut file = sftp
            .create(remote)
            .await
            .map_err(sftp_err)?;

        use tokio::io::AsyncWriteExt;
        file.write_all(content.as_bytes()).await.map_err(|e| {
            SandboxError::VfsIo(e.to_string())
        })?;

        file.shutdown().await.map_err(|e| {
            SandboxError::VfsIo(e.to_string())
        })?;

        Ok(())
    }

    pub async fn append_file(&self, path: &str, content: &str) -> Result<(), SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;

        // Read existing content, append, and write back
        let existing = match sftp.read(remote.clone()).await {
            Ok(data) => data,
            Err(russh_sftp::client::error::Error::Status(status))
                if status.status_code == StatusCode::NoSuchFile =>
            {
                Vec::new()
            }
            Err(e) => return Err(sftp_err(e)),
        };

        let mut combined = existing;
        combined.extend_from_slice(content.as_bytes());

        let mut file = sftp.create(remote).await.map_err(sftp_err)?;

        use tokio::io::AsyncWriteExt;
        file.write_all(&combined).await.map_err(|e| {
            SandboxError::VfsIo(e.to_string())
        })?;

        file.shutdown().await.map_err(|e| {
            SandboxError::VfsIo(e.to_string())
        })?;

        Ok(())
    }

    pub async fn delete_file(&self, path: &str) -> Result<(), SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;
        sftp.remove_file(remote).await.map_err(sftp_err)
    }

    // ── Directory operations ────────────────────────────────────────────

    pub async fn mkdir(&self, path: &str, recursive: bool) -> Result<(), SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;

        if recursive {
            self.mkdir_recursive(&sftp, &remote).await
        } else {
            sftp.create_dir(remote).await.map_err(sftp_err)
        }
    }

    pub async fn readdir(&self, path: &str, recursive: bool) -> Result<Vec<DirEntry>, SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;

        if recursive {
            let mut entries = Vec::new();
            self.readdir_recursive(&sftp, &remote, "", &mut entries)
                .await?;
            Ok(entries)
        } else {
            let read_dir = sftp.read_dir(remote).await.map_err(sftp_err)?;
            let entries = read_dir
                .map(|entry| {
                    let name = entry.file_name();
                    let node_type = if entry.file_type().is_dir() {
                        "directory"
                    } else {
                        "file"
                    };
                    DirEntry {
                        name,
                        node_type: node_type.to_string(),
                    }
                })
                .collect();
            Ok(entries)
        }
    }

    pub async fn rmdir(&self, path: &str, recursive: bool) -> Result<(), SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;

        if recursive {
            self.rmdir_recursive(&sftp, &remote).await
        } else {
            sftp.remove_dir(remote).await.map_err(sftp_err)
        }
    }

    // ── Metadata ────────────────────────────────────────────────────────

    pub async fn stat(&self, path: &str) -> Result<StatResult, SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;

        let attrs = sftp.metadata(remote.clone()).await.map_err(sftp_err)?;

        let node_type = if attrs.is_dir() {
            "directory"
        } else {
            "file"
        };

        Ok(StatResult {
            path: path.to_string(),
            node_type: node_type.to_string(),
            size: attrs.size.unwrap_or(0),
            created_at: attrs.atime.unwrap_or(0) as u64,
            modified_at: attrs.mtime.unwrap_or(0) as u64,
        })
    }

    pub async fn exists(&self, path: &str) -> Result<bool, SandboxError> {
        let remote = self.resolve(path);
        let sftp = self.sftp().await?;

        match sftp.try_exists(remote).await {
            Ok(exists) => Ok(exists),
            Err(e) => Err(sftp_err(e)),
        }
    }

    // ── File management ─────────────────────────────────────────────────

    pub async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), SandboxError> {
        let old_remote = self.resolve(old_path);
        let new_remote = self.resolve(new_path);
        let sftp = self.sftp().await?;
        sftp.rename(old_remote, new_remote).await.map_err(sftp_err)
    }

    pub async fn copy(
        &self,
        src: &str,
        dest: &str,
        _overwrite: bool,
        _recursive: bool,
    ) -> Result<(), SandboxError> {
        let src_remote = self.resolve(src);
        let dest_remote = self.resolve(dest);
        let sftp = self.sftp().await?;

        // SFTP has no native copy -- read source, write to dest
        let data = sftp.read(src_remote).await.map_err(sftp_err)?;

        let mut file = sftp.create(dest_remote).await.map_err(sftp_err)?;

        use tokio::io::AsyncWriteExt;
        file.write_all(&data).await.map_err(|e| {
            SandboxError::VfsIo(e.to_string())
        })?;

        file.shutdown().await.map_err(|e| {
            SandboxError::VfsIo(e.to_string())
        })?;

        Ok(())
    }

    // ── Search & traversal (exec-based) ─────────────────────────────────

    pub async fn glob(&self, patterns: &[String]) -> Result<Vec<String>, SandboxError> {
        if patterns.is_empty() {
            return Ok(Vec::new());
        }

        // Build a find command with -name patterns joined by -o
        let root = shell_escape(self.root.trim_end_matches('/'));
        let mut name_args = Vec::new();
        for pattern in patterns {
            if !name_args.is_empty() {
                name_args.push("-o".to_string());
            }
            name_args.push("-name".to_string());
            name_args.push(shell_escape(pattern));
        }

        let cmd = format!(
            "find {} -type f \\( {} \\) 2>/dev/null",
            root,
            name_args.join(" ")
        );

        let stdout = self.run_remote_cmd(&cmd).await?;

        let results = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        Ok(results)
    }

    pub async fn tree(&self, path: &str) -> Result<String, SandboxError> {
        let remote = self.resolve(path);
        let escaped = shell_escape(&remote);

        // Try `tree` first, fall back to `find`
        let cmd = format!(
            "if command -v tree >/dev/null 2>&1; then tree -a {escaped}; else find {escaped} | sort; fi"
        );

        self.run_remote_cmd(&cmd).await
    }

    pub async fn du(&self, path: &str) -> Result<u64, SandboxError> {
        let remote = self.resolve(path);
        let escaped = shell_escape(&remote);

        let cmd = format!("du -sb {escaped} 2>/dev/null | cut -f1");
        let stdout = self.run_remote_cmd(&cmd).await?;

        let size_str = stdout.trim();
        size_str.parse::<u64>().map_err(|e| {
            SandboxError::VfsIo(
                format!("failed to parse du output '{size_str}': {e}"),
            )
        })
    }

    pub async fn search(
        &self,
        path: &str,
        query: &str,
        opts: &DiskSearchOptions,
    ) -> Result<DiskSearchResult, SandboxError> {
        let remote = self.resolve(path);
        let escaped_path = shell_escape(&remote);
        let escaped_query = shell_escape(query);

        // Build grep flags
        let mut grep_flags = vec!["-rn".to_string()];

        // Add context flags
        if opts.context_before > 0 {
            grep_flags.push(format!("-B{}", opts.context_before));
        }
        if opts.context_after > 0 {
            grep_flags.push(format!("-A{}", opts.context_after));
        }

        // Search mode
        match opts.mode {
            DiskSearchMode::Regex => {
                grep_flags.push("-E".to_string());
            }
            DiskSearchMode::Substring => {
                grep_flags.push("-F".to_string());
            }
        }

        // Max results
        grep_flags.push(format!("-m{}", opts.max_results));

        // Glob filter
        let include = if let Some(ref glob) = opts.glob {
            format!(" --include={}", shell_escape(glob))
        } else {
            String::new()
        };

        let cmd = format!(
            "grep {} {escaped_query} {escaped_path}{include} 2>/dev/null || true",
            grep_flags.join(" "),
        );

        let stdout = self.run_remote_cmd(&cmd).await?;

        if opts.count_only {
            let count = stdout.lines().filter(|l| !l.is_empty()).count();
            return Ok(DiskSearchResult::Count(count));
        }

        let mut results = Vec::new();
        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }

            // Parse grep output: file:line:match_text
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() >= 3 {
                let file_path = parts[0].to_string();
                let line_num = parts[1].parse::<usize>().unwrap_or(0);
                let match_text = parts[2].to_string();

                results.push(SearchResult {
                    path: file_path,
                    line: line_num,
                    column: 0,
                    match_text,
                    context_before: None,
                    context_after: None,
                });

                if results.len() >= opts.max_results {
                    break;
                }
            }
        }

        Ok(DiskSearchResult::Matches(results))
    }

    // ── History & versioning (unsupported over SSH) ─────────────────────

    pub async fn history(&self, _path: &str) -> Result<Vec<HistoryEntry>, SandboxError> {
        Err(SandboxError::VfsInvalidOperation(
            "History not supported over SSH".to_string(),
        ))
    }

    pub async fn diff(
        &self,
        _old_path: &str,
        _new_path: &str,
        _context: usize,
    ) -> Result<DiffOutput, SandboxError> {
        Err(SandboxError::VfsInvalidOperation(
            "Diff not supported over SSH".to_string(),
        ))
    }

    pub async fn diff_versions(
        &self,
        _path: &str,
        _old_version: usize,
        _new_version: Option<usize>,
        _context: usize,
    ) -> Result<DiffOutput, SandboxError> {
        Err(SandboxError::VfsInvalidOperation(
            "Diff versions not supported over SSH".to_string(),
        ))
    }

    pub async fn checkout(&self, _path: &str, _version: usize) -> Result<(), SandboxError> {
        Err(SandboxError::VfsInvalidOperation(
            "Checkout not supported over SSH".to_string(),
        ))
    }
}

/// Convert an SFTP error into a SandboxError.
fn sftp_err(e: russh_sftp::client::error::Error) -> SandboxError {
    match &e {
        russh_sftp::client::error::Error::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => SandboxError::VfsNotFound(status.error_message.clone()),
            StatusCode::PermissionDenied => {
                SandboxError::VfsPermissionDenied(status.error_message.clone())
            }
            _ => SandboxError::VfsIo(e.to_string()),
        },
        _ => SandboxError::VfsIo(e.to_string()),
    }
}

/// Detect whether raw bytes are likely text (UTF-8) or binary.
///
/// Returns `("text", true)` if valid UTF-8, `("binary", false)` otherwise.
fn detect_content_type(data: &[u8]) -> (&'static str, bool) {
    // Check for null bytes (strong binary indicator)
    if data.contains(&0) {
        return ("binary", false);
    }
    // Try UTF-8 decoding
    match std::str::from_utf8(data) {
        Ok(_) => ("text", true),
        Err(_) => ("binary", false),
    }
}

/// Shell-escape a string for safe use in remote commands.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
