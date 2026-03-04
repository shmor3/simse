# simse-sandbox Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a unified sandbox crate combining VFS, VSH, and VNet behind backend-agnostic traits, with Local and SSH backends.

**Architecture:** Each engine crate gets a backend trait + LocalBackend impl. simse-sandbox owns SSH multiplexing (russh) and provides SshBackend impls. Sandbox exposes all three engines via JSON-RPC.

**Tech Stack:** Rust, russh, async-trait, tokio, reqwest, serde, thiserror

**Design Doc:** `docs/plans/2026-03-03-simse-sandbox-design.md`

---

## Phase 1: Engine Backend Traits

### Task 1: VFS FsBackend Trait + LocalBackend

**Files:**
- Create: `simse-vfs/src/backend.rs`
- Create: `simse-vfs/src/local_backend.rs`
- Modify: `simse-vfs/src/lib.rs`
- Modify: `simse-vfs/Cargo.toml`

**Step 1: Add async dependencies to VFS Cargo.toml**

Add `async-trait = "0.1"` and `tokio = { version = "1.0", features = ["rt-multi-thread", "macros"] }` to `[dependencies]`.

**Step 2: Create the FsBackend trait**

Create `simse-vfs/src/backend.rs` with an `#[async_trait]` trait `FsBackend: Send + Sync` mirroring all `DiskFs` public methods:
- `read_file`, `write_file`, `append_file`, `delete_file`
- `mkdir`, `readdir`, `rmdir`, `stat`, `exists`
- `rename`, `copy`, `glob`, `tree`, `du`
- `search`, `history`, `diff`, `diff_versions`, `checkout`

All return `Result<T, VfsError>` matching the existing DiskFs signatures exactly (ReadFileResult, StatResult, DirEntry, DiffOutput, HistoryEntry, DiskSearchResult).

**Step 3: Create LocalFsBackend wrapping DiskFs**

Create `simse-vfs/src/local_backend.rs`:
- Struct `LocalFsBackend` holding a `DiskFs`
- `impl FsBackend for LocalFsBackend` — each async method simply delegates to `self.disk.method()`
- Constructor `new(disk: DiskFs) -> Self`
- Accessor `disk(&self) -> &DiskFs`

**Step 4: Register modules in lib.rs**

Add `pub mod backend;` and `pub mod local_backend;` to `simse-vfs/src/lib.rs`.

**Step 5: Run existing tests**

Run: `cd simse-vfs && cargo test`
Expected: All existing tests pass.

**Step 6: Commit**

```
feat(simse-vfs): add FsBackend trait and LocalFsBackend implementation
```

---

### Task 2: VFS Server Refactor to Use FsBackend

**Files:**
- Modify: `simse-vfs/src/server.rs`
- Modify: `simse-vfs/src/main.rs`

**Step 1: Update VfsServer to hold `Box<dyn FsBackend>`**

In `simse-vfs/src/server.rs`:
- Replace `disk: Option<DiskFs>` with `disk_backend: Option<Box<dyn FsBackend>>`
- In `initialize` handler, create `LocalFsBackend` wrapping the `DiskFs` and store it
- Update all `file://` handlers to call `self.disk_backend.method().await`
- Make `run()` async and handler methods for `file://` scheme async

**Step 2: Make main.rs async**

Update `simse-vfs/src/main.rs` to `#[tokio::main]` and call `server.run().await`.

**Step 3: Run existing tests**

Run: `cd simse-vfs && cargo test`
Expected: All existing tests pass (behavior unchanged).

**Step 4: Commit**

```
refactor(simse-vfs): route file:// operations through FsBackend trait
```

---

### Task 3: VSH ShellBackend Trait + LocalBackend

**Files:**
- Create: `simse-vsh/src/backend.rs`
- Create: `simse-vsh/src/local_backend.rs`
- Modify: `simse-vsh/src/lib.rs`
- Modify: `simse-vsh/Cargo.toml`

**Step 1: Add async-trait**

Add `async-trait = "0.1"` to `simse-vsh/Cargo.toml`.

**Step 2: Create ShellBackend trait**

Create `simse-vsh/src/backend.rs` with `#[async_trait]` trait `ShellBackend: Send + Sync`:
- `execute_command(command, cwd: &Path, env, shell, timeout_ms, max_output_bytes, stdin_input) -> Result<ExecResult, VshError>`
- `execute_git(args, cwd: &Path, env, timeout_ms, max_output_bytes) -> Result<ExecResult, VshError>`

Matches `executor.rs` function signatures exactly.

**Step 3: Create LocalShellBackend**

Create `simse-vsh/src/local_backend.rs`:
- Struct `LocalShellBackend` (unit struct, stateless)
- `impl ShellBackend` delegates to `executor::execute_command()` and `executor::execute_git()`

**Step 4: Register modules, run tests, commit**

Add modules to lib.rs. Run `cd simse-vsh && cargo test`. Commit:
```
feat(simse-vsh): add ShellBackend trait and LocalShellBackend implementation
```

---

### Task 4: VSH Shell Refactor to Use ShellBackend

**Files:**
- Modify: `simse-vsh/src/shell.rs`
- Modify: `simse-vsh/src/server.rs`

**Step 1: Update VirtualShell to accept `Box<dyn ShellBackend>`**

In `simse-vsh/src/shell.rs`:
- Add `backend: Box<dyn ShellBackend>` field to `VirtualShell`
- Update `VirtualShell::new()` to accept `backend` parameter
- In `exec_in_session()`, `exec_git_in_session()`, `exec_raw()`: replace direct `executor::*` calls with `self.backend.execute_command()` / `self.backend.execute_git()`

**Step 2: Update server.rs**

In `initialize` handler: `VirtualShell::new(sandbox, shell, Box::new(LocalShellBackend::new()))`

**Step 3: Run tests, commit**

Run: `cd simse-vsh && cargo test`. All 14 tests pass. Commit:
```
refactor(simse-vsh): route execution through ShellBackend trait
```

---

### Task 5: VNet NetBackend Trait + LocalBackend

**Files:**
- Create: `simse-vnet/src/backend.rs`
- Create: `simse-vnet/src/local_backend.rs`
- Modify: `simse-vnet/src/lib.rs`
- Modify: `simse-vnet/Cargo.toml`

**Step 1: Add async-trait**

Add `async-trait = "0.1"` to `simse-vnet/Cargo.toml`.

**Step 2: Create NetBackend trait**

Create `simse-vnet/src/backend.rs` with `#[async_trait]` trait `NetBackend: Send + Sync`:
- `http_request(url, method, headers, body, timeout_ms, max_response_bytes) -> Result<HttpResponseResult, VnetError>`
- `ws_connect(url, headers) -> Result<String, VnetError>`
- `ws_send(session_id, data) -> Result<(), VnetError>`
- `ws_close(session_id) -> Result<(), VnetError>`
- `tcp_connect(host, port) -> Result<String, VnetError>`
- `tcp_send(session_id, data) -> Result<(), VnetError>`
- `tcp_close(session_id) -> Result<(), VnetError>`
- `udp_send(host, port, data, timeout_ms) -> Result<Option<String>, VnetError>`
- `resolve(hostname) -> Result<Vec<String>, VnetError>`

**Step 3: Create LocalNetBackend**

Create `simse-vnet/src/local_backend.rs`:
- Uses `reqwest::Client` for HTTP requests
- Uses `tokio::net::lookup_host` for DNS resolution
- WS/TCP/UDP: return "not yet implemented" errors (existing mock:// scheme covers these)
- HTTP implementation: build request, send, read response, enforce timeout and max_response_bytes

**Step 4: Register modules, run tests, commit**

Run: `cd simse-vnet && cargo test`. Commit:
```
feat(simse-vnet): add NetBackend trait and LocalNetBackend with real HTTP
```

---

### Task 6: VNet Network/Server Refactor to Use NetBackend

**Files:**
- Modify: `simse-vnet/src/network.rs`
- Modify: `simse-vnet/src/server.rs`

**Step 1: Update VirtualNetwork**

Add `backend: Option<Box<dyn NetBackend>>` field. In `initialize()`, accept optional backend. For `net://` scheme HTTP requests, delegate to `self.backend.http_request()`. Mock store still takes priority for `mock://` scheme.

**Step 2: Update server.rs**

Create `LocalNetBackend` in `initialize` handler, pass to `VirtualNetwork`.

**Step 3: Run tests, commit**

Run: `cd simse-vnet && cargo test`. Commit:
```
refactor(simse-vnet): route net:// operations through NetBackend trait
```

---

## Phase 2: simse-sandbox Crate

### Task 7: Scaffold simse-sandbox Crate

**Files:**
- Create: `simse-sandbox/Cargo.toml`
- Create: `simse-sandbox/src/lib.rs`
- Create: `simse-sandbox/src/main.rs`
- Create: `simse-sandbox/src/error.rs`
- Create: `simse-sandbox/src/transport.rs`
- Create: `simse-sandbox/src/protocol.rs`
- Create: `simse-sandbox/src/config.rs`

**Step 1: Create Cargo.toml**

Dependencies: simse-vfs, simse-vsh, simse-vnet (path deps), serde, serde_json, thiserror 2.0, async-trait, tokio (rt-multi-thread, macros, io-util, time), tracing, tracing-subscriber, russh 0.48, russh-sftp 2.0, russh-keys 0.48. Binary: `simse-sandbox-engine`.

Check latest russh versions at crates.io before implementing.

**Step 2: Create error.rs**

`SandboxError` enum with variants: `NotInitialized`, `SshConnection(String)`, `SshAuth(String)`, `SshChannel(String)`, `BackendSwitch(String)`, `Timeout(String)`, `InvalidParams(String)`, `Vfs(#[from] VfsError)`, `Vsh(#[from] VshError)`, `Vnet(#[from] VnetError)`, `Io(#[from] io::Error)`, `Json(#[from] serde_json::Error)`.

Each variant has a `SANDBOX_*` code. Impl `.code()` and `.to_json_rpc_error()` with `sandboxCode` key. Pass-through variants delegate to the inner error's code.

**Step 3: Create transport.rs**

Copy NdjsonTransport from simse-vsh/src/transport.rs (identical pattern).

**Step 4: Create protocol.rs**

JSON-RPC types: `JsonRpcRequest`, `InitializeParams` (backend, vfs, vsh, vnet config), `BackendParams` (type + ssh config), `SshParams` (host, port, username, auth, max_channels, keepalive_interval_ms), `SshAuthParams` (type + key/password/agent fields), `SwitchBackendParams`.

**Step 5: Create config.rs**

Parsed config types: `BackendConfig` enum (Local, Ssh), `SshConfig` struct, `SshAuth` enum (Key, Password, Agent). Constructors `from_params()` to convert protocol types to config types with validation.

**Step 6: Create lib.rs and minimal main.rs**

lib.rs: `pub mod error, protocol, transport, config`. main.rs: `#[tokio::main]` with tracing init.

**Step 7: Verify compilation, commit**

Run: `cd simse-sandbox && cargo check`. Commit:
```
feat(simse-sandbox): scaffold crate with error, protocol, transport, config
```

---

### Task 8: SSH Pool (russh Connection + Channel Management)

**Files:**
- Create: `simse-sandbox/src/ssh/mod.rs`
- Create: `simse-sandbox/src/ssh/pool.rs`
- Create: `simse-sandbox/src/ssh/channel.rs`
- Modify: `simse-sandbox/src/lib.rs`

**Step 1: Create SSH module**

`ssh/mod.rs`: expose `pool`, `channel`, `fs_backend`, `shell_backend`, `net_backend` submodules.

**Step 2: Create SshPool**

`ssh/pool.rs`:
- `SshPool` struct: `handle: Arc<Mutex<Handle<SshHandler>>>`, config, `health: Arc<AtomicBool>`
- `SshHandler` struct implementing `russh::client::Handler` (set health=false on disconnect)
- `connect(config: &SshConfig)` — creates russh client config, connects, authenticates (key/password/agent)
- `get_exec_channel()` — opens session channel for command execution
- `get_sftp_session()` — opens SFTP subsystem channel
- `get_direct_tcpip(host, port)` — opens direct-tcpip forwarding channel
- `disconnect()` — closes handle
- `is_healthy()` — reads AtomicBool

**Step 3: Create channel helpers**

`ssh/channel.rs`:
- `read_channel_output(channel, timeout_ms, max_bytes)` — reads stdout+stderr with timeout
- SFTP session wrapper for common operations
- Direct-tcpip read/write helpers

**Step 4: Add module to lib.rs, verify, commit**

Add `pub mod ssh;`. Run: `cd simse-sandbox && cargo check`. Commit:
```
feat(simse-sandbox): add SSH pool with russh multiplexed connections
```

---

### Task 9: SSH FsBackend Implementation

**Files:**
- Create: `simse-sandbox/src/ssh/fs_backend.rs`

**Step 1: Implement SshFsBackend**

Struct holds `pool: Arc<SshPool>` and `root: String`.

`impl FsBackend for SshFsBackend`:
- **SFTP-based methods**: read_file, write_file, append_file, delete_file, mkdir, readdir, rmdir, stat, exists, rename, copy
- **Exec-based methods** (no SFTP equivalent): glob (remote `find`), search (remote `grep`), tree (remote `tree`), du (remote `du -sb`)
- **Local computation**: diff (fetch both files via SFTP, diff in memory using simse-vfs's diff module)
- **Unsupported over SSH**: history, diff_versions, checkout — return `VfsError::InvalidOperation("Not supported over SSH")`

**Step 2: Verify, commit**

Run: `cd simse-sandbox && cargo check`. Commit:
```
feat(simse-sandbox): add SshFsBackend using SFTP
```

---

### Task 10: SSH ShellBackend Implementation

**Files:**
- Create: `simse-sandbox/src/ssh/shell_backend.rs`

**Step 1: Implement SshShellBackend**

Struct holds `pool: Arc<SshPool>`.

`impl ShellBackend for SshShellBackend`:
- `execute_command`: opens exec channel, builds command with `cd $cwd && export K=V && $shell -c '$command'`, reads output with timeout, truncates at max_output_bytes
- `execute_git`: delegates to execute_command with `git $args` as the command
- Shell escaping helper: `shell_escape(s)` wraps in single quotes, escapes inner single quotes

**Step 2: Verify, commit**

Run: `cd simse-sandbox && cargo check`. Commit:
```
feat(simse-sandbox): add SshShellBackend for remote command execution
```

---

### Task 11: SSH NetBackend Implementation

**Files:**
- Create: `simse-sandbox/src/ssh/net_backend.rs`

**Step 1: Implement SshNetBackend**

Struct holds `pool: Arc<SshPool>`.

`impl NetBackend for SshNetBackend`:
- `http_request`: exec channel with `curl` (reliable, handles TLS, redirects). Parse curl output for status, headers, body. Alternative: direct-tcpip for plain HTTP.
- `resolve`: exec channel with `getent hosts $hostname` or `dig +short $hostname`
- `tcp_connect`: direct-tcpip channel, store as session
- `tcp_send/close`: read/write/close the direct-tcpip channel
- `ws_connect/send/close`: direct-tcpip channel + WebSocket frame encoding
- `udp_send`: exec channel with `socat` or `nc -u`

For initial implementation, HTTP and resolve are the priority. WS/TCP/UDP can start with todo!() stubs.

**Step 2: Verify, commit**

Run: `cd simse-sandbox && cargo check`. Commit:
```
feat(simse-sandbox): add SshNetBackend for remote network operations
```

---

## Phase 3: Sandbox Orchestrator + Server

### Task 12: Sandbox Orchestrator

**Files:**
- Create: `simse-sandbox/src/sandbox.rs`
- Modify: `simse-sandbox/src/lib.rs`

**Step 1: Create Sandbox struct**

Fields: `backend_config`, `ssh_pool: Option<Arc<SshPool>>`, `fs_backend`, `shell_backend`, `net_backend` (all `Option<Box<dyn Trait>>`), engine state (`vfs: Option<VirtualFs>`, `vsh: Option<VirtualShell>`, `vnet: Option<VirtualNetwork>`), `initialized: bool`.

Methods:
- `new()` — uninitialized
- `initialize(backend_config, vfs_params, vsh_params, vnet_params)` — connect SSH if needed, create backends, init engines
- `switch_backend(new_config)` — disconnect old SSH, create new backends, preserve engine state
- `health()` — report backend type, SSH health, engine status
- `dispose()` — disconnect SSH, clear state

For Local backend: use LocalFsBackend, LocalShellBackend, LocalNetBackend.
For SSH backend: create SshPool, then SshFsBackend, SshShellBackend, SshNetBackend.

**Step 2: Add module, verify, commit**

Add `pub mod sandbox;`. Run: `cd simse-sandbox && cargo check`. Commit:
```
feat(simse-sandbox): add Sandbox orchestrator with backend switching
```

---

### Task 13: Sandbox JSON-RPC Server

**Files:**
- Create: `simse-sandbox/src/server.rs`
- Modify: `simse-sandbox/src/main.rs`

**Step 1: Create SandboxServer**

Struct holds `transport: NdjsonTransport` and `sandbox: Sandbox`.

`run()` method: read stdin lines, parse JSON-RPC, dispatch, write response/error.

`dispatch(method, params)` routes:
- `sandbox/initialize` — parse InitializeParams, call sandbox.initialize()
- `sandbox/dispose` — call sandbox.dispose()
- `sandbox/health` — call sandbox.health()
- `sandbox/switchBackend` — parse SwitchBackendParams, call sandbox.switch_backend()
- `sandbox/vfs/*` — strip prefix, delegate to VFS handler methods
- `sandbox/vsh/*` — strip prefix, delegate to VSH handler methods
- `sandbox/vnet/*` — strip prefix, delegate to VNet handler methods

VFS/VSH/VNet handlers re-implement the dispatch from each engine's server.rs but route through the sandbox's backend instances. They parse the same param types and return the same result types.

**Step 2: Update main.rs**

Create NdjsonTransport, SandboxServer, call `server.run().await`.

**Step 3: Verify, commit**

Run: `cd simse-sandbox && cargo check`. Commit:
```
feat(simse-sandbox): add JSON-RPC server with VFS/VSH/VNet passthrough
```

---

## Phase 4: Testing

### Task 14: Integration Tests (Local Backend)

**Files:**
- Create: `simse-sandbox/tests/integration.rs`

**Step 1: Write 10 integration tests**

Test pattern: spawn simse-sandbox-engine as child process, send JSON-RPC requests via stdin, read responses from stdout.

Tests:
1. `test_initialize_local` — initialize with local backend, verify health response
2. `test_vfs_write_read` — sandbox/vfs/writeFile then sandbox/vfs/readFile
3. `test_vfs_mkdir_readdir` — sandbox/vfs/mkdir then sandbox/vfs/readdir
4. `test_vsh_session_exec` — create session, exec/run, verify output
5. `test_vsh_exec_raw` — exec/runRaw through sandbox
6. `test_vnet_mock_register_request` — register mock, send mock:// HTTP request
7. `test_vnet_resolve_localhost` — resolve "localhost"
8. `test_switch_backend` — switch local→local, verify operations still work
9. `test_dispose` — initialize then dispose, verify health shows uninitialized
10. `test_unknown_method` — send unknown method, verify error response

**Step 2: Run tests, commit**

Run: `cd simse-sandbox && cargo test`. Commit:
```
test(simse-sandbox): add 10 integration tests for local backend
```

---

### Task 15: SSH Integration Tests (Feature-Gated)

**Files:**
- Modify: `simse-sandbox/Cargo.toml`
- Create: `simse-sandbox/tests/ssh_integration.rs`

**Step 1: Add feature flag**

Add `[features] ssh-test = []` to Cargo.toml.

**Step 2: Write gated SSH tests**

All tests wrapped in `#![cfg(feature = "ssh-test")]`. Require running SSH server (Docker in CI).

Tests:
1. `test_ssh_pool_connect` — connect, verify healthy
2. `test_ssh_fs_read_write` — write file remotely, read back
3. `test_ssh_shell_exec` — execute command remotely
4. `test_ssh_net_resolve` — DNS resolve remotely
5. `test_switch_local_to_ssh` — start local, switch to SSH, verify remote exec

**Step 3: Run local tests (SSH skipped), commit**

Run: `cd simse-sandbox && cargo test`. Commit:
```
test(simse-sandbox): add SSH integration tests (feature-gated)
```

---

### Task 16: Update CLAUDE.md and Build Scripts

**Files:**
- Modify: `CLAUDE.md`
- Modify: `package.json`

**Step 1: Update CLAUDE.md**

Add to Commands section:
- `bun run build:sandbox-engine  # cd simse-sandbox && cargo build --release`
- `cd simse-sandbox && cargo test  # Rust sandbox tests`

Add to Repository Layout:
- `simse-sandbox/  # Pure Rust crate — unified sandbox engine (JSON-RPC over stdio)`

Add simse-sandbox module layout to architecture docs.

**Step 2: Add build script**

Add `"build:sandbox-engine": "cd simse-sandbox && cargo build --release"` to package.json scripts.

**Step 3: Commit**

```
docs: add simse-sandbox to CLAUDE.md and build scripts
```

---

## Task Dependencies

```
Tasks 1-2 (VFS) ──┐
Tasks 3-4 (VSH) ──┼── Task 7 (scaffold) ── Task 8 (SSH pool) ──┬── Task 9 (SSH FS)
Tasks 5-6 (VNet) ─┘                                            ├── Task 10 (SSH Shell)
                                                                └── Task 11 (SSH Net)
                                                                         │
                                                                Task 12 (orchestrator)
                                                                         │
                                                                Task 13 (server)
                                                                         │
                                                                Task 14 (local tests)
                                                                         │
                                                                Task 15 (SSH tests)
                                                                         │
                                                                Task 16 (docs)
```

**Parallelizable:** Tasks 1-2, 3-4, 5-6 can run in parallel. Tasks 9, 10, 11 can run in parallel.
