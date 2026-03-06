# Merge simse-vfs, simse-vsh, simse-vnet into simse-sandbox — Design

**Date:** 2026-03-06

## Problem

simse-vfs, simse-vsh, and simse-vnet are three standalone crates that exist solely to serve simse-sandbox. Each has its own binary, server, transport, protocol, and error types — all of which simse-sandbox duplicates in its unified 64-method JSON-RPC server. This creates unnecessary dependency graph complexity, build overhead, and cross-crate friction.

## Decision

Merge all three crates into simse-sandbox. Delete the standalone binaries, unify error handling into a single `SandboxError` enum, replace trait-based backends with enum dispatch, and convert tests to direct Rust API tests.

## File Layout After Merge

```
simse-sandbox/src/
  lib.rs                    # Module declarations
  main.rs                   # Binary entry point (simse-sandbox-engine)
  error.rs                  # Unified SandboxError enum (~30 variants)
  config.rs                 # BackendConfig, SshConfig, SshAuth (unchanged)
  protocol.rs               # JSON-RPC param/result types (unchanged)
  transport.rs              # NdjsonTransport (unchanged)
  server.rs                 # JSON-RPC dispatcher, 64 methods (unchanged)
  sandbox.rs                # Sandbox orchestrator (adapted for enum dispatch)

  # VFS domain (from simse-vfs)
  vfs_store.rs              # VirtualFs: in-memory filesystem
  vfs_disk.rs               # DiskFs: real filesystem with history
  vfs_diff.rs               # Myers diff algorithm
  vfs_glob.rs               # Glob pattern matching
  vfs_search.rs             # Text search
  vfs_path.rs               # Path utilities, VfsLimits
  vfs_backend.rs            # FsImpl enum (Local/Ssh), replaces FsBackend trait

  # VSH domain (from simse-vsh)
  vsh_shell.rs              # VirtualShell: session management
  vsh_executor.rs           # Command execution via tokio::process
  vsh_sandbox.rs            # SandboxConfig: path/command validation
  vsh_backend.rs            # ShellImpl enum (Local/Ssh), replaces ShellBackend trait

  # VNet domain (from simse-vnet)
  vnet_network.rs           # VirtualNetwork: core logic
  vnet_sandbox.rs           # NetSandboxConfig: host/port validation
  vnet_mock_store.rs        # MockStore: mock registry
  vnet_session.rs           # SessionManager: WS/TCP tracking
  vnet_backend.rs           # NetImpl enum (Local/Ssh), replaces NetBackend trait

  # SSH implementations
  ssh/
    mod.rs
    pool.rs                 # SshPool (unchanged)
    channel.rs              # ExecOutput (unchanged)
    fs.rs                   # SSH filesystem ops
    shell.rs                # SSH shell ops
    net.rs                  # SSH network ops

tests/
  integration.rs            # Sandbox JSON-RPC tests (existing, unchanged)
  ssh_integration.rs        # SSH tests (existing, unchanged)
  vfs.rs                    # Direct Rust API tests for VirtualFs + DiskFs
  vsh.rs                    # Direct Rust API tests for VirtualShell
  vnet.rs                   # Direct Rust API tests for VirtualNetwork
```

## Unified Error Type

Single `SandboxError` enum with domain-prefixed variants:

- **Lifecycle:** `NotInitialized`, `AlreadyInitialized`
- **VFS:** `VfsInvalidPath`, `VfsNotFound`, `VfsAlreadyExists`, `VfsNotAFile`, `VfsNotADirectory`, `VfsNotEmpty`, `VfsLimitExceeded`, `VfsInvalidOperation`, `VfsPermissionDenied`, `VfsDiskNotConfigured`
- **VSH:** `VshSessionNotFound`, `VshExecutionFailed`, `VshTimeout`, `VshSandboxViolation`, `VshLimitExceeded`
- **VNet:** `VnetSandboxViolation`, `VnetConnectionFailed`, `VnetTimeout`, `VnetSessionNotFound`, `VnetMockNotFound`, `VnetNoMockMatch`, `VnetLimitExceeded`, `VnetResponseTooLarge`, `VnetDnsResolutionFailed`
- **SSH:** `SshConnection`, `SshAuth`, `SshChannel`
- **Backend:** `BackendSwitch`, `InvalidParams`
- **Generic:** `Io`, `Json`

Each variant maps to a `SANDBOX_*` code string and JSON-RPC error code `-32000`.

## Backend Enum Dispatch

Replace `Box<dyn FsBackend>` / `Box<dyn ShellBackend>` / `Box<dyn NetBackend>` with concrete enums:

```rust
pub enum FsImpl {
    Local(DiskFs),
    Ssh(SshFs),
}

pub enum ShellImpl {
    Local(LocalShell),
    Ssh(SshShell),
}

pub enum NetImpl {
    Local(LocalNet),
    Ssh(SshNet),
}
```

Each enum implements the same methods as the old trait, dispatching via `match`. This removes the `async_trait` dependency for backend dispatch and eliminates vtable overhead.

## Test Strategy

- **Delete** all 68 sub-crate integration tests (they spawned standalone binaries)
- **Create** direct Rust API tests in `tests/vfs.rs`, `tests/vsh.rs`, `tests/vnet.rs` covering the same functionality
- **Keep** existing `tests/integration.rs` and `tests/ssh_integration.rs` unchanged

## What Gets Deleted

- **Directories:** `simse-vfs/`, `simse-vsh/`, `simse-vnet/` (entirely)
- **From merged code:** `server.rs`, `transport.rs`, `main.rs`, `protocol.rs`, `lib.rs`, `backend.rs`, `local_backend.rs` from each sub-crate
- **Cargo.toml deps:** `simse-vfs-engine`, `simse-vsh-engine`, `simse-vnet-engine`
- **Workspace Cargo.toml:** remove three entries from `exclude` list

## Dependencies Added to simse-sandbox

From simse-vfs: `regex`, `sha2`, `base64` (base64 already present)
From simse-vsh: `uuid` (for session IDs)
From simse-vnet: `reqwest` (for HTTP), `uuid`, `regex`
