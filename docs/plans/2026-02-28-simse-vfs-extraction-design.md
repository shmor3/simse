# simse-vfs Extraction Design

**Date:** 2026-02-28

## Goal

Extract the VFS (Virtual Filesystem) system from `src/ai/vfs/` into a standalone `simse-vfs/` package with its own error layer and logger interface — zero imports from simse.

## Current State

6 source files in `src/ai/vfs/` (~1.5K LOC), 3 test files, 1 error module. Consumed by builtin-tools, mcp-server, and tools/types.

## Architecture

Same pattern as simse-vector extraction:

```
simse-vfs/
  package.json
  tsconfig.json
  src/
    lib.ts              <- barrel exports
    errors.ts           <- self-contained VFSError factory + guard
    logger.ts           <- Logger interface + createNoopLogger
    types.ts            <- all VFS types (VirtualFS, VFSDisk, VFSStat, etc.)
    path-utils.ts       <- path normalization, validation, vfs:// scheme
    validators.ts       <- file content validators (JSON, whitespace, etc.)
    vfs.ts              <- createVirtualFS (in-memory)
    vfs-disk.ts         <- createVFSDisk (disk-backed)
  tests/
    vfs.test.ts
    vfs-disk.test.ts
    e2e-vfs-tool.test.ts
```

### Dependencies

- Zero external npm deps (only node:fs, node:path)
- Own Logger interface + createNoopLogger
- Own VFSError factory + isVFSError guard

### Integration

- simse depends on simse-vfs via workspace protocol
- `src/lib.ts` re-exports `* from 'simse-vfs'`
- `src/errors/vfs.ts` re-exports from simse-vfs
- Consumer files import VirtualFS from 'simse-vfs'

### Dependency direction

```
simse-vfs  <-  simse  (never the reverse)
```

## API Impact

No breaking changes. simse re-exports everything from simse-vfs.
