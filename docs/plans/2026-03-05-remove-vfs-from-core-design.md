# Remove VFS from simse-core — Design

**Date:** 2026-03-05

## Problem

simse-core has an in-process VFS layer (`src/vfs/`) that wraps `simse-vfs-engine` directly. This bypasses simse-sandbox, which is the designated owner of all VFS, VSH, and VNet operations. Only simse-sandbox should touch simse-vfs, simse-vsh, and simse-vnet.

## Decision

Remove all VFS code from simse-core. simse-core must not depend on `simse-vfs-engine` at all. The sandbox engine is the single entry point for filesystem, shell, and network operations.

## Scope

### Remove from simse-core

1. **`src/vfs/` module** (5 files): `mod.rs`, `vfs.rs`, `disk.rs`, `exec.rs`, `validators.rs`
2. **`VfsStore` trait + `register_vfs_tools()`** in `tools/builtin.rs`
3. **`CoreContext.vfs`** field + `with_vfs()` builder in `context.rs`
4. **`VfsConfig`** from `config.rs` + `AppConfig.vfs` field
5. **`VfsEngine` error variant** + VFS error helpers in `error.rs`
6. **`simse-vfs-engine` dependency** from `Cargo.toml`
7. **Tests**: `tests/vfs.rs` (delete), VFS-related tests in `tests/integration.rs`, `tests/builtin_tools.rs`, `tests/error.rs`, `tests/tool_registry.rs`

### Update docs

8. **CLAUDE.md**: Remove `simse-bridge` from layout (doesn't exist), update simse-core module layout to remove `vfs/`

## Architecture After

```
simse-sandbox ──owns──> simse-vfs, simse-vsh, simse-vnet
simse-core ──no VFS dep──> pure orchestration (events, sessions, tools, hooks, chains, loops)
```

simse-core's `ToolRegistry` remains — sandbox-backed tools are registered externally by whatever binary wires things up (e.g., simse-tui).
