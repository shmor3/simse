# simse-sandbox Design

**Date:** 2026-03-03
**Status:** Approved

## Overview

simse-sandbox is a new Rust JSON-RPC binary crate providing a unified agent sandbox layer. It combines VFS, VSH, and VNet behind backend-agnostic traits so agents interact with a single sandbox API without knowing whether operations execute locally, over SSH, or (future) inside a Firecracker VM.

## Architecture

### Approach: Trait-per-engine with sandbox orchestrator

Each engine crate defines a backend trait. Two implementations per trait: `LocalBackend` (wraps existing logic) and `SshBackend` (new, uses `russh`). The sandbox crate owns a multiplexed SSH connection, creates backend instances, and exposes all three engines via JSON-RPC.

### Data Flow

```
Agent → JSON-RPC → simse-sandbox
                        │
                        ├─ sandbox/vfs/* → VFS engine (FsBackend trait)
                        │                     ├─ LocalBackend → disk
                        │                     └─ SshBackend → SFTP channel
                        │
                        ├─ sandbox/vsh/* → VSH engine (ShellBackend trait)
                        │                     ├─ LocalBackend → tokio::process
                        │                     └─ SshBackend → exec channel
                        │
                        └─ sandbox/vnet/* → VNet engine (NetBackend trait)
                                              ├─ LocalBackend → reqwest/tokio
                                              ├─ SshBackend → direct-tcpip channel
                                              └─ MockBackend → mock store
```

## Backend Traits

### FsBackend (`simse-vfs`)

```rust
#[async_trait]
pub trait FsBackend: Send + Sync {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    async fn write_file(&self, path: &str, content: &[u8]) -> Result<()>;
    async fn append_file(&self, path: &str, content: &[u8]) -> Result<()>;
    async fn delete_file(&self, path: &str) -> Result<()>;
    async fn mkdir(&self, path: &str, recursive: bool) -> Result<()>;
    async fn readdir(&self, path: &str) -> Result<Vec<DirEntry>>;
    async fn rmdir(&self, path: &str, recursive: bool) -> Result<()>;
    async fn rename(&self, from: &str, to: &str) -> Result<()>;
    async fn copy(&self, from: &str, to: &str) -> Result<()>;
    async fn stat(&self, path: &str) -> Result<FileStat>;
    async fn exists(&self, path: &str) -> Result<bool>;
    async fn glob(&self, pattern: &str) -> Result<Vec<String>>;
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;
    async fn diff(&self, path: &str, version: Option<usize>) -> Result<String>;
}
```

- `LocalBackend`: wraps existing `DiskFs` (for `file://` paths)
- `SshBackend`: SFTP operations over a `russh` channel
- `VirtualFs` (in-memory, `vfs://` paths) stays local-only, no trait needed

### ShellBackend (`simse-vsh`)

```rust
#[async_trait]
pub trait ShellBackend: Send + Sync {
    async fn execute(&self, command: &str, cwd: &str, env: &HashMap<String, String>,
                     timeout_ms: u64) -> Result<ExecResult>;
    async fn execute_git(&self, args: &[String], cwd: &str, env: &HashMap<String, String>,
                         timeout_ms: u64) -> Result<ExecResult>;
}
```

- `LocalBackend`: wraps existing `executor.rs` (tokio::process)
- `SshBackend`: remote exec over a `russh` channel with `cd $cwd && export K=V && ...` prefix
- Session state (env, cwd, aliases, history) stays in `VirtualShell`, backend handles execution only

### NetBackend (`simse-vnet`)

```rust
#[async_trait]
pub trait NetBackend: Send + Sync {
    async fn http_request(&self, req: &HttpRequest) -> Result<HttpResponse>;
    async fn ws_connect(&self, url: &str, headers: &HashMap<String, String>) -> Result<String>;
    async fn ws_send(&self, session_id: &str, message: &str) -> Result<()>;
    async fn ws_close(&self, session_id: &str) -> Result<()>;
    async fn tcp_connect(&self, host: &str, port: u16) -> Result<String>;
    async fn tcp_send(&self, session_id: &str, data: &[u8]) -> Result<()>;
    async fn tcp_close(&self, session_id: &str) -> Result<()>;
    async fn udp_send(&self, host: &str, port: u16, data: &[u8]) -> Result<Vec<u8>>;
    async fn resolve(&self, hostname: &str) -> Result<Vec<String>>;
}
```

- `LocalBackend`: real HTTP (reqwest), TCP/UDP/WS (tokio::net) — new, since VNet currently only has mock
- `SshBackend`: HTTP via direct-tcpip, TCP via direct-tcpip, WS over direct-tcpip, UDP via remote `socat`, DNS via remote `dig`/`getent`
- `MockBackend`: existing mock store (separate layer, wraps any backend, mock takes priority)

## simse-sandbox Crate Structure

```tree
simse-sandbox/
  Cargo.toml
  src/
    lib.rs              # Module declarations, re-exports
    main.rs             # Binary entry point (simse-sandbox-engine)
    error.rs            # SandboxError enum (SANDBOX_ prefix)
    protocol.rs         # JSON-RPC param/result types
    transport.rs        # NdjsonTransport (same pattern as other crates)
    server.rs           # JSON-RPC dispatcher
    sandbox.rs          # Sandbox: unified orchestrator
    config.rs           # SandboxConfig: backend selection, SSH creds, unified policies
    ssh/
      mod.rs            # SSH module root
      pool.rs           # SshPool: multiplexed connection manager (russh)
      channel.rs        # Channel allocation + lifecycle
      fs_backend.rs     # FsBackend impl over SFTP
      shell_backend.rs  # ShellBackend impl over exec channel
      net_backend.rs    # NetBackend impl over direct-tcpip / port forwarding
  tests/
    integration.rs
```

### Sandbox Orchestrator

```rust
pub struct Sandbox {
    config: SandboxConfig,
    vfs: VfsServer,      // VFS engine configured with Local or SSH FsBackend
    vsh: VshServer,      // VSH engine configured with Local or SSH ShellBackend
    vnet: VnetServer,    // VNet engine configured with Local or SSH NetBackend
    ssh_pool: Option<Arc<SshPool>>,  // None if using local backend
}
```

### SandboxConfig

```rust
pub struct SandboxConfig {
    pub backend: BackendConfig,
    pub vfs: VfsConfig,      // VFS-specific sandbox rules
    pub vsh: VshConfig,      // VSH-specific sandbox rules
    pub vnet: VnetConfig,    // VNet-specific sandbox rules
}

pub enum BackendConfig {
    Local,
    Ssh(SshConfig),
}

pub struct SshConfig {
    pub host: String,
    pub port: u16,              // default 22
    pub username: String,
    pub auth: SshAuth,
    pub max_channels: usize,    // max multiplexed channels
    pub keepalive_interval_ms: u64,
}

pub enum SshAuth {
    Key { private_key_path: String, passphrase: Option<String> },
    Password { password: String },
    Agent,  // SSH agent forwarding
}
```

### JSON-RPC Methods

| Domain | Methods |
|--------|---------|
| `sandbox/` | `initialize`, `dispose`, `health`, `switchBackend` |
| `sandbox/vfs/` | All VFS methods (passthrough) |
| `sandbox/vsh/` | All VSH methods (passthrough) |
| `sandbox/vnet/` | All VNet methods (passthrough) |

## SSH Multiplexing

### SshPool

```rust
pub struct SshPool {
    handle: Arc<Handle>,           // russh client handle (one TCP connection)
    config: SshConfig,
    channels: Arc<Mutex<Vec<ChannelHandle>>>,
    health: Arc<AtomicBool>,
}

impl SshPool {
    pub async fn connect(config: SshConfig) -> Result<Self>;
    pub async fn get_exec_channel(&self) -> Result<Channel>;      // for VSH
    pub async fn get_sftp_channel(&self) -> Result<SftpChannel>;  // for VFS
    pub async fn get_direct_tcpip(&self, host: &str, port: u16) -> Result<Channel>; // for VNet
    pub async fn disconnect(&self) -> Result<()>;
    pub fn is_healthy(&self) -> bool;
}
```

- One `russh::client::Handle` = one TCP connection, multiple logical channels
- Each `get_*_channel()` opens a new SSH channel over the same connection
- Health monitoring via `Arc<AtomicBool>` (consistent with simse-acp pattern)
- Auto-reconnect on connection drop with configurable retry

### SSH Backend Details

**SshFsBackend:**
- SFTP channel for file I/O operations
- `glob` and `search` via remote shell commands (`find`, `grep`) since SFTP has no glob
- `diff` runs remote `diff` or fetches both versions for local diffing

**SshShellBackend:**
- Exec channel per command
- `cd $cwd && export KEY=VAL && ...` prefix for session state
- Timeout via channel close after deadline

**SshNetBackend:**
- HTTP: direct-tcpip channel to target host:port, raw HTTP over the channel
- TCP: direct-tcpip channel kept open as persistent session
- WS: direct-tcpip + WebSocket handshake over channel
- UDP: exec channel running `socat` (no native SSH UDP forwarding)
- DNS: exec channel running `dig` or `getent hosts`

## Engine Refactoring

### simse-vfs

1. Add `backend.rs` with `FsBackend` trait
2. Create `local_backend.rs` wrapping existing `DiskFs` methods
3. Keep `VirtualFs` (in-memory) unchanged — always local
4. Modify `server.rs`: `file://` paths use `dyn FsBackend`, `vfs://` stays direct
5. No breaking API changes

### simse-vsh

1. Add `backend.rs` with `ShellBackend` trait
2. Create `local_backend.rs` wrapping existing `executor.rs`
3. Modify `VirtualShell` to accept `Box<dyn ShellBackend>`
4. Session state stays in `VirtualShell`, backend handles execution only
5. No breaking API changes

### simse-vnet

1. Add `backend.rs` with `NetBackend` trait
2. Create `local_backend.rs` — new real network implementation (reqwest + tokio::net)
3. Keep `MockStore` as separate wrapping layer around any backend
4. Modify `VirtualNetwork` to accept `Box<dyn NetBackend>`
5. No breaking API changes

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    SshConnection(String),    // SANDBOX_SSH_CONNECTION
    SshAuth(String),          // SANDBOX_SSH_AUTH
    SshChannel(String),       // SANDBOX_SSH_CHANNEL
    NotInitialized,           // SANDBOX_NOT_INITIALIZED
    BackendSwitch(String),    // SANDBOX_BACKEND_SWITCH
    Vfs(VfsError),            // Pass-through
    Vsh(VshError),            // Pass-through
    Vnet(VnetError),          // Pass-through
    Timeout(String),          // SANDBOX_TIMEOUT
}
```

JSON-RPC error data uses `sandboxCode` key.

## Testing Strategy

- **Unit tests**: SSH backend impls tested against mock SSH server (russh server-side)
- **Integration tests**: Full JSON-RPC round-trips with local backend
- **SSH integration tests**: `#[cfg(feature = "ssh-test")]`, require real SSH target (Docker + sshd in CI)
- **Backend switching test**: Initialize local, switch to SSH, verify operations

## Scope

- **Phase 1 (this design):** Local + SSH backends
- **Phase 2 (future):** Firecracker VM backend (another trait impl)

## Dependencies

- `russh` — pure Rust SSH2 client with multiplexing and SFTP
- `async-trait` — for async trait definitions
- `reqwest` — HTTP client for VNet local backend
- `tokio` — async runtime (already used by VSH and VNet)
- Engine crates as library dependencies: `simse-vfs`, `simse-vsh`, `simse-vnet`
