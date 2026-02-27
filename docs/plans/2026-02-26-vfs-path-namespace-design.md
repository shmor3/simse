# VFS Path Namespace — Design

**Date**: 2026-02-26
**Status**: Approved

## Problem

When simse connects to an ACP agent (e.g. Claude Code), two tool systems coexist:

1. **simse's VFS tools** (`vfs_read`, `vfs_write`) — operate on an in-memory virtual filesystem
2. **The ACP agent's native tools** (`Read`, `Write`) — operate on the real disk filesystem

VFS tool results return paths like `/hello.js`, which look identical to real filesystem paths. The ACP agent sees these paths in conversation context and attempts to read/write them using its own filesystem tools, causing `"No such file or directory"` errors.

## Solution

Adopt a `vfs://` URI scheme for all VFS paths. Internal storage, API parameters, tool inputs/outputs, and CLI display all use `vfs:///path` format. This makes VFS paths unambiguously distinct from real filesystem paths.

## Path Format

Standard URI format: `vfs://` (scheme + empty authority) + `/path`

| Example | Meaning |
|---------|---------|
| `vfs:///` | VFS root |
| `vfs:///hello.js` | File at root |
| `vfs:///src/components/Button.tsx` | Nested file |

## Input Strictness

`normalizePath()` **rejects** inputs without the `vfs://` prefix. Only `vfs:///path` is accepted. Plain `/path` or `path` throws an error.

## Changes

### `src/ai/vfs/path-utils.ts`

- `VFS_SCHEME = 'vfs://'` constant
- `normalizePath(input)` — rejects non-`vfs://` inputs, normalizes path portion (collapse `//`, strip trailing `/`, resolve `.`/`..`)
- New `toLocalPath(vfsPath)` — strips `vfs://` prefix, returns `/hello.js` for disk operations
- `parentPath()`, `baseName()`, `ancestorPaths()`, `pathDepth()` — operate on path portion after scheme
- `validatePath()` — validates path portion, not scheme

### `src/ai/vfs/vfs.ts`

- All internal `Map` keys use `vfs:///path` format
- All returned paths (`stat().path`, `search().path`, `history()`, `diffVersions().oldPath`/`.newPath`) use `vfs://` prefix
- `readdir()` entries still return just `name` (unchanged — `name` is `"hello.js"`)
- `tree()` output shows `vfs:///` as root label

### `src/ai/vfs/vfs-disk.ts`

- `commit()` calls `toLocalPath()` before joining with disk base directory
- `load()` constructs `vfs://` paths when importing disk files into VFS
- `VFSCommitOperation.path` becomes `vfs:///src/Button.tsx`
- `VFSCommitOperation.diskPath` remains OS-native (unchanged)

### `src/ai/vfs/validators.ts`

- Validators receive `vfs://` paths, pass through to `VFSValidationIssue.path`
- No logic changes needed

### Tool Boundaries

**`src/ai/tools/builtin-tools.ts`** (simse core):
- Tool descriptions updated: `"path (string, required): VFS path using vfs:// scheme (e.g. vfs:///hello.js)"`
- Handlers pass `vfs://` paths directly to VFS APIs
- Result strings: `"Wrote 67 bytes to vfs:///hello.js"`

**`src/ai/mcp/mcp-server.ts`** (MCP tools):
- Same changes as builtin-tools

**`simse-code/tool-registry.ts`** (CLI tools):
- Same changes — tool result strings include `vfs://` prefix

### CLI Display

- `tryRenderInlineDiff()` — paths extracted from tool args already have `vfs://`, passed to VFS APIs
- `onFileWrite` callback — `[vfs] created vfs:///hello.js (67 bytes)`
- Validation display — `WARN vfs:///hello.js: File does not end with newline`
- `vfs.tree()` root label becomes `vfs:///`
- File tracker stores `vfs://` paths internally

### Tests

All VFS test files need path assertion updates (`/path` → `vfs:///path`):
- `tests/vfs.test.ts`
- `tests/e2e-vfs-tool.test.ts`
- `tests/builtin-tools.test.ts`
- `simse-code/tests/tool-registry.test.ts`
