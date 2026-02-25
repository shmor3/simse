# Headless Agent Server & Coding Agent Tools Design

## Overview

Transform simse into a full coding agent framework with a headless HTTP+SSE server, host filesystem/bash/git tools, two-phase context management, batch tool execution, plugin hooks, and provider-specific prompts. Inspired by opencode's split architecture.

## 1. Headless Agent Server

### Architecture

```
simse-server (Hono HTTP + SSE)
  POST /sessions              → create session
  POST /sessions/:id/prompt   → send user message
  GET  /sessions/:id/events   → SSE stream
  GET  /sessions/:id          → get session state
  DELETE /sessions/:id        → close session
  POST /sessions/:id/abort    → cancel current run
  POST /sessions/:id/feedback → tool permission decision
  GET  /tools                 → list available tools
  GET  /agents                → list ACP agents
  GET  /health                → server status
```

### Factory

```typescript
export interface SimseServerConfig {
  readonly port?: number;
  readonly host?: string;
  readonly acpServers: readonly ACPServerEntry[];
  readonly mcpServers?: readonly MCPServerEntry[];
  readonly workingDirectory: string;
  readonly toolPermissions?: ToolPermissionConfig;
  readonly hooks?: HookRegistration[];
  readonly memory?: MemoryConfig;
  readonly providerPrompts?: ProviderPromptConfig;
  readonly instructionDiscovery?: InstructionDiscoveryOptions;
}

export function createSimseServer(config: SimseServerConfig): SimseServer;
// returns { start(), stop(), port, url }
```

### Session Management

- Each session holds: conversation, tool state, event subscriptions
- Sessions persist in storage (file or SQLite) for restart survival
- Session lifecycle: created → active → completed/aborted
- Subagent sessions are child sessions with restricted permissions

## 2. Event Bus

### Types

```typescript
export interface EventBus {
  readonly publish: <T extends EventType>(type: T, payload: EventPayload<T>) => void;
  readonly subscribe: <T extends EventType>(type: T, handler: Handler<T>) => () => void;
  readonly subscribeAll: (handler: (type: EventType, payload: unknown) => void) => () => void;
}

export type EventType =
  | 'session.created'
  | 'session.prompt'
  | 'session.completed'
  | 'session.error'
  | 'stream.delta'
  | 'stream.complete'
  | 'tool.call.start'
  | 'tool.call.end'
  | 'tool.call.error'
  | 'turn.complete'
  | 'compaction.start'
  | 'compaction.complete'
  | 'permission.request'
  | 'permission.response'
  | 'abort';
```

### Integration

- Agentic loop publishes events (replaces internal callback calls)
- LoopCallbacks stays for backward compat — internally subscribes to EventBus
- SSE endpoint subscribes to EventBus and serializes events to clients
- Multiple clients can subscribe to the same session

## 3. Host Tools

### 3a. Filesystem Tools

```typescript
export function registerFilesystemTools(
  registry: ToolRegistry,
  options: FilesystemToolOptions,
): void;
```

| Tool | Description |
|------|-------------|
| `fs_read` | Read file with line ranges, binary/image detection |
| `fs_write` | Create/overwrite, generates diff for permission |
| `fs_edit` | Exact replacement with 5-strategy fuzzy fallback |
| `fs_glob` | Pattern search, sorted by mtime |
| `fs_grep` | Regex content search via child_process |
| `fs_list` | Directory listing with depth |

**Edit fallback chain:**
1. Exact string match
2. Line-trimmed match (strip leading/trailing whitespace)
3. Whitespace-normalized match
4. Indentation-flexible match (remove common indent)
5. Block-anchor match (first/last line + Levenshtein)

**Sandboxing:** Paths resolved relative to `workingDirectory`. Escape attempts rejected. Configurable `allowedPaths` whitelist.

### 3b. Bash Tool

```typescript
export function registerBashTool(
  registry: ToolRegistry,
  options: BashToolOptions,
): void;
```

| Tool | Description |
|------|-------------|
| `bash` | Shell execution, timeout, abort, env vars, cwd tracking |

- `node:child_process.spawn` with `shell: true`
- Output truncation (default 50KB)
- AbortSignal support
- Working directory persists across calls in session

### 3c. Git Tools

```typescript
export function registerGitTools(
  registry: ToolRegistry,
  options: GitToolOptions,
): void;
```

| Tool | Description |
|------|-------------|
| `git_status` | Working tree status |
| `git_diff` | Staged/unstaged diffs |
| `git_log` | Commit history |
| `git_commit` | Create commit |
| `git_branch` | List/create/switch branches |

Implemented via bash tool under the hood.

### 3d. Tool Permission System

```typescript
export interface ToolPermissionConfig {
  readonly defaultPolicy: 'allow' | 'ask' | 'deny';
  readonly rules: readonly ToolPermissionRule[];
}

export interface ToolPermissionRule {
  readonly tool: string;       // tool name or glob
  readonly pattern?: string;   // for bash: command glob
  readonly policy: 'allow' | 'ask' | 'deny';
}
```

- `ask` emits `permission.request` event → client prompts user → sends response
- Glob matching for bash commands (e.g., `git status *` → allow)
- Last matching rule wins

## 4. Two-Phase Context Management

### Phase 1: Tool Output Pruning (no LLM needed)

- Walk conversation backward, skip 2 most recent turns
- Stop at first summary message
- Replace old tool outputs with `[OUTPUT PRUNED — {n} chars]`
- Preserve tool name, arguments, timing
- Configurable thresholds:
  - `pruneMinimumTokens: 20_000`
  - `pruneProtectTokens: 40_000`
  - `pruneProtectedTools: string[]`

### Phase 2: Full LLM Compaction

- Triggered when Phase 1 insufficient
- Dedicated compaction prompt generates structured summary
- Summary message marked for future phase boundaries
- Old messages excluded from future prompts, kept in storage

### Integration

```typescript
export interface ConversationOptions {
  readonly autoCompactChars?: number;        // existing
  readonly pruneMinimumTokens?: number;      // new
  readonly pruneProtectTokens?: number;      // new
  readonly pruneProtectedTools?: string[];   // new
}
```

Phase 1 runs every turn automatically. Phase 2 only when over limit.

## 5. Batch Tool Execution

```typescript
// On ToolRegistry
readonly batchExecute: (
  calls: readonly ToolCallRequest[],
  options?: { maxConcurrency?: number },
) => Promise<readonly ToolCallResult[]>;
```

- `Promise.allSettled` with configurable concurrency (default 8)
- Each call goes through permission individually
- Results returned in input order
- Errors isolated per call
- Agentic loop uses `batchExecute` when multiple tool calls in one response

## 6. Plugin Hook System

```typescript
export interface HookSystem {
  readonly register: <T extends HookType>(
    type: T,
    handler: HookHandler<T>,
  ) => () => void;
}

export type HookType =
  | 'tool.execute.before'
  | 'tool.execute.after'
  | 'prompt.system.transform'
  | 'prompt.messages.transform'
  | 'session.compacting'
  | 'tool.result.validate';
```

### Key Hooks

- `tool.execute.before`: Mutate/block tool calls. Returns modified request or `{ blocked, reason }`.
- `tool.execute.after`: Inspect/transform results. Returns modified result.
- `tool.result.validate`: Post-execution validation. Returns messages injected into next turn. For LSP diagnostics, test results, linter output.
- `prompt.system.transform`: Mutate system prompt per turn. For instruction injection, dynamic context.
- `prompt.messages.transform`: Mutate message history before sending.
- `session.compacting`: Customize compaction behavior.

## 7. Provider Prompts & Instruction Discovery

### Provider-Specific Prompts

```typescript
export interface ProviderPromptConfig {
  readonly prompts?: Record<string, string>; // pattern → prompt content
  readonly defaultPrompt?: string;
}
```

Match patterns: `anthropic/*` → anthropic prompt, `openai/*` → openai prompt.

### Instruction File Discovery

```typescript
export interface InstructionDiscoveryOptions {
  readonly patterns?: string[];  // default: ['CLAUDE.md', 'AGENTS.md', '.simse/instructions.md']
  readonly rootDir: string;
  readonly autoInject?: boolean;
}
```

Walk from working directory to root, collect instruction files, inject into system prompt.

## Non-Goals

- Custom TUI/desktop client (consumers build their own)
- SQLite storage (use file-based storage for now)
- LSP integration (can be added via hooks later)
- AST parsing (out of scope — tools work on text)

## Dependencies

- `hono` — HTTP server framework (new external dep)
- Existing: `@modelcontextprotocol/sdk`, `@huggingface/transformers`
