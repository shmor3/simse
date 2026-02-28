# simse-vfs Improvements + Command Passthrough Design

**Date:** 2026-02-28

## Goal

Make simse-vfs a complete agent sandbox: add a pluggable command execution interface (for future Firecracker integration), improve search/glob, add atomic transactions, and add file change events.

## Context

The VFS acts as a safety layer between the agent and the real filesystem. Commands will eventually run inside Firecracker VMs with the VFS as their filesystem. For now, we define the execution interface and types without a concrete backend.

## Changes

### 1. Command Execution Interface (exec.ts)

Pluggable backend pattern:

```typescript
interface ExecResult {
    stdout: string;
    stderr: string;
    exitCode: number;
    filesChanged: string[];
}

interface ExecOptions {
    cwd?: string;
    env?: Record<string, string>;
    timeout?: number;
    stdin?: string;
}

interface ExecBackend {
    run(command: string, args: string[], vfs: VirtualFS, options?: ExecOptions): Promise<ExecResult>;
    dispose(): Promise<void>;
}

interface VFSExecutor {
    run(command: string, args: string[], options?: ExecOptions): Promise<ExecResult>;
    dispose(): Promise<void>;
}

function createVFSExecutor(vfs: VirtualFS, backend: ExecBackend): VFSExecutor;
```

No concrete backend implementation yet. Just the interface, types, and factory.

### 2. Search Enhancements

Add to existing `search()` method:
- Regex mode (new `mode` field: `'substring' | 'regex'`)
- Context lines (`contextBefore`, `contextAfter` in options)
- Count-only mode (`countOnly: true` returns match count without results)

### 3. Glob Enhancements

Add to existing `glob()` method:
- Negation patterns (`!node_modules/**`)
- Brace expansion (`*.{ts,tsx}`)

### 4. Atomic Transactions

New method on VirtualFS:

```typescript
type VFSOp =
    | { type: 'writeFile'; path: string; content: string | Uint8Array }
    | { type: 'deleteFile'; path: string }
    | { type: 'mkdir'; path: string }
    | { type: 'rmdir'; path: string }
    | { type: 'rename'; oldPath: string; newPath: string }
    | { type: 'copy'; src: string; dest: string };

transaction(ops: readonly VFSOp[]): void;
```

Takes a snapshot before executing, rolls back on any failure.

### 5. File Change Events

New callback options on VirtualFS:

```typescript
interface VFSCallbacks {
    onWrite?: (path: string, size: number) => void;
    onDelete?: (path: string) => void;
    onRename?: (oldPath: string, newPath: string) => void;
    onMkdir?: (path: string) => void;
}
```

Passed via VirtualFSOptions. Fired synchronously after each successful operation.

## Non-Goals

- No concrete ExecBackend implementation (Firecracker comes later)
- No tool registration changes (VFS is infrastructure, not a tool)
- No symlinks or file permissions
