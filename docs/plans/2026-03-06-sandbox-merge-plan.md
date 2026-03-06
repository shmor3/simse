# Sandbox Merge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Merge simse-vfs, simse-vsh, and simse-vnet into simse-sandbox as a single crate.

**Architecture:** Copy domain logic files into simse-sandbox with `vfs_`/`vsh_`/`vnet_` prefixes, unify all error types into one `SandboxError` enum, replace `Box<dyn Backend>` trait objects with concrete enum dispatch (`FsImpl`/`ShellImpl`/`NetImpl`), delete the three standalone crates.

**Tech Stack:** Rust, tokio, serde, thiserror, reqwest, russh

---

### Task 1: Create feature branch

**Step 1: Create and checkout branch**

```bash
git checkout -b feat/sandbox-merge
```

**Step 2: Commit**

No changes yet — branch created.

---

### Task 2: Update Cargo.toml — add new dependencies

**Files:**
- Modify: `simse-sandbox/Cargo.toml`

**Step 1: Add dependencies from sub-crates**

Add these dependencies (from simse-vfs, simse-vsh, simse-vnet) that simse-sandbox doesn't already have. Keep the three `simse-*-engine` deps for now — we'll remove them in a later task.

Add under `[dependencies]`:
```toml
regex = "1"
sha2 = "0.10"
uuid = { version = "1", features = ["v4"] }
reqwest = { version = "0.12", features = ["json"] }
```

Note: `base64 = "0.22"` is already present.

Add under `[dev-dependencies]`:
```toml
tempfile = "3"
```

**Step 2: Verify it compiles**

Run: `cd simse-sandbox && cargo check`
Expected: compiles with no errors.

**Step 3: Commit**

```bash
git add simse-sandbox/Cargo.toml
git commit -m "chore(sandbox): add dependencies for vfs/vsh/vnet merge"
```

---

### Task 3: Write unified SandboxError

**Files:**
- Modify: `simse-sandbox/src/error.rs`

**Step 1: Replace error.rs with unified enum**

Replace the entire file. The new `SandboxError` inlines all VFS/VSH/VNet error variants with domain prefixes. Keep the `Vfs(VfsError)`, `Vsh(VshError)`, `Vnet(VnetError)` wrapper variants temporarily so existing code still compiles — we'll remove them once domain files are migrated.

```rust
use thiserror::Error;

// Temporary: keep old error imports until domain files are migrated
use simse_vfs_engine::error::VfsError;
use simse_vnet_engine::error::VnetError;
use simse_vsh_engine::error::VshError;

#[derive(Debug, Error)]
pub enum SandboxError {
    // ── Lifecycle ────────────────────────────────────────────────────
    #[error("Not initialized")]
    NotInitialized,
    #[error("Already initialized")]
    AlreadyInitialized,

    // ── VFS ──────────────────────────────────────────────────────────
    #[error("Invalid path: {0}")]
    VfsInvalidPath(String),
    #[error("Not found: {0}")]
    VfsNotFound(String),
    #[error("Already exists: {0}")]
    VfsAlreadyExists(String),
    #[error("Not a file: {0}")]
    VfsNotAFile(String),
    #[error("Not a directory: {0}")]
    VfsNotADirectory(String),
    #[error("Not empty: {0}")]
    VfsNotEmpty(String),
    #[error("VFS limit exceeded: {0}")]
    VfsLimitExceeded(String),
    #[error("Invalid operation: {0}")]
    VfsInvalidOperation(String),
    #[error("Permission denied: {0}")]
    VfsPermissionDenied(String),
    #[error("Disk not configured")]
    VfsDiskNotConfigured,

    // ── VSH ──────────────────────────────────────────────────────────
    #[error("Session not found: {0}")]
    VshSessionNotFound(String),
    #[error("Execution failed: {0}")]
    VshExecutionFailed(String),
    #[error("Command timeout: {0}")]
    VshTimeout(String),
    #[error("Sandbox violation: {0}")]
    VshSandboxViolation(String),
    #[error("VSH limit exceeded: {0}")]
    VshLimitExceeded(String),

    // ── VNet ─────────────────────────────────────────────────────────
    #[error("Network sandbox violation: {0}")]
    VnetSandboxViolation(String),
    #[error("Connection failed: {0}")]
    VnetConnectionFailed(String),
    #[error("Network timeout: {0}")]
    VnetTimeout(String),
    #[error("Network session not found: {0}")]
    VnetSessionNotFound(String),
    #[error("Mock not found: {0}")]
    VnetMockNotFound(String),
    #[error("No mock match: {0}")]
    VnetNoMockMatch(String),
    #[error("VNet limit exceeded: {0}")]
    VnetLimitExceeded(String),
    #[error("Response too large: {0}")]
    VnetResponseTooLarge(String),
    #[error("DNS resolution failed: {0}")]
    VnetDnsResolutionFailed(String),

    // ── SSH ──────────────────────────────────────────────────────────
    #[error("SSH connection error: {0}")]
    SshConnection(String),
    #[error("SSH authentication error: {0}")]
    SshAuth(String),
    #[error("SSH channel error: {0}")]
    SshChannel(String),

    // ── Backend ──────────────────────────────────────────────────────
    #[error("Backend switch error: {0}")]
    BackendSwitch(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),

    // ── Generic ──────────────────────────────────────────────────────
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // ── Temporary wrappers (removed after migration) ─────────────────
    #[error("VFS error: {0}")]
    Vfs(#[from] VfsError),
    #[error("VSH error: {0}")]
    Vsh(#[from] VshError),
    #[error("VNet error: {0}")]
    Vnet(#[from] VnetError),
}

impl SandboxError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "SANDBOX_NOT_INITIALIZED",
            Self::AlreadyInitialized => "SANDBOX_ALREADY_INITIALIZED",

            Self::VfsInvalidPath(_) => "SANDBOX_VFS_INVALID_PATH",
            Self::VfsNotFound(_) => "SANDBOX_VFS_NOT_FOUND",
            Self::VfsAlreadyExists(_) => "SANDBOX_VFS_ALREADY_EXISTS",
            Self::VfsNotAFile(_) => "SANDBOX_VFS_NOT_FILE",
            Self::VfsNotADirectory(_) => "SANDBOX_VFS_NOT_DIRECTORY",
            Self::VfsNotEmpty(_) => "SANDBOX_VFS_NOT_EMPTY",
            Self::VfsLimitExceeded(_) => "SANDBOX_VFS_LIMIT_EXCEEDED",
            Self::VfsInvalidOperation(_) => "SANDBOX_VFS_INVALID_OPERATION",
            Self::VfsPermissionDenied(_) => "SANDBOX_VFS_PERMISSION_DENIED",
            Self::VfsDiskNotConfigured => "SANDBOX_VFS_DISK_NOT_CONFIGURED",

            Self::VshSessionNotFound(_) => "SANDBOX_VSH_SESSION_NOT_FOUND",
            Self::VshExecutionFailed(_) => "SANDBOX_VSH_EXECUTION_FAILED",
            Self::VshTimeout(_) => "SANDBOX_VSH_TIMEOUT",
            Self::VshSandboxViolation(_) => "SANDBOX_VSH_SANDBOX_VIOLATION",
            Self::VshLimitExceeded(_) => "SANDBOX_VSH_LIMIT_EXCEEDED",

            Self::VnetSandboxViolation(_) => "SANDBOX_VNET_SANDBOX_VIOLATION",
            Self::VnetConnectionFailed(_) => "SANDBOX_VNET_CONNECTION_FAILED",
            Self::VnetTimeout(_) => "SANDBOX_VNET_TIMEOUT",
            Self::VnetSessionNotFound(_) => "SANDBOX_VNET_SESSION_NOT_FOUND",
            Self::VnetMockNotFound(_) => "SANDBOX_VNET_MOCK_NOT_FOUND",
            Self::VnetNoMockMatch(_) => "SANDBOX_VNET_NO_MOCK_MATCH",
            Self::VnetLimitExceeded(_) => "SANDBOX_VNET_LIMIT_EXCEEDED",
            Self::VnetResponseTooLarge(_) => "SANDBOX_VNET_RESPONSE_TOO_LARGE",
            Self::VnetDnsResolutionFailed(_) => "SANDBOX_VNET_DNS_FAILED",

            Self::SshConnection(_) => "SANDBOX_SSH_CONNECTION",
            Self::SshAuth(_) => "SANDBOX_SSH_AUTH",
            Self::SshChannel(_) => "SANDBOX_SSH_CHANNEL",

            Self::BackendSwitch(_) => "SANDBOX_BACKEND_SWITCH",
            Self::InvalidParams(_) => "SANDBOX_INVALID_PARAMS",

            Self::Io(_) => "SANDBOX_IO_ERROR",
            Self::Json(_) => "SANDBOX_JSON_ERROR",

            // Temporary wrappers delegate to inner error code
            Self::Vfs(e) => e.code(),
            Self::Vsh(e) => e.code(),
            Self::Vnet(e) => e.code(),
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "sandboxCode": self.code(),
            "message": self.to_string(),
        })
    }
}
```

**Step 2: Verify it compiles**

Run: `cd simse-sandbox && cargo check`
Expected: compiles — existing code still uses `Vfs(VfsError)` etc. wrappers.

**Step 3: Commit**

```bash
git add simse-sandbox/src/error.rs
git commit -m "feat(sandbox): unified SandboxError with domain-prefixed variants"
```

---

### Task 4: Copy VFS domain files into simse-sandbox

**Files:**
- Create: `simse-sandbox/src/vfs_store.rs` (from `simse-vfs/src/vfs.rs`)
- Create: `simse-sandbox/src/vfs_disk.rs` (from `simse-vfs/src/disk.rs`)
- Create: `simse-sandbox/src/vfs_diff.rs` (from `simse-vfs/src/diff.rs`)
- Create: `simse-sandbox/src/vfs_glob.rs` (from `simse-vfs/src/glob.rs`)
- Create: `simse-sandbox/src/vfs_search.rs` (from `simse-vfs/src/search.rs`)
- Create: `simse-sandbox/src/vfs_path.rs` (from `simse-vfs/src/path.rs`)
- Modify: `simse-sandbox/src/lib.rs`

**Step 1: Copy files with renaming**

```bash
cp simse-vfs/src/vfs.rs simse-sandbox/src/vfs_store.rs
cp simse-vfs/src/disk.rs simse-sandbox/src/vfs_disk.rs
cp simse-vfs/src/diff.rs simse-sandbox/src/vfs_diff.rs
cp simse-vfs/src/glob.rs simse-sandbox/src/vfs_glob.rs
cp simse-vfs/src/search.rs simse-sandbox/src/vfs_search.rs
cp simse-vfs/src/path.rs simse-sandbox/src/vfs_path.rs
```

**Step 2: Update imports in all copied files**

In each file, replace `crate::` references to point to the new module names:

- `crate::error::VfsError` → `crate::error::SandboxError`
- `crate::diff::` → `crate::vfs_diff::`
- `crate::glob::` → `crate::vfs_glob::`
- `crate::search::` → `crate::vfs_search::`
- `crate::path::` → `crate::vfs_path::`
- `crate::protocol::` → `crate::vfs_protocol::` ... but wait — the VFS protocol types are already in `simse-sandbox/src/protocol.rs`. So: replace `crate::protocol::{DirEntry, HistoryEntry, ReadFileResult, StatResult, ...}` with the types as they exist in the sandbox protocol module, OR inline these types into the VFS files if they don't exist in sandbox protocol.

Actually, check: the sandbox `server.rs` already imports VFS types from `simse_vfs_engine::vfs::*` and `simse_vfs_engine::disk::*` directly. The VFS `protocol.rs` types (`DirEntry`, `HistoryEntry`, `ReadFileResult`, `StatResult`) are defined in `simse-vfs/src/protocol.rs` and used by both `vfs.rs` and `disk.rs`. These types need to move too.

Create `simse-sandbox/src/vfs_types.rs` from the struct definitions in `simse-vfs/src/protocol.rs` (just the data types — `DirEntry`, `HistoryEntry`, `ReadFileResult`, `StatResult`, `SearchResult` — NOT the JSON-RPC request/response param types).

**Step 3: Create vfs_types.rs**

Copy the data type structs from `simse-vfs/src/protocol.rs` into `simse-sandbox/src/vfs_types.rs`. These are the types like `DirEntry`, `HistoryEntry`, `ReadFileResult`, `StatResult` that are used by VFS domain logic. Do NOT copy JSON-RPC method parameter/result types (those belong in `protocol.rs`).

**Step 4: Update all VfsError usage to SandboxError**

In each copied VFS file, change every `VfsError::X(msg)` to `SandboxError::VfsX(msg)`:
- `VfsError::InvalidPath(x)` → `SandboxError::VfsInvalidPath(x)`
- `VfsError::NotFound(x)` → `SandboxError::VfsNotFound(x)`
- `VfsError::AlreadyExists(x)` → `SandboxError::VfsAlreadyExists(x)`
- `VfsError::NotAFile(x)` → `SandboxError::VfsNotAFile(x)`
- `VfsError::NotADirectory(x)` → `SandboxError::VfsNotADirectory(x)`
- `VfsError::NotEmpty(x)` → `SandboxError::VfsNotEmpty(x)`
- `VfsError::LimitExceeded(x)` → `SandboxError::VfsLimitExceeded(x)`
- `VfsError::InvalidOperation(x)` → `SandboxError::VfsInvalidOperation(x)`
- `VfsError::PermissionDenied(x)` → `SandboxError::VfsPermissionDenied(x)`
- `VfsError::DiskNotConfigured` → `SandboxError::VfsDiskNotConfigured`

Also update all `Result<_, VfsError>` return types to `Result<_, SandboxError>`.

**Step 5: Add modules to lib.rs**

Add to `simse-sandbox/src/lib.rs`:
```rust
pub mod vfs_store;
pub mod vfs_disk;
pub mod vfs_diff;
pub mod vfs_glob;
pub mod vfs_search;
pub mod vfs_path;
pub mod vfs_types;
```

**Step 6: Verify it compiles**

Run: `cd simse-sandbox && cargo check`
Fix any remaining import issues until it compiles.

**Step 7: Commit**

```bash
git add simse-sandbox/src/vfs_*.rs simse-sandbox/src/lib.rs
git commit -m "feat(sandbox): move VFS domain files into sandbox crate"
```

---

### Task 5: Copy VSH domain files into simse-sandbox

**Files:**
- Create: `simse-sandbox/src/vsh_shell.rs` (from `simse-vsh/src/shell.rs`)
- Create: `simse-sandbox/src/vsh_executor.rs` (from `simse-vsh/src/executor.rs`)
- Create: `simse-sandbox/src/vsh_sandbox.rs` (from `simse-vsh/src/sandbox.rs`)
- Modify: `simse-sandbox/src/lib.rs`

**Step 1: Copy files**

```bash
cp simse-vsh/src/shell.rs simse-sandbox/src/vsh_shell.rs
cp simse-vsh/src/executor.rs simse-sandbox/src/vsh_executor.rs
cp simse-vsh/src/sandbox.rs simse-sandbox/src/vsh_sandbox.rs
```

**Step 2: Update imports in all copied files**

- `crate::error::VshError` → `crate::error::SandboxError`
- `crate::executor::` → `crate::vsh_executor::`
- `crate::sandbox::` → `crate::vsh_sandbox::`
- `crate::backend::ShellBackend` → remove (will be replaced by enum dispatch later)

**Step 3: Update all VshError usage to SandboxError**

- `VshError::NotInitialized` → `SandboxError::NotInitialized`
- `VshError::SessionNotFound(x)` → `SandboxError::VshSessionNotFound(x)`
- `VshError::ExecutionFailed(x)` → `SandboxError::VshExecutionFailed(x)`
- `VshError::Timeout(x)` → `SandboxError::VshTimeout(x)`
- `VshError::SandboxViolation(x)` → `SandboxError::VshSandboxViolation(x)`
- `VshError::InvalidParams(x)` → `SandboxError::InvalidParams(x)`
- `VshError::LimitExceeded(x)` → `SandboxError::VshLimitExceeded(x)`

**Step 4: Handle VirtualShell's backend field**

`VirtualShell` currently holds `backend: Box<dyn ShellBackend>`. Temporarily change to accept a generic or keep as `Box<dyn ShellBackend>` — it will be replaced by `ShellImpl` in Task 8. For now, make VirtualShell take the executor functions directly (since `LocalShellBackend` is just a thin wrapper over `vsh_executor::execute_command` and `vsh_executor::execute_git`).

Actually, for compile-ability, keep the `ShellBackend` trait temporarily in `vsh_shell.rs` as a private trait, or refactor `VirtualShell` to call executor functions directly. The simplest approach: inline the `execute_command`/`execute_git` calls into `VirtualShell` methods, removing the backend indirection. The SSH backend will be wired through `ShellImpl` enum in Task 8.

**Step 5: Add modules to lib.rs**

```rust
pub mod vsh_shell;
pub mod vsh_executor;
pub mod vsh_sandbox;
```

**Step 6: Verify it compiles**

Run: `cd simse-sandbox && cargo check`

**Step 7: Commit**

```bash
git add simse-sandbox/src/vsh_*.rs simse-sandbox/src/lib.rs
git commit -m "feat(sandbox): move VSH domain files into sandbox crate"
```

---

### Task 6: Copy VNet domain files into simse-sandbox

**Files:**
- Create: `simse-sandbox/src/vnet_network.rs` (from `simse-vnet/src/network.rs`)
- Create: `simse-sandbox/src/vnet_sandbox.rs` (from `simse-vnet/src/sandbox.rs`)
- Create: `simse-sandbox/src/vnet_mock_store.rs` (from `simse-vnet/src/mock_store.rs`)
- Create: `simse-sandbox/src/vnet_session.rs` (from `simse-vnet/src/session.rs`)
- Modify: `simse-sandbox/src/lib.rs`

**Step 1: Copy files**

```bash
cp simse-vnet/src/network.rs simse-sandbox/src/vnet_network.rs
cp simse-vnet/src/sandbox.rs simse-sandbox/src/vnet_sandbox.rs
cp simse-vnet/src/mock_store.rs simse-sandbox/src/vnet_mock_store.rs
cp simse-vnet/src/session.rs simse-sandbox/src/vnet_session.rs
```

**Step 2: Update imports**

- `crate::error::VnetError` → `crate::error::SandboxError`
- `crate::mock_store::` → `crate::vnet_mock_store::`
- `crate::session::` → `crate::vnet_session::`
- `crate::sandbox::` → `crate::vnet_sandbox::`
- `crate::protocol::HttpResponseResult` → move `HttpResponseResult` to `vnet_network.rs` or a shared types file
- `crate::backend::NetBackend` → remove (replaced by enum dispatch later)

**Step 3: Update all VnetError usage to SandboxError**

- `VnetError::NotInitialized` → `SandboxError::NotInitialized`
- `VnetError::SandboxViolation(x)` → `SandboxError::VnetSandboxViolation(x)`
- `VnetError::ConnectionFailed(x)` → `SandboxError::VnetConnectionFailed(x)`
- `VnetError::Timeout(x)` → `SandboxError::VnetTimeout(x)`
- `VnetError::SessionNotFound(x)` → `SandboxError::VnetSessionNotFound(x)`
- `VnetError::MockNotFound(x)` → `SandboxError::VnetMockNotFound(x)`
- `VnetError::NoMockMatch(x)` → `SandboxError::VnetNoMockMatch(x)`
- `VnetError::LimitExceeded(x)` → `SandboxError::VnetLimitExceeded(x)`
- `VnetError::InvalidParams(x)` → `SandboxError::InvalidParams(x)`
- `VnetError::ResponseTooLarge(x)` → `SandboxError::VnetResponseTooLarge(x)`
- `VnetError::DnsResolutionFailed(x)` → `SandboxError::VnetDnsResolutionFailed(x)`

**Step 4: Handle VirtualNetwork's backend field**

Same approach as VSH: `VirtualNetwork` currently holds `Option<Box<dyn NetBackend>>`. Temporarily keep compiling — will be replaced by `NetImpl` enum in Task 8.

**Step 5: Add modules to lib.rs**

```rust
pub mod vnet_network;
pub mod vnet_sandbox;
pub mod vnet_mock_store;
pub mod vnet_session;
```

**Step 6: Verify it compiles**

Run: `cd simse-sandbox && cargo check`

**Step 7: Commit**

```bash
git add simse-sandbox/src/vnet_*.rs simse-sandbox/src/lib.rs
git commit -m "feat(sandbox): move VNet domain files into sandbox crate"
```

---

### Task 7: Move local backend logic into domain files

**Files:**
- Modify: `simse-sandbox/src/vfs_disk.rs` (absorb `LocalFsBackend` logic — it's just a thin wrapper)
- Modify: `simse-sandbox/src/vsh_executor.rs` (already contains execution logic; `LocalShellBackend` was a passthrough)
- Create: `simse-sandbox/src/vnet_local.rs` (from `simse-vnet/src/local_backend.rs` — contains reqwest HTTP logic)

**Step 1: VFS local backend**

`LocalFsBackend` was just a wrapper around `DiskFs` — every method simply delegated. No separate file needed. The `DiskFs` in `vfs_disk.rs` IS the local backend.

**Step 2: VSH local backend**

`LocalShellBackend` was a passthrough to `executor::execute_command` and `executor::execute_git`. No separate file needed. `vsh_executor.rs` already has the logic.

**Step 3: VNet local backend**

`LocalNetBackend` contains real HTTP logic (reqwest client, DNS resolution). Copy from `simse-vnet/src/local_backend.rs` into `simse-sandbox/src/vnet_local.rs`. Update error types from `VnetError` to `SandboxError`. Remove the `#[async_trait]` and `impl NetBackend for` — make the methods standalone `impl LocalNet` methods.

```bash
cp simse-vnet/src/local_backend.rs simse-sandbox/src/vnet_local.rs
```

Update imports and error types in the copied file.

**Step 4: Add module to lib.rs**

```rust
pub mod vnet_local;
```

**Step 5: Verify it compiles**

Run: `cd simse-sandbox && cargo check`

**Step 6: Commit**

```bash
git add simse-sandbox/src/vnet_local.rs simse-sandbox/src/lib.rs
git commit -m "feat(sandbox): move local backend logic into sandbox crate"
```

---

### Task 8: Create backend enum dispatch

**Files:**
- Create: `simse-sandbox/src/vfs_backend.rs`
- Create: `simse-sandbox/src/vsh_backend.rs`
- Create: `simse-sandbox/src/vnet_backend.rs`
- Modify: `simse-sandbox/src/ssh/fs_backend.rs` → rename to `simse-sandbox/src/ssh/fs.rs`
- Modify: `simse-sandbox/src/ssh/shell_backend.rs` → rename to `simse-sandbox/src/ssh/shell.rs`
- Modify: `simse-sandbox/src/ssh/net_backend.rs` → rename to `simse-sandbox/src/ssh/net.rs`
- Modify: `simse-sandbox/src/ssh/mod.rs`
- Modify: `simse-sandbox/src/lib.rs`

**Step 1: Rename SSH backend files**

```bash
mv simse-sandbox/src/ssh/fs_backend.rs simse-sandbox/src/ssh/fs.rs
mv simse-sandbox/src/ssh/shell_backend.rs simse-sandbox/src/ssh/shell.rs
mv simse-sandbox/src/ssh/net_backend.rs simse-sandbox/src/ssh/net.rs
```

Update `simse-sandbox/src/ssh/mod.rs`:
```rust
pub mod channel;
pub mod pool;
pub mod fs;
pub mod shell;
pub mod net;
```

**Step 2: Update SSH backend files**

In each SSH backend file, replace old cross-crate imports:
- `simse_vfs_engine::backend::FsBackend` → remove trait impl, make methods standalone
- `simse_vfs_engine::error::VfsError` → `crate::error::SandboxError`
- `simse_vfs_engine::diff::DiffOutput` → `crate::vfs_diff::DiffOutput`
- `simse_vfs_engine::disk::*` → `crate::vfs_disk::*`
- `simse_vfs_engine::protocol::*` → `crate::vfs_types::*`
- `simse_vsh_engine::backend::ShellBackend` → remove trait impl
- `simse_vsh_engine::error::VshError` → `crate::error::SandboxError`
- `simse_vsh_engine::executor::ExecResult` → `crate::vsh_executor::ExecResult`
- `simse_vnet_engine::backend::NetBackend` → remove trait impl
- `simse_vnet_engine::error::VnetError` → `crate::error::SandboxError`
- `simse_vnet_engine::protocol::HttpResponseResult` → use the type from wherever it now lives

Remove `#[async_trait]` and `impl XBackend for SshX` — make methods plain `impl SshFs`, `impl SshShell`, `impl SshNet`.

**Step 3: Create FsImpl enum**

Create `simse-sandbox/src/vfs_backend.rs`:

```rust
use crate::error::SandboxError;
use crate::vfs_disk::DiskFs;
use crate::vfs_diff::DiffOutput;
use crate::vfs_types::*;
use crate::ssh::fs::SshFs;

pub enum FsImpl {
    Local(DiskFs),
    Ssh(SshFs),
}

impl FsImpl {
    pub async fn read_file(&self, path: &str) -> Result<ReadFileResult, SandboxError> {
        match self {
            Self::Local(d) => d.read_file(path).map_err(Into::into),
            Self::Ssh(s) => s.read_file(path).await,
        }
    }
    // ... all 16 methods from FsBackend trait, each dispatching via match
}
```

**Step 4: Create ShellImpl enum**

Create `simse-sandbox/src/vsh_backend.rs`:

```rust
use crate::error::SandboxError;
use crate::vsh_executor::ExecResult;
use crate::ssh::shell::SshShell;
use std::collections::HashMap;
use std::path::Path;

pub enum ShellImpl {
    Local(LocalShell),
    Ssh(SshShell),
}

pub struct LocalShell;

impl ShellImpl {
    pub async fn execute_command(
        &self,
        command: &str,
        cwd: &Path,
        env: &HashMap<String, String>,
        shell: &str,
        timeout_ms: u64,
        max_output_bytes: usize,
        stdin_input: Option<&str>,
    ) -> Result<ExecResult, SandboxError> {
        match self {
            Self::Local(_) => {
                crate::vsh_executor::execute_command(
                    command, cwd, env, shell, timeout_ms, max_output_bytes, stdin_input,
                ).await
            }
            Self::Ssh(s) => {
                s.execute_command(command, cwd, env, shell, timeout_ms, max_output_bytes, stdin_input).await
            }
        }
    }

    pub async fn execute_git(
        &self,
        args: &[String],
        cwd: &Path,
        env: &HashMap<String, String>,
        timeout_ms: u64,
        max_output_bytes: usize,
    ) -> Result<ExecResult, SandboxError> {
        match self {
            Self::Local(_) => {
                crate::vsh_executor::execute_git(args, cwd, env, timeout_ms, max_output_bytes).await
            }
            Self::Ssh(s) => s.execute_git(args, cwd, env, timeout_ms, max_output_bytes).await,
        }
    }
}
```

**Step 5: Create NetImpl enum**

Create `simse-sandbox/src/vnet_backend.rs`:

```rust
use crate::error::SandboxError;
use crate::vnet_local::LocalNet;
use crate::ssh::net::SshNet;
use std::collections::HashMap;

pub enum NetImpl {
    Local(LocalNet),
    Ssh(SshNet),
}

impl NetImpl {
    pub async fn http_request(...) -> Result<HttpResponseResult, SandboxError> {
        match self { ... }
    }
    // ... all 9 methods from NetBackend trait
}
```

**Step 6: Add modules to lib.rs**

```rust
pub mod vfs_backend;
pub mod vsh_backend;
pub mod vnet_backend;
```

**Step 7: Verify it compiles**

Run: `cd simse-sandbox && cargo check`

**Step 8: Commit**

```bash
git add simse-sandbox/src/vfs_backend.rs simse-sandbox/src/vsh_backend.rs simse-sandbox/src/vnet_backend.rs simse-sandbox/src/ssh/ simse-sandbox/src/lib.rs
git commit -m "feat(sandbox): create backend enum dispatch (FsImpl/ShellImpl/NetImpl)"
```

---

### Task 9: Update sandbox.rs to use enum dispatch

**Files:**
- Modify: `simse-sandbox/src/sandbox.rs`

**Step 1: Replace trait object fields with enum fields**

Replace:
```rust
use simse_vfs_engine::backend::FsBackend;
// etc.
fs_backend: Option<Box<dyn FsBackend>>,
```

With:
```rust
use crate::vfs_backend::FsImpl;
use crate::vsh_backend::{ShellImpl, LocalShell};
use crate::vnet_backend::NetImpl;
use crate::vnet_local::LocalNet;
// etc.
fs_backend: Option<FsImpl>,
```

**Step 2: Replace all simse_vfs_engine/simse_vsh_engine/simse_vnet_engine imports**

Replace cross-crate imports with local module imports:
- `simse_vfs_engine::vfs::VirtualFs` → `crate::vfs_store::VirtualFs`
- `simse_vfs_engine::disk::DiskFs` → `crate::vfs_disk::DiskFs`
- `simse_vfs_engine::path::VfsLimits` → `crate::vfs_path::VfsLimits`
- `simse_vsh_engine::shell::VirtualShell` → `crate::vsh_shell::VirtualShell`
- `simse_vsh_engine::sandbox::SandboxConfig` → `crate::vsh_sandbox::SandboxConfig`
- `simse_vnet_engine::network::VirtualNetwork` → `crate::vnet_network::VirtualNetwork`
- etc.

**Step 3: Update backend creation methods**

Replace `Box::new(LocalFsBackend::new(disk))` with `FsImpl::Local(disk)`.
Replace `Box::new(SshFsBackend::new(pool, root))` with `FsImpl::Ssh(SshFs::new(pool, root))`.
Same pattern for `ShellImpl` and `NetImpl`.

**Step 4: Update VirtualShell construction**

`VirtualShell::new()` no longer takes `Box<dyn ShellBackend>`. Instead it takes `ShellImpl`. Update the constructor call and `VirtualShell` struct.

**Step 5: Update VirtualNetwork construction**

`VirtualNetwork` no longer takes `Option<Box<dyn NetBackend>>`. Instead it takes `Option<NetImpl>`. Update accordingly.

**Step 6: Verify it compiles**

Run: `cd simse-sandbox && cargo check`

**Step 7: Commit**

```bash
git add simse-sandbox/src/sandbox.rs
git commit -m "refactor(sandbox): use enum dispatch in sandbox orchestrator"
```

---

### Task 10: Update server.rs imports

**Files:**
- Modify: `simse-sandbox/src/server.rs`

**Step 1: Replace all cross-crate imports**

Replace:
```rust
use simse_vfs_engine::diff::DiffOutput;
use simse_vfs_engine::disk::{DiskSearchMode, DiskSearchOptions, DiskSearchResult};
use simse_vfs_engine::vfs::{...};
use simse_vnet_engine::mock_store::MockResponse;
use simse_vnet_engine::session::{Scheme as VnetScheme, SessionType as VnetSessionType};
```

With:
```rust
use crate::vfs_diff::DiffOutput;
use crate::vfs_disk::{DiskSearchMode, DiskSearchOptions, DiskSearchResult};
use crate::vfs_store::{...};
use crate::vnet_mock_store::MockResponse;
use crate::vnet_session::{Scheme as VnetScheme, SessionType as VnetSessionType};
```

**Step 2: Update error handling in server methods**

Where server methods currently use `?` to propagate VfsError/VshError/VnetError through the `From` impls, the domain files now return `SandboxError` directly. So most `?` usage should still work. Remove any explicit `.map_err(SandboxError::Vfs)` calls since errors are already `SandboxError`.

**Step 3: Verify it compiles**

Run: `cd simse-sandbox && cargo check`

**Step 4: Commit**

```bash
git add simse-sandbox/src/server.rs
git commit -m "refactor(sandbox): update server.rs imports to use local modules"
```

---

### Task 11: Remove old crate dependencies and temporary error wrappers

**Files:**
- Modify: `simse-sandbox/Cargo.toml`
- Modify: `simse-sandbox/src/error.rs`

**Step 1: Remove old dependencies from Cargo.toml**

Remove these three lines:
```toml
simse-vfs-engine = { path = "../simse-vfs" }
simse-vsh-engine = { path = "../simse-vsh" }
simse-vnet-engine = { path = "../simse-vnet" }
```

Also remove `async-trait` if no longer used.

**Step 2: Remove temporary wrapper variants from error.rs**

Remove from `SandboxError`:
```rust
// These three variants and their From impls:
Vfs(#[from] VfsError),
Vsh(#[from] VshError),
Vnet(#[from] VnetError),
```

Remove the old error imports:
```rust
use simse_vfs_engine::error::VfsError;
use simse_vnet_engine::error::VnetError;
use simse_vsh_engine::error::VshError;
```

Remove the delegation arms in `code()`:
```rust
Self::Vfs(e) => e.code(),
Self::Vsh(e) => e.code(),
Self::Vnet(e) => e.code(),
```

**Step 3: Verify it compiles**

Run: `cd simse-sandbox && cargo check`
Expected: compiles — all code now uses `SandboxError` directly.

**Step 4: Commit**

```bash
git add simse-sandbox/Cargo.toml simse-sandbox/src/error.rs
git commit -m "refactor(sandbox): remove old crate deps and temporary error wrappers"
```

---

### Task 12: Write VFS tests

**Files:**
- Create: `simse-sandbox/tests/vfs.rs`

**Step 1: Write direct Rust API tests for VirtualFs and DiskFs**

Port the 38 tests from `simse-vfs/tests/integration.rs` to direct Rust API calls. Instead of spawning a binary and sending JSON-RPC, call `VirtualFs::new()` and `DiskFs::new()` directly.

Key test categories:
- VirtualFs: write/read, mkdir/readdir, delete, rename, search, glob, diff, snapshot/restore, transaction, limits
- DiskFs: write/read, append, delete, mkdir/readdir, rmdir, stat, exists, rename, copy, glob, tree, du, search, sandbox violations, history, diff, checkout

Each test creates its own `VirtualFs` or `DiskFs` instance — no shared state.

Use `tempfile::TempDir` for DiskFs tests (same as the old integration tests).

**Step 2: Run tests**

Run: `cd simse-sandbox && cargo test --test vfs`
Expected: all tests pass.

**Step 3: Commit**

```bash
git add simse-sandbox/tests/vfs.rs
git commit -m "test(sandbox): add direct Rust API tests for VFS domain"
```

---

### Task 13: Write VSH tests

**Files:**
- Create: `simse-sandbox/tests/vsh.rs`

**Step 1: Write direct Rust API tests for VirtualShell**

Port the 14 tests from `simse-vsh/tests/integration.rs`. Call `VirtualShell::new()` directly with a local backend.

Key test categories:
- Session CRUD, exec with env, raw exec, cwd changes, aliases, history, sandbox violation, timeout, env operations, metrics

Use `tempfile::TempDir` for sandbox root.

**Step 2: Run tests**

Run: `cd simse-sandbox && cargo test --test vsh`
Expected: all tests pass.

**Step 3: Commit**

```bash
git add simse-sandbox/tests/vsh.rs
git commit -m "test(sandbox): add direct Rust API tests for VSH domain"
```

---

### Task 14: Write VNet tests

**Files:**
- Create: `simse-sandbox/tests/vnet.rs`

**Step 1: Write direct Rust API tests for VirtualNetwork**

Port the 16 tests from `simse-vnet/tests/integration.rs`. Call `VirtualNetwork::new()` directly.

Key test categories:
- Initialize, mock register/match, glob patterns, mock times limit, mock list/clear/history, WS/TCP/UDP session lifecycle, metrics, sandbox validation

**Step 2: Run tests**

Run: `cd simse-sandbox && cargo test --test vnet`
Expected: all tests pass.

**Step 3: Commit**

```bash
git add simse-sandbox/tests/vnet.rs
git commit -m "test(sandbox): add direct Rust API tests for VNet domain"
```

---

### Task 15: Run full test suite

**Step 1: Run all sandbox tests**

Run: `cd simse-sandbox && cargo test`
Expected: all tests pass (existing integration tests + new vfs/vsh/vnet tests).

**Step 2: Run clippy**

Run: `cd simse-sandbox && cargo clippy -- -D warnings`
Expected: no warnings.

**Step 3: Commit any fixes**

If clippy finds issues, fix and commit.

---

### Task 16: Delete old crate directories

**Files:**
- Delete: `simse-vfs/` (entire directory)
- Delete: `simse-vsh/` (entire directory)
- Delete: `simse-vnet/` (entire directory)
- Modify: `Cargo.toml` (workspace root)
- Modify: `CLAUDE.md`

**Step 1: Remove directories**

```bash
rm -rf simse-vfs simse-vsh simse-vnet
```

**Step 2: Update workspace Cargo.toml**

Remove from `exclude` list:
```toml
"simse-vfs",
"simse-vsh",
"simse-vnet",
```

**Step 3: Update CLAUDE.md**

- Remove `simse-vfs`, `simse-vsh`, `simse-vnet` from the repository layout section
- Update the "Other Rust Crates" section to remove simse-vfs, simse-vsh, simse-vnet entries
- Update simse-sandbox description to reflect it now contains all VFS/VSH/VNet logic
- Update build/test commands to remove `simse-vfs`, `simse-vsh`, `simse-vnet` entries

**Step 4: Verify workspace still builds**

Run: `cd simse-sandbox && cargo build`

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: delete simse-vfs, simse-vsh, simse-vnet crates (merged into sandbox)"
```

---

### Task 17: Final verification

**Step 1: Clean build**

```bash
cd simse-sandbox && cargo clean && cargo build --release
```

**Step 2: Full test suite**

```bash
cd simse-sandbox && cargo test
```

**Step 3: Verify no references to old crates remain**

```bash
grep -r "simse-vfs-engine\|simse-vsh-engine\|simse-vnet-engine" --include="*.rs" --include="*.toml" .
```

Expected: no matches.

**Step 4: Commit any final fixes**

---

## Task Dependency Graph

```
T1 (branch) → T2 (deps) → T3 (error) → T4 (vfs) → T5 (vsh) → T6 (vnet) → T7 (local backends) → T8 (enum dispatch) → T9 (sandbox.rs) → T10 (server.rs) → T11 (remove old deps)
                                                                                                                                                                          ↓
                                                                                                                                                                    T12 (vfs tests)
                                                                                                                                                                    T13 (vsh tests)
                                                                                                                                                                    T14 (vnet tests)
                                                                                                                                                                          ↓
                                                                                                                                                                    T15 (full suite) → T16 (delete dirs) → T17 (final verify)
```

Tasks 12-14 can run in parallel after Task 11.
