# Headless Agent Server Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform simse into a full coding agent framework with headless HTTP+SSE server, host filesystem/bash/git tools, two-phase context management, batch tool execution, plugin hooks, and provider-specific prompts.

**Architecture:** Split architecture inspired by opencode. An EventBus decouples the agentic loop from consumers. Host tools (filesystem, bash, git) are registered on the ToolRegistry. A Hono HTTP server exposes sessions over REST+SSE. Plugin hooks allow transforming prompts, intercepting tools, and injecting validation. Two-phase context management (prune then compact) keeps conversations within token limits.

**Tech Stack:** TypeScript (strict, ESM-only), Bun runtime, Hono (HTTP server), Bun.spawn/spawnSync (subprocess), existing ToolRegistry/Conversation/AgenticLoop

**Design Reference:** `docs/plans/2026-02-25-agent-server-design.md`

---

### Task 1: Event Bus

**Files:**
- Create: `src/events/event-bus.ts`
- Create: `src/events/types.ts`
- Create: `src/events/index.ts`
- Test: `tests/event-bus.test.ts`

**Step 1: Write the failing test**

```typescript
// tests/event-bus.test.ts
import { describe, expect, it, mock } from 'bun:test';
import { createEventBus } from '../src/events/event-bus.js';
import type { EventBus, EventPayloadMap } from '../src/events/types.js';

describe('EventBus', () => {
  it('delivers events to subscribers', () => {
    const bus = createEventBus();
    const handler = mock(() => {});
    bus.subscribe('tool.call.start', handler);
    bus.publish('tool.call.start', { callId: '1', name: 'test', args: {} });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('unsubscribes correctly', () => {
    const bus = createEventBus();
    const handler = mock(() => {});
    const unsub = bus.subscribe('tool.call.start', handler);
    unsub();
    bus.publish('tool.call.start', { callId: '1', name: 'test', args: {} });
    expect(handler).not.toHaveBeenCalled();
  });

  it('subscribeAll receives all event types', () => {
    const bus = createEventBus();
    const handler = mock(() => {});
    bus.subscribeAll(handler);
    bus.publish('stream.delta', { text: 'hi' });
    bus.publish('abort', { reason: 'user' });
    expect(handler).toHaveBeenCalledTimes(2);
  });

  it('does not throw when publishing with no subscribers', () => {
    const bus = createEventBus();
    expect(() => bus.publish('abort', { reason: 'test' })).not.toThrow();
  });

  it('isolates handler errors from other handlers', () => {
    const bus = createEventBus();
    const bad = mock(() => { throw new Error('boom'); });
    const good = mock(() => {});
    bus.subscribe('abort', bad);
    bus.subscribe('abort', good);
    bus.publish('abort', { reason: 'test' });
    expect(good).toHaveBeenCalledTimes(1);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/event-bus.test.ts`
Expected: FAIL — modules do not exist

**Step 3: Write types**

```typescript
// src/events/types.ts
export interface EventPayloadMap {
  'session.created': { readonly sessionId: string };
  'session.prompt': { readonly sessionId: string; readonly prompt: string };
  'session.completed': { readonly sessionId: string; readonly result: unknown };
  'session.error': { readonly sessionId: string; readonly error: Error };
  'stream.delta': { readonly text: string };
  'stream.complete': { readonly text: string };
  'tool.call.start': { readonly callId: string; readonly name: string; readonly args: Record<string, unknown> };
  'tool.call.end': { readonly callId: string; readonly name: string; readonly output: string; readonly isError: boolean; readonly durationMs: number };
  'tool.call.error': { readonly callId: string; readonly name: string; readonly error: Error };
  'turn.complete': { readonly turn: number; readonly type: 'text' | 'tool_use' };
  'compaction.start': { readonly messageCount: number; readonly estimatedChars: number };
  'compaction.complete': { readonly summaryLength: number };
  'permission.request': { readonly callId: string; readonly toolName: string; readonly args: Record<string, unknown> };
  'permission.response': { readonly callId: string; readonly allowed: boolean };
  'abort': { readonly reason: string };
}

export type EventType = keyof EventPayloadMap;
export type EventPayload<T extends EventType> = EventPayloadMap[T];

export type EventHandler<T extends EventType> = (payload: EventPayload<T>) => void;

export interface EventBus {
  readonly publish: <T extends EventType>(type: T, payload: EventPayload<T>) => void;
  readonly subscribe: <T extends EventType>(type: T, handler: EventHandler<T>) => () => void;
  readonly subscribeAll: (handler: (type: EventType, payload: unknown) => void) => () => void;
}
```

**Step 4: Write implementation**

```typescript
// src/events/event-bus.ts
import type { EventBus, EventHandler, EventPayloadMap, EventType } from './types.js';

export function createEventBus(): EventBus {
  const handlers = new Map<EventType, Set<EventHandler<never>>>();
  const globalHandlers = new Set<(type: EventType, payload: unknown) => void>();

  const publish = <T extends EventType>(type: T, payload: EventPayloadMap[T]): void => {
    const typeHandlers = handlers.get(type);
    if (typeHandlers) {
      for (const handler of typeHandlers) {
        try { handler(payload); } catch { /* isolate errors */ }
      }
    }
    for (const handler of globalHandlers) {
      try { handler(type, payload); } catch { /* isolate errors */ }
    }
  };

  const subscribe = <T extends EventType>(type: T, handler: EventHandler<T>): (() => void) => {
    if (!handlers.has(type)) handlers.set(type, new Set());
    handlers.get(type)!.add(handler as EventHandler<never>);
    return () => { handlers.get(type)?.delete(handler as EventHandler<never>); };
  };

  const subscribeAll = (handler: (type: EventType, payload: unknown) => void): (() => void) => {
    globalHandlers.add(handler);
    return () => { globalHandlers.delete(handler); };
  };

  return Object.freeze({ publish, subscribe, subscribeAll });
}
```

```typescript
// src/events/index.ts
export { createEventBus } from './event-bus.js';
export type { EventBus, EventHandler, EventPayload, EventPayloadMap, EventType } from './types.js';
```

**Step 5: Run test to verify it passes**

Run: `bun test tests/event-bus.test.ts`
Expected: PASS (5/5)

**Step 6: Commit**

```bash
git add src/events/ tests/event-bus.test.ts
git commit -m "feat: add typed event bus with publish/subscribe"
```

---

### Task 2: Tool Permission System

**Files:**
- Create: `src/ai/tools/permissions.ts`
- Test: `tests/tool-permissions.test.ts`

**Step 1: Write the failing test**

```typescript
// tests/tool-permissions.test.ts
import { describe, expect, it } from 'bun:test';
import { createToolPermissionResolver } from '../src/ai/tools/permissions.js';

describe('ToolPermissionResolver', () => {
  it('allows by default with allow policy', async () => {
    const resolver = createToolPermissionResolver({ defaultPolicy: 'allow', rules: [] });
    const result = await resolver.check({ id: '1', name: 'fs_read', arguments: {} });
    expect(result).toBe(true);
  });

  it('denies by default with deny policy', async () => {
    const resolver = createToolPermissionResolver({ defaultPolicy: 'deny', rules: [] });
    const result = await resolver.check({ id: '1', name: 'fs_read', arguments: {} });
    expect(result).toBe(false);
  });

  it('matches tool name glob', async () => {
    const resolver = createToolPermissionResolver({
      defaultPolicy: 'deny',
      rules: [{ tool: 'fs_*', policy: 'allow' }],
    });
    expect(await resolver.check({ id: '1', name: 'fs_read', arguments: {} })).toBe(true);
    expect(await resolver.check({ id: '2', name: 'bash', arguments: {} })).toBe(false);
  });

  it('matches bash command pattern', async () => {
    const resolver = createToolPermissionResolver({
      defaultPolicy: 'deny',
      rules: [{ tool: 'bash', pattern: 'git *', policy: 'allow' }],
    });
    expect(await resolver.check({ id: '1', name: 'bash', arguments: { command: 'git status' } })).toBe(true);
    expect(await resolver.check({ id: '2', name: 'bash', arguments: { command: 'rm -rf /' } })).toBe(false);
  });

  it('last matching rule wins', async () => {
    const resolver = createToolPermissionResolver({
      defaultPolicy: 'allow',
      rules: [
        { tool: 'bash', policy: 'deny' },
        { tool: 'bash', pattern: 'git *', policy: 'allow' },
      ],
    });
    expect(await resolver.check({ id: '1', name: 'bash', arguments: { command: 'git status' } })).toBe(true);
    expect(await resolver.check({ id: '2', name: 'bash', arguments: { command: 'echo hi' } })).toBe(false);
  });

  it('ask policy emits permission.request and waits', async () => {
    const resolver = createToolPermissionResolver({
      defaultPolicy: 'ask',
      rules: [],
      onPermissionRequest: async () => true,
    });
    const result = await resolver.check({ id: '1', name: 'bash', arguments: {} });
    expect(result).toBe(true);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/tool-permissions.test.ts`
Expected: FAIL — module does not exist

**Step 3: Write implementation**

```typescript
// src/ai/tools/permissions.ts
import type { ToolCallRequest, ToolPermissionResolver } from './types.js';

export interface ToolPermissionRule {
  readonly tool: string;
  readonly pattern?: string;
  readonly policy: 'allow' | 'ask' | 'deny';
}

export interface ToolPermissionConfig {
  readonly defaultPolicy: 'allow' | 'ask' | 'deny';
  readonly rules: readonly ToolPermissionRule[];
  readonly onPermissionRequest?: (request: ToolCallRequest) => Promise<boolean>;
}

function globMatch(pattern: string, value: string): boolean {
  const regex = new RegExp(
    '^' + pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.') + '$',
  );
  return regex.test(value);
}

export function createToolPermissionResolver(config: ToolPermissionConfig): ToolPermissionResolver {
  const { defaultPolicy, rules, onPermissionRequest } = config;

  const check = async (request: ToolCallRequest): Promise<boolean> => {
    let policy = defaultPolicy;

    for (const rule of rules) {
      if (!globMatch(rule.tool, request.name)) continue;
      if (rule.pattern) {
        const cmd = typeof request.arguments.command === 'string' ? request.arguments.command : '';
        if (!globMatch(rule.pattern, cmd)) continue;
      }
      policy = rule.policy;
    }

    if (policy === 'allow') return true;
    if (policy === 'deny') return false;
    // ask
    if (onPermissionRequest) return onPermissionRequest(request);
    return false;
  };

  return Object.freeze({ check });
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/tool-permissions.test.ts`
Expected: PASS (6/6)

**Step 5: Commit**

```bash
git add src/ai/tools/permissions.ts tests/tool-permissions.test.ts
git commit -m "feat: add tool permission system with glob matching"
```

---

### Task 3: Batch Tool Execution

**Files:**
- Modify: `src/ai/tools/tool-registry.ts` — add `batchExecute` method
- Modify: `src/ai/tools/types.ts` — add `batchExecute` to `ToolRegistry` interface
- Test: `tests/batch-execution.test.ts`

**Step 1: Write the failing test**

```typescript
// tests/batch-execution.test.ts
import { describe, expect, it, mock } from 'bun:test';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';

describe('batchExecute', () => {
  it('runs multiple tool calls concurrently', async () => {
    const registry = createToolRegistry({});
    registry.register(
      { name: 'echo', description: 'echo', parameters: { text: { type: 'string', description: '' } } },
      async (args) => String(args.text),
    );

    const results = await registry.batchExecute([
      { id: '1', name: 'echo', arguments: { text: 'a' } },
      { id: '2', name: 'echo', arguments: { text: 'b' } },
      { id: '3', name: 'echo', arguments: { text: 'c' } },
    ]);

    expect(results).toHaveLength(3);
    expect(results[0].output).toBe('a');
    expect(results[1].output).toBe('b');
    expect(results[2].output).toBe('c');
  });

  it('returns results in input order', async () => {
    const registry = createToolRegistry({});
    let callOrder = 0;
    registry.register(
      { name: 'delayed', description: '', parameters: {} },
      async () => { callOrder++; return String(callOrder); },
    );

    const results = await registry.batchExecute([
      { id: '1', name: 'delayed', arguments: {} },
      { id: '2', name: 'delayed', arguments: {} },
    ]);

    expect(results).toHaveLength(2);
    expect(results[0].id).toBe('1');
    expect(results[1].id).toBe('2');
  });

  it('isolates errors per call', async () => {
    const registry = createToolRegistry({});
    registry.register(
      { name: 'maybe_fail', description: '', parameters: { fail: { type: 'boolean', description: '' } } },
      async (args) => { if (args.fail) throw new Error('boom'); return 'ok'; },
    );

    const results = await registry.batchExecute([
      { id: '1', name: 'maybe_fail', arguments: { fail: false } },
      { id: '2', name: 'maybe_fail', arguments: { fail: true } },
    ]);

    expect(results[0].isError).toBe(false);
    expect(results[1].isError).toBe(true);
  });

  it('respects maxConcurrency', async () => {
    const registry = createToolRegistry({});
    let concurrent = 0;
    let maxConcurrent = 0;

    registry.register(
      { name: 'track', description: '', parameters: {} },
      async () => {
        concurrent++;
        maxConcurrent = Math.max(maxConcurrent, concurrent);
        await new Promise(r => setTimeout(r, 50));
        concurrent--;
        return 'done';
      },
    );

    await registry.batchExecute(
      Array.from({ length: 6 }, (_, i) => ({ id: String(i), name: 'track', arguments: {} })),
      { maxConcurrency: 2 },
    );

    expect(maxConcurrent).toBeLessThanOrEqual(2);
  });

  it('returns empty array for empty input', async () => {
    const registry = createToolRegistry({});
    const results = await registry.batchExecute([]);
    expect(results).toEqual([]);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/batch-execution.test.ts`
Expected: FAIL — `batchExecute` not found on registry

**Step 3: Add batchExecute to ToolRegistry interface**

In `src/ai/tools/types.ts`, add to `ToolRegistry`:
```typescript
readonly batchExecute: (
  calls: readonly ToolCallRequest[],
  options?: { readonly maxConcurrency?: number },
) => Promise<readonly ToolCallResult[]>;
```

**Step 4: Implement batchExecute in tool-registry.ts**

Add to `createToolRegistry()` before the return:
```typescript
const batchExecute = async (
  calls: readonly ToolCallRequest[],
  options?: { readonly maxConcurrency?: number },
): Promise<readonly ToolCallResult[]> => {
  if (calls.length === 0) return [];
  const maxConcurrency = options?.maxConcurrency ?? 8;
  const results: ToolCallResult[] = new Array(calls.length);
  let nextIndex = 0;

  const worker = async (): Promise<void> => {
    while (nextIndex < calls.length) {
      const i = nextIndex++;
      results[i] = await execute(calls[i]);
    }
  };

  const workers = Array.from(
    { length: Math.min(maxConcurrency, calls.length) },
    () => worker(),
  );
  await Promise.allSettled(workers);

  return Object.freeze(results);
};
```

Add `batchExecute` to the returned frozen object.

**Step 5: Run test to verify it passes**

Run: `bun test tests/batch-execution.test.ts`
Expected: PASS (5/5)

**Step 6: Commit**

```bash
git add src/ai/tools/tool-registry.ts src/ai/tools/types.ts tests/batch-execution.test.ts
git commit -m "feat: add batch tool execution with configurable concurrency"
```

---

### Task 4: Plugin Hook System

**Files:**
- Create: `src/hooks/hook-system.ts`
- Create: `src/hooks/types.ts`
- Create: `src/hooks/index.ts`
- Test: `tests/hook-system.test.ts`

**Step 1: Write the failing test**

```typescript
// tests/hook-system.test.ts
import { describe, expect, it, mock } from 'bun:test';
import { createHookSystem } from '../src/hooks/hook-system.js';
import type { HookSystem } from '../src/hooks/types.js';

describe('HookSystem', () => {
  it('runs tool.execute.before hooks and returns modified request', async () => {
    const hooks = createHookSystem();
    hooks.register('tool.execute.before', async (ctx) => ({
      ...ctx.request,
      arguments: { ...ctx.request.arguments, injected: true },
    }));
    const result = await hooks.run('tool.execute.before', {
      request: { id: '1', name: 'test', arguments: {} },
    });
    expect(result.arguments.injected).toBe(true);
  });

  it('blocks tool execution when hook returns blocked', async () => {
    const hooks = createHookSystem();
    hooks.register('tool.execute.before', async () => ({
      blocked: true,
      reason: 'Not allowed',
    }));
    const result = await hooks.run('tool.execute.before', {
      request: { id: '1', name: 'test', arguments: {} },
    });
    expect(result.blocked).toBe(true);
    expect(result.reason).toBe('Not allowed');
  });

  it('runs tool.execute.after hooks', async () => {
    const hooks = createHookSystem();
    hooks.register('tool.execute.after', async (ctx) => ({
      ...ctx.result,
      output: ctx.result.output + ' [modified]',
    }));
    const result = await hooks.run('tool.execute.after', {
      request: { id: '1', name: 'test', arguments: {} },
      result: { id: '1', name: 'test', output: 'hello', isError: false },
    });
    expect(result.output).toBe('hello [modified]');
  });

  it('runs prompt.system.transform hooks', async () => {
    const hooks = createHookSystem();
    hooks.register('prompt.system.transform', async (ctx) => ctx.prompt + '\nExtra instructions');
    const result = await hooks.run('prompt.system.transform', { prompt: 'Base prompt' });
    expect(result).toBe('Base prompt\nExtra instructions');
  });

  it('unregisters hooks', async () => {
    const hooks = createHookSystem();
    const handler = mock(async (ctx: { prompt: string }) => ctx.prompt);
    const unsub = hooks.register('prompt.system.transform', handler);
    unsub();
    await hooks.run('prompt.system.transform', { prompt: 'test' });
    expect(handler).not.toHaveBeenCalled();
  });

  it('chains multiple hooks in registration order', async () => {
    const hooks = createHookSystem();
    hooks.register('prompt.system.transform', async (ctx) => ctx.prompt + ' [A]');
    hooks.register('prompt.system.transform', async (ctx) => ctx.prompt + ' [B]');
    const result = await hooks.run('prompt.system.transform', { prompt: 'Start' });
    expect(result).toBe('Start [A] [B]');
  });

  it('runs tool.result.validate hooks returning messages', async () => {
    const hooks = createHookSystem();
    hooks.register('tool.result.validate', async () => ['Warning: file has lint errors']);
    const result = await hooks.run('tool.result.validate', {
      request: { id: '1', name: 'fs_write', arguments: {} },
      result: { id: '1', name: 'fs_write', output: 'ok', isError: false },
    });
    expect(result).toEqual(['Warning: file has lint errors']);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/hook-system.test.ts`
Expected: FAIL — modules do not exist

**Step 3: Write types**

```typescript
// src/hooks/types.ts
import type { ToolCallRequest, ToolCallResult } from '../ai/tools/types.js';

export interface HookContextMap {
  'tool.execute.before': { readonly request: ToolCallRequest };
  'tool.execute.after': { readonly request: ToolCallRequest; readonly result: ToolCallResult };
  'tool.result.validate': { readonly request: ToolCallRequest; readonly result: ToolCallResult };
  'prompt.system.transform': { readonly prompt: string };
  'prompt.messages.transform': { readonly messages: readonly unknown[] };
  'session.compacting': { readonly messages: readonly unknown[]; readonly summary: string };
}

export type HookType = keyof HookContextMap;

export interface BlockedResult {
  readonly blocked: true;
  readonly reason: string;
}

export type HookResultMap = {
  'tool.execute.before': ToolCallRequest | BlockedResult;
  'tool.execute.after': ToolCallResult;
  'tool.result.validate': readonly string[];
  'prompt.system.transform': string;
  'prompt.messages.transform': readonly unknown[];
  'session.compacting': string;
};

export type HookHandler<T extends HookType> = (
  context: HookContextMap[T],
) => Promise<HookResultMap[T]>;

export interface HookSystem {
  readonly register: <T extends HookType>(type: T, handler: HookHandler<T>) => () => void;
  readonly run: <T extends HookType>(type: T, context: HookContextMap[T]) => Promise<HookResultMap[T]>;
}
```

**Step 4: Write implementation**

```typescript
// src/hooks/hook-system.ts
import type { HookHandler, HookResultMap, HookSystem, HookType } from './types.js';

export function createHookSystem(): HookSystem {
  const handlers = new Map<HookType, Set<HookHandler<never>>>();

  const register = <T extends HookType>(type: T, handler: HookHandler<T>): (() => void) => {
    if (!handlers.has(type)) handlers.set(type, new Set());
    const set = handlers.get(type)!;
    set.add(handler as HookHandler<never>);
    return () => { set.delete(handler as HookHandler<never>); };
  };

  const run = async <T extends HookType>(
    type: T,
    context: Parameters<HookHandler<T>>[0],
  ): Promise<HookResultMap[T]> => {
    const set = handlers.get(type);
    if (!set || set.size === 0) {
      // Return sensible defaults when no hooks registered
      if (type === 'tool.execute.before') return (context as { request: unknown }).request as HookResultMap[T];
      if (type === 'tool.execute.after') return (context as { result: unknown }).result as HookResultMap[T];
      if (type === 'tool.result.validate') return [] as unknown as HookResultMap[T];
      if (type === 'prompt.system.transform') return (context as { prompt: string }).prompt as HookResultMap[T];
      if (type === 'prompt.messages.transform') return (context as { messages: unknown }).messages as HookResultMap[T];
      if (type === 'session.compacting') return (context as { summary: string }).summary as HookResultMap[T];
      return undefined as HookResultMap[T];
    }

    let result: unknown;

    for (const handler of set) {
      const hookResult = await (handler as HookHandler<T>)(context);

      // Special handling per hook type for chaining
      if (type === 'tool.execute.before') {
        if (hookResult && typeof hookResult === 'object' && 'blocked' in hookResult) {
          return hookResult as HookResultMap[T];
        }
        context = { ...context, request: hookResult } as Parameters<HookHandler<T>>[0];
        result = hookResult;
      } else if (type === 'tool.execute.after') {
        context = { ...context, result: hookResult } as Parameters<HookHandler<T>>[0];
        result = hookResult;
      } else if (type === 'tool.result.validate') {
        result = result ? [...(result as string[]), ...(hookResult as string[])] : hookResult;
      } else if (type === 'prompt.system.transform') {
        context = { prompt: hookResult } as Parameters<HookHandler<T>>[0];
        result = hookResult;
      } else if (type === 'prompt.messages.transform') {
        context = { messages: hookResult } as Parameters<HookHandler<T>>[0];
        result = hookResult;
      } else if (type === 'session.compacting') {
        context = { ...context, summary: hookResult } as Parameters<HookHandler<T>>[0];
        result = hookResult;
      }
    }

    return result as HookResultMap[T];
  };

  return Object.freeze({ register, run });
}
```

```typescript
// src/hooks/index.ts
export { createHookSystem } from './hook-system.js';
export type { BlockedResult, HookContextMap, HookHandler, HookResultMap, HookSystem, HookType } from './types.js';
```

**Step 5: Run test to verify it passes**

Run: `bun test tests/hook-system.test.ts`
Expected: PASS (7/7)

**Step 6: Commit**

```bash
git add src/hooks/ tests/hook-system.test.ts
git commit -m "feat: add plugin hook system for tool/prompt lifecycle"
```

---

### Task 5: Two-Phase Context Management

**Files:**
- Create: `src/ai/conversation/context-pruner.ts`
- Modify: `src/ai/conversation/types.ts` — add pruning options
- Test: `tests/context-pruner.test.ts`

**Step 1: Write the failing test**

```typescript
// tests/context-pruner.test.ts
import { describe, expect, it } from 'bun:test';
import { createContextPruner } from '../src/ai/conversation/context-pruner.js';
import type { ConversationMessage } from '../src/ai/conversation/types.js';

function makeMessages(count: number): ConversationMessage[] {
  const messages: ConversationMessage[] = [];
  for (let i = 0; i < count; i++) {
    messages.push({ role: 'user', content: `Question ${i}`, timestamp: Date.now() - (count - i) * 1000 });
    messages.push({ role: 'assistant', content: `Answer ${i}` });
    messages.push({ role: 'tool_result', content: 'x'.repeat(5000), toolCallId: `t${i}`, toolName: `tool${i}` });
  }
  return messages;
}

describe('ContextPruner', () => {
  it('prunes old tool outputs beyond protected window', () => {
    const pruner = createContextPruner({ protectRecentTurns: 2, pruneProtectedTools: [] });
    const messages = makeMessages(6);
    const pruned = pruner.prune(messages);
    // Recent 2 turns (6 messages) should be untouched
    const toolResults = pruned.filter(m => m.role === 'tool_result');
    const prunedOnes = toolResults.filter(m => m.content.startsWith('[OUTPUT PRUNED'));
    expect(prunedOnes.length).toBeGreaterThan(0);
    // Recent tool results should be intact
    const lastToolResult = pruned[pruned.length - 1];
    expect(lastToolResult.content).not.toContain('[OUTPUT PRUNED');
  });

  it('preserves protected tools', () => {
    const pruner = createContextPruner({ protectRecentTurns: 1, pruneProtectedTools: ['tool0'] });
    const messages = makeMessages(4);
    const pruned = pruner.prune(messages);
    const tool0Result = pruned.find(m => m.toolName === 'tool0');
    expect(tool0Result?.content).not.toContain('[OUTPUT PRUNED');
  });

  it('preserves messages after summary marker', () => {
    const pruner = createContextPruner({ protectRecentTurns: 1, pruneProtectedTools: [] });
    const messages: ConversationMessage[] = [
      { role: 'user', content: 'old question' },
      { role: 'assistant', content: '[SUMMARY]' },
      { role: 'tool_result', content: 'x'.repeat(5000), toolCallId: 't1', toolName: 'test' },
      { role: 'user', content: 'new question' },
      { role: 'assistant', content: 'answer' },
    ];
    const pruned = pruner.prune(messages);
    // The tool result after summary should still be prunable if outside recent window
    expect(pruned.length).toBe(messages.length);
  });

  it('returns same messages when nothing to prune', () => {
    const pruner = createContextPruner({ protectRecentTurns: 10, pruneProtectedTools: [] });
    const messages = makeMessages(2);
    const pruned = pruner.prune(messages);
    expect(pruned.length).toBe(messages.length);
  });

  it('includes original size in pruned marker', () => {
    const pruner = createContextPruner({ protectRecentTurns: 1, pruneProtectedTools: [] });
    const messages = makeMessages(4);
    const pruned = pruner.prune(messages);
    const prunedMsg = pruned.find(m => m.content.includes('[OUTPUT PRUNED'));
    expect(prunedMsg?.content).toMatch(/\d+ chars/);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/context-pruner.test.ts`
Expected: FAIL — module does not exist

**Step 3: Write implementation**

```typescript
// src/ai/conversation/context-pruner.ts
import type { ConversationMessage } from './types.js';

export interface ContextPrunerOptions {
  readonly protectRecentTurns?: number;
  readonly pruneProtectedTools?: readonly string[];
}

export interface ContextPruner {
  readonly prune: (messages: readonly ConversationMessage[]) => readonly ConversationMessage[];
}

export function createContextPruner(options?: ContextPrunerOptions): ContextPruner {
  const protectRecentTurns = options?.protectRecentTurns ?? 2;
  const protectedTools = new Set(options?.pruneProtectedTools ?? []);

  const prune = (messages: readonly ConversationMessage[]): readonly ConversationMessage[] => {
    if (messages.length === 0) return messages;

    // Find the boundary: protect the most recent N turns
    // A "turn" is a user message + assistant response + tool results
    let protectedStart = messages.length;
    let turnsFound = 0;

    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].role === 'user') {
        turnsFound++;
        if (turnsFound >= protectRecentTurns) {
          protectedStart = i;
          break;
        }
      }
    }

    // Find summary boundary — don't prune before the most recent summary
    let summaryBoundary = 0;
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].role === 'assistant' && messages[i].content.startsWith('[SUMMARY]')) {
        summaryBoundary = i + 1;
        break;
      }
    }

    const pruneEnd = Math.min(protectedStart, messages.length);

    return messages.map((msg, i) => {
      if (i >= pruneEnd) return msg;
      if (i < summaryBoundary) return msg;
      if (msg.role !== 'tool_result') return msg;
      if (msg.toolName && protectedTools.has(msg.toolName)) return msg;
      if (msg.content.length < 200) return msg;

      return Object.freeze({
        ...msg,
        content: `[OUTPUT PRUNED — ${msg.content.length} chars]`,
      });
    });
  };

  return Object.freeze({ prune });
}
```

**Step 4: Add pruning options to ConversationOptions**

In `src/ai/conversation/types.ts`, add:
```typescript
readonly pruneProtectTurns?: number;
readonly pruneProtectedTools?: readonly string[];
```

**Step 5: Run test to verify it passes**

Run: `bun test tests/context-pruner.test.ts`
Expected: PASS (5/5)

**Step 6: Commit**

```bash
git add src/ai/conversation/context-pruner.ts src/ai/conversation/types.ts tests/context-pruner.test.ts
git commit -m "feat: add two-phase context management with tool output pruning"
```

---

### Task 6: Filesystem Tools

**Files:**
- Create: `src/ai/tools/host/filesystem.ts`
- Create: `src/ai/tools/host/fuzzy-edit.ts`
- Test: `tests/filesystem-tools.test.ts`
- Test: `tests/fuzzy-edit.test.ts`

**Step 1: Write the failing tests**

```typescript
// tests/fuzzy-edit.test.ts
import { describe, expect, it } from 'bun:test';
import { fuzzyMatch } from '../src/ai/tools/host/fuzzy-edit.js';

describe('fuzzyMatch', () => {
  it('exact match', () => {
    const content = 'line 1\nline 2\nline 3\n';
    const result = fuzzyMatch(content, 'line 2', 'LINE TWO');
    expect(result).not.toBeNull();
    expect(result!.replaced).toContain('LINE TWO');
  });

  it('line-trimmed match', () => {
    const content = '  line 1  \n  line 2  \n  line 3  \n';
    const result = fuzzyMatch(content, 'line 2', 'LINE TWO');
    expect(result).not.toBeNull();
  });

  it('whitespace-normalized match', () => {
    const content = 'line   1\nline    2\nline   3\n';
    const result = fuzzyMatch(content, 'line 2', 'LINE TWO');
    expect(result).not.toBeNull();
  });

  it('indentation-flexible match', () => {
    const content = '    if (true) {\n        doThing();\n    }\n';
    const result = fuzzyMatch(content, 'if (true) {\n    doThing();\n}', 'if (false) {\n    doOther();\n}');
    expect(result).not.toBeNull();
  });

  it('block-anchor match with levenshtein', () => {
    const content = 'function hello() {\n  console.log("hello");\n  return true;\n}\n';
    const result = fuzzyMatch(content, 'function hello() {\n  console.log("helo");\n  return true;\n}', 'function hello() {\n  console.log("hi");\n  return true;\n}');
    expect(result).not.toBeNull();
  });

  it('returns null when no strategy matches', () => {
    const content = 'completely different content\n';
    const result = fuzzyMatch(content, 'nothing matches this', 'replacement');
    expect(result).toBeNull();
  });
});
```

```typescript
// tests/filesystem-tools.test.ts
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtemp, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { registerFilesystemTools } from '../src/ai/tools/host/filesystem.js';

describe('Filesystem Tools', () => {
  let tempDir: string;
  let registry: ReturnType<typeof createToolRegistry>;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'simse-fs-'));
    registry = createToolRegistry({});
    registerFilesystemTools(registry, { workingDirectory: tempDir });
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('fs_read reads a file', async () => {
    await writeFile(join(tempDir, 'test.txt'), 'hello world');
    const result = await registry.execute({ id: '1', name: 'fs_read', arguments: { path: 'test.txt' } });
    expect(result.isError).toBe(false);
    expect(result.output).toContain('hello world');
  });

  it('fs_read rejects path escape', async () => {
    const result = await registry.execute({ id: '1', name: 'fs_read', arguments: { path: '../../../etc/passwd' } });
    expect(result.isError).toBe(true);
  });

  it('fs_write creates a file', async () => {
    const result = await registry.execute({
      id: '1', name: 'fs_write',
      arguments: { path: 'new.txt', content: 'created' },
    });
    expect(result.isError).toBe(false);
    const file = await Bun.file(join(tempDir, 'new.txt')).text();
    expect(file).toBe('created');
  });

  it('fs_edit replaces text in file', async () => {
    await writeFile(join(tempDir, 'edit.txt'), 'hello world');
    const result = await registry.execute({
      id: '1', name: 'fs_edit',
      arguments: { path: 'edit.txt', old_string: 'hello', new_string: 'goodbye' },
    });
    expect(result.isError).toBe(false);
    const file = await Bun.file(join(tempDir, 'edit.txt')).text();
    expect(file).toContain('goodbye');
  });

  it('fs_glob finds files by pattern', async () => {
    await writeFile(join(tempDir, 'a.ts'), '');
    await writeFile(join(tempDir, 'b.ts'), '');
    await writeFile(join(tempDir, 'c.js'), '');
    const result = await registry.execute({ id: '1', name: 'fs_glob', arguments: { pattern: '*.ts' } });
    expect(result.isError).toBe(false);
    expect(result.output).toContain('a.ts');
    expect(result.output).toContain('b.ts');
    expect(result.output).not.toContain('c.js');
  });

  it('fs_grep searches file contents', async () => {
    await writeFile(join(tempDir, 'search.txt'), 'foo bar baz\nhello world\nfoo again');
    const result = await registry.execute({ id: '1', name: 'fs_grep', arguments: { pattern: 'foo', path: '.' } });
    expect(result.isError).toBe(false);
    expect(result.output).toContain('foo');
  });

  it('fs_list lists directory contents', async () => {
    await writeFile(join(tempDir, 'file1.txt'), '');
    await mkdir(join(tempDir, 'subdir'));
    const result = await registry.execute({ id: '1', name: 'fs_list', arguments: { path: '.' } });
    expect(result.isError).toBe(false);
    expect(result.output).toContain('file1.txt');
    expect(result.output).toContain('subdir');
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test tests/fuzzy-edit.test.ts tests/filesystem-tools.test.ts`
Expected: FAIL — modules do not exist

**Step 3: Write fuzzy-edit**

```typescript
// src/ai/tools/host/fuzzy-edit.ts
// 5-strategy fuzzy matching for file edits

export interface FuzzyMatchResult {
  readonly replaced: string;
  readonly strategy: string;
}

function levenshtein(a: string, b: string): number {
  const m = a.length;
  const n = b.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
  for (let i = 0; i <= m; i++) dp[i][0] = i;
  for (let j = 0; j <= n; j++) dp[0][j] = j;
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] = a[i - 1] === b[j - 1]
        ? dp[i - 1][j - 1]
        : 1 + Math.min(dp[i - 1][j], dp[i][j - 1], dp[i - 1][j - 1]);
    }
  }
  return dp[m][n];
}

// Strategy 1: Exact string match
function exactMatch(content: string, oldStr: string, newStr: string): FuzzyMatchResult | null {
  const idx = content.indexOf(oldStr);
  if (idx === -1) return null;
  return { replaced: content.slice(0, idx) + newStr + content.slice(idx + oldStr.length), strategy: 'exact' };
}

// Strategy 2: Line-trimmed match
function lineTrimmedMatch(content: string, oldStr: string, newStr: string): FuzzyMatchResult | null {
  const contentLines = content.split('\n');
  const oldLines = oldStr.split('\n').map(l => l.trim());
  for (let i = 0; i <= contentLines.length - oldLines.length; i++) {
    const slice = contentLines.slice(i, i + oldLines.length);
    if (slice.every((l, j) => l.trim() === oldLines[j])) {
      const result = [...contentLines.slice(0, i), ...newStr.split('\n'), ...contentLines.slice(i + oldLines.length)];
      return { replaced: result.join('\n'), strategy: 'line-trimmed' };
    }
  }
  return null;
}

// Strategy 3: Whitespace-normalized match
function whitespaceNormalizedMatch(content: string, oldStr: string, newStr: string): FuzzyMatchResult | null {
  const normalize = (s: string) => s.replace(/\s+/g, ' ').trim();
  const contentLines = content.split('\n');
  const oldLines = oldStr.split('\n');
  const normalizedOld = oldLines.map(normalize);

  for (let i = 0; i <= contentLines.length - oldLines.length; i++) {
    const slice = contentLines.slice(i, i + oldLines.length);
    if (slice.every((l, j) => normalize(l) === normalizedOld[j])) {
      const result = [...contentLines.slice(0, i), ...newStr.split('\n'), ...contentLines.slice(i + oldLines.length)];
      return { replaced: result.join('\n'), strategy: 'whitespace-normalized' };
    }
  }
  return null;
}

// Strategy 4: Indentation-flexible match
function indentFlexibleMatch(content: string, oldStr: string, newStr: string): FuzzyMatchResult | null {
  const contentLines = content.split('\n');
  const oldLines = oldStr.split('\n');
  const stripIndent = (lines: string[]): string[] => {
    const minIndent = Math.min(...lines.filter(l => l.trim()).map(l => l.match(/^(\s*)/)?.[1].length ?? 0));
    return lines.map(l => l.slice(minIndent));
  };
  const strippedOld = stripIndent(oldLines);

  for (let i = 0; i <= contentLines.length - oldLines.length; i++) {
    const slice = contentLines.slice(i, i + oldLines.length);
    const strippedSlice = stripIndent(slice);
    if (strippedSlice.every((l, j) => l === strippedOld[j])) {
      // Determine the indentation to apply to new string
      const baseIndent = slice[0].match(/^(\s*)/)?.[1] ?? '';
      const newLines = newStr.split('\n').map((l, j) => j === 0 ? baseIndent + l : baseIndent + l);
      const result = [...contentLines.slice(0, i), ...newLines, ...contentLines.slice(i + oldLines.length)];
      return { replaced: result.join('\n'), strategy: 'indent-flexible' };
    }
  }
  return null;
}

// Strategy 5: Block-anchor match with Levenshtein
function blockAnchorMatch(content: string, oldStr: string, newStr: string): FuzzyMatchResult | null {
  const contentLines = content.split('\n');
  const oldLines = oldStr.split('\n');
  if (oldLines.length < 2) return null;

  const firstLine = oldLines[0].trim();
  const lastLine = oldLines[oldLines.length - 1].trim();
  const maxDist = Math.floor(oldStr.length * 0.3); // 30% tolerance

  for (let i = 0; i <= contentLines.length - oldLines.length; i++) {
    if (contentLines[i].trim() !== firstLine) continue;
    for (let end = i + oldLines.length - 2; end <= Math.min(i + oldLines.length + 2, contentLines.length - 1); end++) {
      if (contentLines[end].trim() !== lastLine) continue;
      const candidate = contentLines.slice(i, end + 1).join('\n');
      if (levenshtein(candidate, oldStr) <= maxDist) {
        const result = [...contentLines.slice(0, i), ...newStr.split('\n'), ...contentLines.slice(end + 1)];
        return { replaced: result.join('\n'), strategy: 'block-anchor' };
      }
    }
  }
  return null;
}

export function fuzzyMatch(content: string, oldStr: string, newStr: string): FuzzyMatchResult | null {
  return (
    exactMatch(content, oldStr, newStr) ??
    lineTrimmedMatch(content, oldStr, newStr) ??
    whitespaceNormalizedMatch(content, oldStr, newStr) ??
    indentFlexibleMatch(content, oldStr, newStr) ??
    blockAnchorMatch(content, oldStr, newStr)
  );
}
```

**Step 4: Write filesystem tools**

```typescript
// src/ai/tools/host/filesystem.ts
import { readFile, writeFile, readdir, stat, mkdir } from 'node:fs/promises';
import { join, resolve, relative, isAbsolute } from 'node:path';
import type { ToolRegistry } from '../types.js';
import { fuzzyMatch } from './fuzzy-edit.js';

export interface FilesystemToolOptions {
  readonly workingDirectory: string;
  readonly allowedPaths?: readonly string[];
}

function resolveSafe(workDir: string, filePath: string): string {
  const resolved = isAbsolute(filePath) ? filePath : resolve(workDir, filePath);
  const rel = relative(workDir, resolved);
  if (rel.startsWith('..') || isAbsolute(rel)) {
    throw new Error(`Path escapes working directory: ${filePath}`);
  }
  return resolved;
}

export function registerFilesystemTools(registry: ToolRegistry, options: FilesystemToolOptions): void {
  const { workingDirectory } = options;

  // fs_read
  registry.register(
    {
      name: 'fs_read',
      description: 'Read file contents with optional line range',
      parameters: {
        path: { type: 'string', description: 'File path relative to working directory', required: true },
        offset: { type: 'number', description: 'Start line (1-based)' },
        limit: { type: 'number', description: 'Number of lines to read' },
      },
      category: 'read',
      annotations: { readOnly: true },
    },
    async (args) => {
      const filePath = resolveSafe(workingDirectory, String(args.path));
      const content = await readFile(filePath, 'utf-8');
      const lines = content.split('\n');
      const offset = typeof args.offset === 'number' ? Math.max(1, args.offset) : 1;
      const limit = typeof args.limit === 'number' ? args.limit : lines.length;
      const slice = lines.slice(offset - 1, offset - 1 + limit);
      return slice.map((line, i) => `${offset + i}\t${line}`).join('\n');
    },
  );

  // fs_write
  registry.register(
    {
      name: 'fs_write',
      description: 'Create or overwrite a file',
      parameters: {
        path: { type: 'string', description: 'File path relative to working directory', required: true },
        content: { type: 'string', description: 'File content', required: true },
      },
      category: 'edit',
      annotations: { destructive: true },
    },
    async (args) => {
      const filePath = resolveSafe(workingDirectory, String(args.path));
      const dir = join(filePath, '..');
      await mkdir(dir, { recursive: true });
      await writeFile(filePath, String(args.content), 'utf-8');
      return `Wrote ${String(args.content).length} bytes to ${args.path}`;
    },
  );

  // fs_edit
  registry.register(
    {
      name: 'fs_edit',
      description: 'Edit a file by replacing text with 5-strategy fuzzy matching fallback',
      parameters: {
        path: { type: 'string', description: 'File path', required: true },
        old_string: { type: 'string', description: 'Text to find', required: true },
        new_string: { type: 'string', description: 'Replacement text', required: true },
      },
      category: 'edit',
    },
    async (args) => {
      const filePath = resolveSafe(workingDirectory, String(args.path));
      const content = await readFile(filePath, 'utf-8');
      const result = fuzzyMatch(content, String(args.old_string), String(args.new_string));
      if (!result) return `Error: Could not find matching text in ${args.path}`;
      await writeFile(filePath, result.replaced, 'utf-8');
      return `Edited ${args.path} (strategy: ${result.strategy})`;
    },
  );

  // fs_glob
  registry.register(
    {
      name: 'fs_glob',
      description: 'Find files matching a glob pattern',
      parameters: {
        pattern: { type: 'string', description: 'Glob pattern (e.g., **/*.ts)', required: true },
      },
      category: 'search',
      annotations: { readOnly: true },
    },
    async (args) => {
      const glob = new Bun.Glob(String(args.pattern));
      const matches: string[] = [];
      for await (const path of glob.scan({ cwd: workingDirectory })) {
        matches.push(path);
        if (matches.length >= 1000) break;
      }
      return matches.join('\n') || 'No matches found';
    },
  );

  // fs_grep
  registry.register(
    {
      name: 'fs_grep',
      description: 'Search file contents with regex',
      parameters: {
        pattern: { type: 'string', description: 'Regex pattern', required: true },
        path: { type: 'string', description: 'Directory or file to search', required: true },
        glob: { type: 'string', description: 'File glob filter (e.g., *.ts)' },
      },
      category: 'search',
      annotations: { readOnly: true },
    },
    async (args) => {
      const searchPath = resolveSafe(workingDirectory, String(args.path));
      const regex = new RegExp(String(args.pattern), 'gm');
      const results: string[] = [];

      const searchFile = async (filePath: string): Promise<void> => {
        try {
          const content = await readFile(filePath, 'utf-8');
          const lines = content.split('\n');
          for (let i = 0; i < lines.length; i++) {
            if (regex.test(lines[i])) {
              const relPath = relative(workingDirectory, filePath);
              results.push(`${relPath}:${i + 1}:${lines[i]}`);
            }
            regex.lastIndex = 0;
          }
        } catch { /* skip binary/unreadable files */ }
      };

      const fileStat = await stat(searchPath);
      if (fileStat.isFile()) {
        await searchFile(searchPath);
      } else {
        const fileGlob = args.glob ? String(args.glob) : '**/*';
        const glob = new Bun.Glob(fileGlob);
        for await (const path of glob.scan({ cwd: searchPath })) {
          await searchFile(join(searchPath, path));
          if (results.length >= 500) break;
        }
      }

      return results.join('\n') || 'No matches found';
    },
  );

  // fs_list
  registry.register(
    {
      name: 'fs_list',
      description: 'List directory contents',
      parameters: {
        path: { type: 'string', description: 'Directory path', required: true },
        depth: { type: 'number', description: 'Max depth (default 1)' },
      },
      category: 'read',
      annotations: { readOnly: true },
    },
    async (args) => {
      const dirPath = resolveSafe(workingDirectory, String(args.path));
      const maxDepth = typeof args.depth === 'number' ? args.depth : 1;
      const entries: string[] = [];

      const list = async (dir: string, depth: number): Promise<void> => {
        if (depth > maxDepth) return;
        const items = await readdir(dir, { withFileTypes: true });
        for (const item of items) {
          const rel = relative(workingDirectory, join(dir, item.name));
          const suffix = item.isDirectory() ? '/' : '';
          entries.push(rel + suffix);
          if (item.isDirectory() && depth < maxDepth) {
            await list(join(dir, item.name), depth + 1);
          }
        }
      };

      await list(dirPath, 1);
      return entries.join('\n') || 'Empty directory';
    },
  );
}
```

**Step 5: Run tests to verify they pass**

Run: `bun test tests/fuzzy-edit.test.ts tests/filesystem-tools.test.ts`
Expected: PASS (6/6 fuzzy, 7/7 filesystem)

**Step 6: Commit**

```bash
git add src/ai/tools/host/ tests/fuzzy-edit.test.ts tests/filesystem-tools.test.ts
git commit -m "feat: add filesystem tools with 5-strategy fuzzy edit"
```

---

### Task 7: Bash Tool

**Files:**
- Create: `src/ai/tools/host/bash.ts`
- Test: `tests/bash-tool.test.ts`

Uses `Bun.spawn` for safe subprocess execution — no shell string injection.

**Step 1: Write the failing test**

```typescript
// tests/bash-tool.test.ts
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { registerBashTool } from '../src/ai/tools/host/bash.js';

describe('Bash Tool', () => {
  let tempDir: string;
  let registry: ReturnType<typeof createToolRegistry>;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'simse-bash-'));
    registry = createToolRegistry({});
    registerBashTool(registry, { workingDirectory: tempDir });
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('runs a simple command', async () => {
    const result = await registry.execute({ id: '1', name: 'bash', arguments: { command: 'echo hello' } });
    expect(result.isError).toBe(false);
    expect(result.output.trim()).toBe('hello');
  });

  it('returns error for failing command', async () => {
    const result = await registry.execute({ id: '1', name: 'bash', arguments: { command: 'false' } });
    expect(result.isError).toBe(true);
  });

  it('captures stderr', async () => {
    const result = await registry.execute({ id: '1', name: 'bash', arguments: { command: 'echo err >&2' } });
    expect(result.output).toContain('err');
  });

  it('respects timeout', async () => {
    const result = await registry.execute({
      id: '1', name: 'bash',
      arguments: { command: 'sleep 60', timeout: 500 },
    });
    expect(result.isError).toBe(true);
    expect(result.output).toContain('timeout');
  });

  it('truncates large output', async () => {
    registry = createToolRegistry({});
    registerBashTool(registry, { workingDirectory: tempDir, maxOutputBytes: 100 });
    const result = await registry.execute({
      id: '1', name: 'bash',
      arguments: { command: 'yes | head -1000' },
    });
    expect(result.output.length).toBeLessThanOrEqual(200); // some overhead for truncation message
  });
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/bash-tool.test.ts`
Expected: FAIL — module does not exist

**Step 3: Write implementation**

Uses `Bun.spawn` with `cmd` array to avoid shell injection risks.

```typescript
// src/ai/tools/host/bash.ts
import type { ToolRegistry } from '../types.js';

export interface BashToolOptions {
  readonly workingDirectory: string;
  readonly defaultTimeoutMs?: number;
  readonly maxOutputBytes?: number;
  readonly env?: Readonly<Record<string, string>>;
  readonly shell?: string;
}

export function registerBashTool(registry: ToolRegistry, options: BashToolOptions): void {
  const {
    workingDirectory,
    defaultTimeoutMs = 120_000,
    maxOutputBytes = 50_000,
    env,
    shell = process.platform === 'win32' ? 'bash' : '/bin/sh',
  } = options;

  registry.register(
    {
      name: 'bash',
      description: 'Run a shell command',
      parameters: {
        command: { type: 'string', description: 'Shell command to run', required: true },
        timeout: { type: 'number', description: 'Timeout in milliseconds' },
        cwd: { type: 'string', description: 'Working directory override' },
      },
      category: 'execute',
      annotations: { destructive: true },
    },
    async (args) => {
      const command = String(args.command);
      const timeout = typeof args.timeout === 'number' ? args.timeout : defaultTimeoutMs;
      const cwd = typeof args.cwd === 'string' ? args.cwd : workingDirectory;

      try {
        const proc = Bun.spawn([shell, '-c', command], {
          cwd,
          env: { ...process.env, ...env },
          stdout: 'pipe',
          stderr: 'pipe',
        });

        // Set up timeout
        const timer = setTimeout(() => { proc.kill(); }, timeout);

        const [stdout, stderr] = await Promise.all([
          new Response(proc.stdout).text(),
          new Response(proc.stderr).text(),
        ]);

        const exitCode = await proc.exited;
        clearTimeout(timer);

        let output = stdout + (stderr ? stderr : '');

        // Truncate if needed
        if (output.length > maxOutputBytes) {
          output = output.slice(0, maxOutputBytes) + `\n[output truncated at ${maxOutputBytes} bytes]`;
        }

        if (exitCode !== 0) {
          return `[exit code ${exitCode}]\n${output}`;
        }

        return output;
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        if (error.message.includes('kill') || error.message.includes('abort')) {
          return `[timeout after ${timeout}ms]`;
        }
        return `[error] ${error.message}`;
      }
    },
  );
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/bash-tool.test.ts`
Expected: PASS (5/5)

**Step 5: Commit**

```bash
git add src/ai/tools/host/bash.ts tests/bash-tool.test.ts
git commit -m "feat: add bash tool with timeout and output truncation"
```

---

### Task 8: Git Tools

**Files:**
- Create: `src/ai/tools/host/git.ts`
- Test: `tests/git-tools.test.ts`

Uses `Bun.spawnSync` for synchronous git command execution — no shell string interpolation.

**Step 1: Write the failing test**

```typescript
// tests/git-tools.test.ts
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { registerGitTools } from '../src/ai/tools/host/git.js';

function runGitSetup(cwd: string, ...commands: string[]): void {
  for (const cmd of commands) {
    Bun.spawnSync(['bash', '-c', cmd], { cwd });
  }
}

describe('Git Tools', () => {
  let tempDir: string;
  let registry: ReturnType<typeof createToolRegistry>;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'simse-git-'));
    runGitSetup(tempDir,
      'git init',
      'git config user.email "test@test.com"',
      'git config user.name "Test"',
    );
    await writeFile(join(tempDir, 'README.md'), '# Test');
    runGitSetup(tempDir, 'git add .', 'git commit -m "init"');
    registry = createToolRegistry({});
    registerGitTools(registry, { workingDirectory: tempDir });
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('git_status shows working tree status', async () => {
    await writeFile(join(tempDir, 'new.txt'), 'hello');
    const result = await registry.execute({ id: '1', name: 'git_status', arguments: {} });
    expect(result.isError).toBe(false);
    expect(result.output).toContain('new.txt');
  });

  it('git_diff shows changes', async () => {
    await writeFile(join(tempDir, 'README.md'), '# Changed');
    const result = await registry.execute({ id: '1', name: 'git_diff', arguments: {} });
    expect(result.isError).toBe(false);
    expect(result.output).toContain('Changed');
  });

  it('git_log shows commit history', async () => {
    const result = await registry.execute({ id: '1', name: 'git_log', arguments: {} });
    expect(result.isError).toBe(false);
    expect(result.output).toContain('init');
  });

  it('git_commit creates a commit', async () => {
    await writeFile(join(tempDir, 'new.txt'), 'hello');
    runGitSetup(tempDir, 'git add .');
    const result = await registry.execute({
      id: '1', name: 'git_commit',
      arguments: { message: 'add new file' },
    });
    expect(result.isError).toBe(false);
    const log = Bun.spawnSync(['git', 'log', '--oneline'], { cwd: tempDir });
    expect(new TextDecoder().decode(log.stdout)).toContain('add new file');
  });

  it('git_branch lists branches', async () => {
    const result = await registry.execute({ id: '1', name: 'git_branch', arguments: {} });
    expect(result.isError).toBe(false);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/git-tools.test.ts`
Expected: FAIL — module does not exist

**Step 3: Write implementation**

```typescript
// src/ai/tools/host/git.ts
import type { ToolRegistry } from '../types.js';

export interface GitToolOptions {
  readonly workingDirectory: string;
}

function runGit(args: readonly string[], cwd: string): string {
  const result = Bun.spawnSync(['git', ...args], {
    cwd,
    stdout: 'pipe',
    stderr: 'pipe',
  });
  const stdout = new TextDecoder().decode(result.stdout).trim();
  const stderr = new TextDecoder().decode(result.stderr).trim();

  if (result.exitCode !== 0) {
    throw new Error(stderr || stdout || 'Git command failed');
  }

  return stdout || stderr;
}

export function registerGitTools(registry: ToolRegistry, options: GitToolOptions): void {
  const { workingDirectory } = options;

  registry.register(
    {
      name: 'git_status',
      description: 'Show working tree status',
      parameters: {},
      category: 'read',
      annotations: { readOnly: true },
    },
    async () => runGit(['status'], workingDirectory),
  );

  registry.register(
    {
      name: 'git_diff',
      description: 'Show staged and unstaged diffs',
      parameters: {
        staged: { type: 'boolean', description: 'Show only staged changes' },
        path: { type: 'string', description: 'Limit diff to path' },
      },
      category: 'read',
      annotations: { readOnly: true },
    },
    async (args) => {
      const gitArgs = ['diff'];
      if (args.staged) gitArgs.push('--staged');
      if (typeof args.path === 'string') { gitArgs.push('--'); gitArgs.push(args.path); }
      return runGit(gitArgs, workingDirectory);
    },
  );

  registry.register(
    {
      name: 'git_log',
      description: 'Show commit history',
      parameters: {
        count: { type: 'number', description: 'Number of commits (default 10)' },
        oneline: { type: 'boolean', description: 'One-line format' },
      },
      category: 'read',
      annotations: { readOnly: true },
    },
    async (args) => {
      const count = typeof args.count === 'number' ? args.count : 10;
      const gitArgs = ['log', `-${count}`];
      if (args.oneline !== false) gitArgs.push('--oneline');
      return runGit(gitArgs, workingDirectory);
    },
  );

  registry.register(
    {
      name: 'git_commit',
      description: 'Create a commit',
      parameters: {
        message: { type: 'string', description: 'Commit message', required: true },
      },
      category: 'execute',
      annotations: { destructive: true },
    },
    async (args) => {
      return runGit(['commit', '-m', String(args.message)], workingDirectory);
    },
  );

  registry.register(
    {
      name: 'git_branch',
      description: 'List, create, or switch branches',
      parameters: {
        name: { type: 'string', description: 'Branch name to create or switch to' },
        create: { type: 'boolean', description: 'Create new branch' },
      },
      category: 'execute',
    },
    async (args) => {
      if (typeof args.name === 'string') {
        if (args.create) return runGit(['checkout', '-b', args.name], workingDirectory);
        return runGit(['checkout', args.name], workingDirectory);
      }
      return runGit(['branch', '-v'], workingDirectory);
    },
  );
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/git-tools.test.ts`
Expected: PASS (5/5)

**Step 5: Commit**

```bash
git add src/ai/tools/host/git.ts tests/git-tools.test.ts
git commit -m "feat: add git tools (status, diff, log, commit, branch)"
```

---

### Task 9: Event Bus Integration into Agentic Loop

**Files:**
- Modify: `src/ai/loop/types.ts` — add optional `eventBus` to `AgenticLoopOptions`
- Modify: `src/ai/loop/agentic-loop.ts` — publish events during loop
- Test: `tests/loop-events.test.ts`

**Step 1: Write the failing test**

```typescript
// tests/loop-events.test.ts
import { describe, expect, it, mock } from 'bun:test';
import { createEventBus } from '../src/events/event-bus.js';
import type { EventType } from '../src/events/types.js';

describe('Loop EventBus integration', () => {
  it('AgenticLoopOptions accepts eventBus', async () => {
    // Type check — verify the import compiles
    const { createAgenticLoop } = await import('../src/ai/loop/agentic-loop.js');
    const bus = createEventBus();
    expect(typeof createAgenticLoop).toBe('function');
    expect(typeof bus.publish).toBe('function');
  });

  it('EventBus collects events from subscribeAll', () => {
    const bus = createEventBus();
    const events: EventType[] = [];
    bus.subscribeAll((type) => { events.push(type); });

    bus.publish('stream.delta', { text: 'hi' });
    bus.publish('tool.call.start', { callId: '1', name: 'test', args: {} });
    bus.publish('turn.complete', { turn: 1, type: 'tool_use' });

    expect(events).toEqual(['stream.delta', 'tool.call.start', 'turn.complete']);
  });

  it('LoopCallbacks and EventBus can coexist', () => {
    const bus = createEventBus();
    const busEvents: string[] = [];
    bus.subscribeAll((type) => { busEvents.push(type); });

    // Simulate what the loop does: publish to bus
    bus.publish('stream.delta', { text: 'hello' });
    expect(busEvents).toContain('stream.delta');
  });
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/loop-events.test.ts`
Expected: PASS or FAIL depending on Task 1 completion

**Step 3: Add eventBus to AgenticLoopOptions**

In `src/ai/loop/types.ts`, add import and field:
```typescript
import type { EventBus } from '../../events/types.js';

// In AgenticLoopOptions:
readonly eventBus?: EventBus;
```

**Step 4: Integrate EventBus publishing in agentic-loop.ts**

At key points in `createAgenticLoop`, add `eventBus?.publish(...)` calls alongside existing callbacks:
- On stream delta: `eventBus?.publish('stream.delta', { text })`
- On tool call start: `eventBus?.publish('tool.call.start', { callId, name, args })`
- On tool call end: `eventBus?.publish('tool.call.end', { callId, name, output, isError, durationMs })`
- On turn complete: `eventBus?.publish('turn.complete', { turn, type })`
- On compaction: `eventBus?.publish('compaction.start', ...)` and `eventBus?.publish('compaction.complete', ...)`

**Step 5: Run test to verify it passes**

Run: `bun test tests/loop-events.test.ts`
Expected: PASS (3/3)

**Step 6: Commit**

```bash
git add src/ai/loop/types.ts src/ai/loop/agentic-loop.ts tests/loop-events.test.ts
git commit -m "feat: integrate event bus into agentic loop"
```

---

### Task 10: Provider Prompts & Instruction Discovery

**Files:**
- Create: `src/ai/prompts/provider-prompts.ts`
- Create: `src/ai/prompts/instruction-discovery.ts`
- Create: `src/ai/prompts/types.ts`
- Create: `src/ai/prompts/index.ts`
- Test: `tests/provider-prompts.test.ts`
- Test: `tests/instruction-discovery.test.ts`

**Step 1: Write the failing tests**

```typescript
// tests/provider-prompts.test.ts
import { describe, expect, it } from 'bun:test';
import { createProviderPromptResolver } from '../src/ai/prompts/provider-prompts.js';

describe('ProviderPromptResolver', () => {
  it('matches exact provider pattern', () => {
    const resolver = createProviderPromptResolver({
      prompts: { 'anthropic/*': 'You are Claude.' },
    });
    expect(resolver.resolve('anthropic/claude-3')).toBe('You are Claude.');
  });

  it('falls back to default prompt', () => {
    const resolver = createProviderPromptResolver({
      prompts: { 'anthropic/*': 'Claude prompt' },
      defaultPrompt: 'Default prompt',
    });
    expect(resolver.resolve('openai/gpt-4')).toBe('Default prompt');
  });

  it('returns empty string when no match and no default', () => {
    const resolver = createProviderPromptResolver({ prompts: {} });
    expect(resolver.resolve('unknown/model')).toBe('');
  });

  it('matches wildcard patterns', () => {
    const resolver = createProviderPromptResolver({
      prompts: { 'openai/*': 'OpenAI prompt', 'anthropic/*': 'Anthropic prompt' },
    });
    expect(resolver.resolve('openai/gpt-4o')).toBe('OpenAI prompt');
    expect(resolver.resolve('anthropic/claude-opus')).toBe('Anthropic prompt');
  });
});
```

```typescript
// tests/instruction-discovery.test.ts
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtemp, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { discoverInstructions } from '../src/ai/prompts/instruction-discovery.js';

describe('InstructionDiscovery', () => {
  let tempDir: string;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'simse-instr-'));
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('finds CLAUDE.md in root', async () => {
    await writeFile(join(tempDir, 'CLAUDE.md'), '# Instructions\nDo things.');
    const instructions = await discoverInstructions({ rootDir: tempDir });
    expect(instructions.length).toBe(1);
    expect(instructions[0].content).toContain('Do things');
  });

  it('finds AGENTS.md', async () => {
    await writeFile(join(tempDir, 'AGENTS.md'), '# Agent instructions');
    const instructions = await discoverInstructions({ rootDir: tempDir });
    expect(instructions.length).toBe(1);
  });

  it('finds .simse/instructions.md', async () => {
    await mkdir(join(tempDir, '.simse'), { recursive: true });
    await writeFile(join(tempDir, '.simse', 'instructions.md'), 'Custom instructions');
    const instructions = await discoverInstructions({ rootDir: tempDir });
    expect(instructions.length).toBe(1);
  });

  it('returns empty array when no files found', async () => {
    const instructions = await discoverInstructions({ rootDir: tempDir });
    expect(instructions).toEqual([]);
  });

  it('supports custom patterns', async () => {
    await writeFile(join(tempDir, 'CUSTOM.md'), 'Custom');
    const instructions = await discoverInstructions({
      rootDir: tempDir,
      patterns: ['CUSTOM.md'],
    });
    expect(instructions.length).toBe(1);
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test tests/provider-prompts.test.ts tests/instruction-discovery.test.ts`
Expected: FAIL — modules do not exist

**Step 3: Write implementations**

```typescript
// src/ai/prompts/types.ts
export interface ProviderPromptConfig {
  readonly prompts?: Readonly<Record<string, string>>;
  readonly defaultPrompt?: string;
}

export interface InstructionDiscoveryOptions {
  readonly patterns?: readonly string[];
  readonly rootDir: string;
}

export interface DiscoveredInstruction {
  readonly path: string;
  readonly content: string;
}

export interface ProviderPromptResolver {
  readonly resolve: (modelId: string) => string;
}
```

```typescript
// src/ai/prompts/provider-prompts.ts
import type { ProviderPromptConfig, ProviderPromptResolver } from './types.js';

function globMatch(pattern: string, value: string): boolean {
  const regex = new RegExp(
    '^' + pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*') + '$',
  );
  return regex.test(value);
}

export function createProviderPromptResolver(config: ProviderPromptConfig): ProviderPromptResolver {
  const { prompts = {}, defaultPrompt = '' } = config;

  const resolve = (modelId: string): string => {
    for (const [pattern, prompt] of Object.entries(prompts)) {
      if (globMatch(pattern, modelId)) return prompt;
    }
    return defaultPrompt;
  };

  return Object.freeze({ resolve });
}
```

```typescript
// src/ai/prompts/instruction-discovery.ts
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { DiscoveredInstruction, InstructionDiscoveryOptions } from './types.js';

const DEFAULT_PATTERNS = ['CLAUDE.md', 'AGENTS.md', '.simse/instructions.md'];

export async function discoverInstructions(
  options: InstructionDiscoveryOptions,
): Promise<readonly DiscoveredInstruction[]> {
  const { rootDir, patterns = DEFAULT_PATTERNS } = options;
  const instructions: DiscoveredInstruction[] = [];

  for (const pattern of patterns) {
    const filePath = join(rootDir, pattern);
    try {
      const content = await readFile(filePath, 'utf-8');
      instructions.push(Object.freeze({ path: filePath, content }));
    } catch {
      // File does not exist — skip
    }
  }

  return Object.freeze(instructions);
}
```

```typescript
// src/ai/prompts/index.ts
export { createProviderPromptResolver } from './provider-prompts.js';
export { discoverInstructions } from './instruction-discovery.js';
export type {
  DiscoveredInstruction,
  InstructionDiscoveryOptions,
  ProviderPromptConfig,
  ProviderPromptResolver,
} from './types.js';
```

**Step 4: Run tests to verify they pass**

Run: `bun test tests/provider-prompts.test.ts tests/instruction-discovery.test.ts`
Expected: PASS (4/4 + 5/5)

**Step 5: Commit**

```bash
git add src/ai/prompts/ tests/provider-prompts.test.ts tests/instruction-discovery.test.ts
git commit -m "feat: add provider prompts and instruction file discovery"
```

---

### Task 11: HTTP+SSE Server (Hono)

**Files:**
- Create: `src/server/server.ts`
- Create: `src/server/types.ts`
- Create: `src/server/session-manager.ts`
- Create: `src/server/index.ts`
- Modify: `package.json` — add `hono` dependency
- Test: `tests/server.test.ts`

**Step 1: Install Hono**

Run: `bun add hono`

**Step 2: Write the failing test**

```typescript
// tests/server.test.ts
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { createSimseServer } from '../src/server/server.js';

describe('SimseServer', () => {
  let server: Awaited<ReturnType<typeof createSimseServer>>;

  beforeEach(async () => {
    server = createSimseServer({
      port: 0, // random port
      workingDirectory: process.cwd(),
      acpServers: [],
    });
    await server.start();
  });

  afterEach(async () => {
    await server.stop();
  });

  it('responds to health check', async () => {
    const res = await fetch(`${server.url}/health`);
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.status).toBe('ok');
  });

  it('creates a session', async () => {
    const res = await fetch(`${server.url}/sessions`, { method: 'POST' });
    expect(res.status).toBe(201);
    const body = await res.json();
    expect(body.sessionId).toBeTruthy();
  });

  it('gets session state', async () => {
    const create = await fetch(`${server.url}/sessions`, { method: 'POST' });
    const { sessionId } = await create.json();
    const res = await fetch(`${server.url}/sessions/${sessionId}`);
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.status).toBe('active');
  });

  it('returns 404 for unknown session', async () => {
    const res = await fetch(`${server.url}/sessions/nonexistent`);
    expect(res.status).toBe(404);
  });

  it('deletes a session', async () => {
    const create = await fetch(`${server.url}/sessions`, { method: 'POST' });
    const { sessionId } = await create.json();
    const del = await fetch(`${server.url}/sessions/${sessionId}`, { method: 'DELETE' });
    expect(del.status).toBe(200);
    const get = await fetch(`${server.url}/sessions/${sessionId}`);
    expect(get.status).toBe(404);
  });

  it('lists available tools', async () => {
    const res = await fetch(`${server.url}/tools`);
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(Array.isArray(body.tools)).toBe(true);
  });
});
```

**Step 3: Write session manager**

```typescript
// src/server/session-manager.ts
import { createConversation } from '../ai/conversation/conversation.js';
import { createEventBus } from '../events/event-bus.js';
import type { Conversation } from '../ai/conversation/types.js';
import type { EventBus } from '../events/types.js';

export interface Session {
  readonly id: string;
  readonly conversation: Conversation;
  readonly eventBus: EventBus;
  readonly status: 'active' | 'completed' | 'aborted';
  readonly createdAt: number;
}

export interface SessionManager {
  readonly create: () => Session;
  readonly get: (id: string) => Session | undefined;
  readonly delete: (id: string) => boolean;
  readonly list: () => readonly Session[];
}

export function createSessionManager(): SessionManager {
  const sessions = new Map<string, Session>();
  let counter = 0;

  const create = (): Session => {
    const id = `session_${++counter}_${Date.now()}`;
    const session: Session = Object.freeze({
      id,
      conversation: createConversation(),
      eventBus: createEventBus(),
      status: 'active' as const,
      createdAt: Date.now(),
    });
    sessions.set(id, session);
    return session;
  };

  const get = (id: string): Session | undefined => sessions.get(id);

  const del = (id: string): boolean => sessions.delete(id);

  const list = (): readonly Session[] => Object.freeze([...sessions.values()]);

  return Object.freeze({ create, get, delete: del, list });
}
```

**Step 4: Write server types**

```typescript
// src/server/types.ts
export interface ACPServerEntry {
  readonly name: string;
  readonly command: string;
  readonly args?: readonly string[];
}

export interface SimseServerConfig {
  readonly port?: number;
  readonly host?: string;
  readonly acpServers: readonly ACPServerEntry[];
  readonly mcpServers?: readonly unknown[];
  readonly workingDirectory: string;
}

export interface SimseServer {
  readonly start: () => Promise<void>;
  readonly stop: () => Promise<void>;
  readonly port: number;
  readonly url: string;
}
```

**Step 5: Write server**

```typescript
// src/server/server.ts
import { Hono } from 'hono';
import { createToolRegistry } from '../ai/tools/tool-registry.js';
import { createSessionManager } from './session-manager.js';
import type { SimseServer, SimseServerConfig } from './types.js';

export function createSimseServer(config: SimseServerConfig): SimseServer {
  const { port: configPort = 0, host = '127.0.0.1', workingDirectory } = config;

  const sessions = createSessionManager();
  const toolRegistry = createToolRegistry({});
  const app = new Hono();

  // Health
  app.get('/health', (c) => c.json({ status: 'ok', timestamp: Date.now() }));

  // Sessions
  app.post('/sessions', (c) => {
    const session = sessions.create();
    return c.json({ sessionId: session.id }, 201);
  });

  app.get('/sessions/:id', (c) => {
    const session = sessions.get(c.req.param('id'));
    if (!session) return c.json({ error: 'Session not found' }, 404);
    return c.json({
      sessionId: session.id,
      status: session.status,
      createdAt: session.createdAt,
      messageCount: session.conversation.messageCount,
    });
  });

  app.delete('/sessions/:id', (c) => {
    const deleted = sessions.delete(c.req.param('id'));
    if (!deleted) return c.json({ error: 'Session not found' }, 404);
    return c.json({ deleted: true });
  });

  // SSE events
  app.get('/sessions/:id/events', (c) => {
    const session = sessions.get(c.req.param('id'));
    if (!session) return c.json({ error: 'Session not found' }, 404);

    return new Response(
      new ReadableStream({
        start(controller) {
          const encoder = new TextEncoder();
          const unsub = session.eventBus.subscribeAll((type, payload) => {
            const data = JSON.stringify({ type, payload });
            controller.enqueue(encoder.encode(`data: ${data}\n\n`));
          });

          // Clean up on close
          c.req.raw.signal.addEventListener('abort', () => {
            unsub();
            controller.close();
          });
        },
      }),
      {
        headers: {
          'Content-Type': 'text/event-stream',
          'Cache-Control': 'no-cache',
          Connection: 'keep-alive',
        },
      },
    );
  });

  // Tools
  app.get('/tools', (c) => {
    const tools = toolRegistry.getToolDefinitions();
    return c.json({ tools });
  });

  // Agents
  app.get('/agents', (c) => {
    return c.json({ agents: config.acpServers });
  });

  let bunServer: ReturnType<typeof Bun.serve> | null = null;
  let actualPort = configPort;

  const start = async (): Promise<void> => {
    bunServer = Bun.serve({
      port: configPort,
      hostname: host,
      fetch: app.fetch,
    });
    actualPort = bunServer.port;
  };

  const stop = async (): Promise<void> => {
    bunServer?.stop();
    bunServer = null;
  };

  return Object.freeze({
    start,
    stop,
    get port() { return actualPort; },
    get url() { return `http://${host}:${actualPort}`; },
  });
}
```

```typescript
// src/server/index.ts
export { createSimseServer } from './server.js';
export { createSessionManager } from './session-manager.js';
export type { ACPServerEntry, SimseServer, SimseServerConfig } from './types.js';
export type { Session, SessionManager } from './session-manager.js';
```

**Step 6: Run test to verify it passes**

Run: `bun test tests/server.test.ts`
Expected: PASS (6/6)

**Step 7: Commit**

```bash
git add src/server/ tests/server.test.ts package.json bun.lockb
git commit -m "feat: add headless HTTP+SSE server with Hono"
```

---

### Task 12: Export All New Modules from lib.ts

**Files:**
- Modify: `src/lib.ts`

**Step 1: Add exports**

Add to `src/lib.ts`:
```typescript
// Events
export { createEventBus } from './events/index.js';
export type { EventBus, EventHandler, EventPayload, EventPayloadMap, EventType } from './events/index.js';

// Hooks
export { createHookSystem } from './hooks/index.js';
export type { BlockedResult, HookContextMap, HookHandler, HookResultMap, HookSystem, HookType } from './hooks/index.js';

// Host Tools
export { registerFilesystemTools } from './ai/tools/host/filesystem.js';
export { registerBashTool } from './ai/tools/host/bash.js';
export { registerGitTools } from './ai/tools/host/git.js';
export type { FilesystemToolOptions } from './ai/tools/host/filesystem.js';
export type { BashToolOptions } from './ai/tools/host/bash.js';
export type { GitToolOptions } from './ai/tools/host/git.js';

// Fuzzy Edit
export { fuzzyMatch } from './ai/tools/host/fuzzy-edit.js';
export type { FuzzyMatchResult } from './ai/tools/host/fuzzy-edit.js';

// Permissions
export { createToolPermissionResolver } from './ai/tools/permissions.js';
export type { ToolPermissionConfig, ToolPermissionRule } from './ai/tools/permissions.js';

// Context Pruning
export { createContextPruner } from './ai/conversation/context-pruner.js';
export type { ContextPruner, ContextPrunerOptions } from './ai/conversation/context-pruner.js';

// Provider Prompts & Instructions
export { createProviderPromptResolver, discoverInstructions } from './ai/prompts/index.js';
export type {
  DiscoveredInstruction,
  InstructionDiscoveryOptions,
  ProviderPromptConfig,
  ProviderPromptResolver,
} from './ai/prompts/index.js';

// Server
export { createSimseServer } from './server/index.js';
export type { ACPServerEntry, SimseServer, SimseServerConfig } from './server/index.js';
export type { Session, SessionManager } from './server/index.js';
```

**Step 2: Verify typecheck**

Run: `bun x tsc --noEmit`
Expected: 0 errors

**Step 3: Commit**

```bash
git add src/lib.ts
git commit -m "feat: export all new agent server modules from public API"
```

---

### Task 13: Integration Test

**Files:**
- Create: `tests/agent-server-integration.test.ts`

**Step 1: Write integration test**

```typescript
// tests/agent-server-integration.test.ts
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { createEventBus } from '../src/events/event-bus.js';
import { createHookSystem } from '../src/hooks/hook-system.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { registerFilesystemTools } from '../src/ai/tools/host/filesystem.js';
import { registerBashTool } from '../src/ai/tools/host/bash.js';
import { createToolPermissionResolver } from '../src/ai/tools/permissions.js';
import { createContextPruner } from '../src/ai/conversation/context-pruner.js';
import { createProviderPromptResolver, discoverInstructions } from '../src/ai/prompts/index.js';
import { fuzzyMatch } from '../src/ai/tools/host/fuzzy-edit.js';

describe('Agent Server Integration', () => {
  let tempDir: string;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'simse-integration-'));
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('event bus + tool registry + hooks work together', async () => {
    const bus = createEventBus();
    const hooks = createHookSystem();
    const registry = createToolRegistry({});
    registerFilesystemTools(registry, { workingDirectory: tempDir });

    const events: string[] = [];
    bus.subscribeAll((type) => events.push(type));

    // Hook that logs tool calls via event bus
    hooks.register('tool.execute.before', async (ctx) => {
      bus.publish('tool.call.start', {
        callId: ctx.request.id,
        name: ctx.request.name,
        args: ctx.request.arguments,
      });
      return ctx.request;
    });

    // Simulate tool execution with hook
    const request = { id: '1', name: 'fs_write', arguments: { path: 'test.txt', content: 'hello' } };
    const hookResult = await hooks.run('tool.execute.before', { request });
    expect(hookResult).toBeDefined();

    const result = await registry.execute(request);
    expect(result.isError).toBe(false);
    expect(events).toContain('tool.call.start');
  });

  it('permission resolver gates tool execution', async () => {
    const resolver = createToolPermissionResolver({
      defaultPolicy: 'deny',
      rules: [
        { tool: 'fs_read', policy: 'allow' },
        { tool: 'fs_write', policy: 'deny' },
      ],
    });

    expect(await resolver.check({ id: '1', name: 'fs_read', arguments: {} })).toBe(true);
    expect(await resolver.check({ id: '2', name: 'fs_write', arguments: {} })).toBe(false);
  });

  it('batch execution runs tools concurrently', async () => {
    const registry = createToolRegistry({});
    registerFilesystemTools(registry, { workingDirectory: tempDir });
    await writeFile(join(tempDir, 'a.txt'), 'content a');
    await writeFile(join(tempDir, 'b.txt'), 'content b');

    const results = await registry.batchExecute([
      { id: '1', name: 'fs_read', arguments: { path: 'a.txt' } },
      { id: '2', name: 'fs_read', arguments: { path: 'b.txt' } },
    ]);

    expect(results).toHaveLength(2);
    expect(results[0].output).toContain('content a');
    expect(results[1].output).toContain('content b');
  });

  it('context pruner reduces conversation size', () => {
    const pruner = createContextPruner({ protectRecentTurns: 1 });
    const messages = Array.from({ length: 10 }, (_, i) => ([
      { role: 'user' as const, content: `Q${i}`, timestamp: Date.now() - (10 - i) * 1000 },
      { role: 'tool_result' as const, content: 'x'.repeat(5000), toolCallId: `t${i}`, toolName: `tool${i}` },
    ])).flat();

    const pruned = pruner.prune(messages);
    const totalSize = pruned.reduce((sum, m) => sum + m.content.length, 0);
    const originalSize = messages.reduce((sum, m) => sum + m.content.length, 0);
    expect(totalSize).toBeLessThan(originalSize);
  });

  it('provider prompts resolve correctly', () => {
    const resolver = createProviderPromptResolver({
      prompts: {
        'anthropic/*': 'Claude mode',
        'openai/*': 'GPT mode',
      },
      defaultPrompt: 'Generic mode',
    });
    expect(resolver.resolve('anthropic/claude-3')).toBe('Claude mode');
    expect(resolver.resolve('openai/gpt-4')).toBe('GPT mode');
    expect(resolver.resolve('mistral/large')).toBe('Generic mode');
  });

  it('instruction discovery finds project files', async () => {
    await writeFile(join(tempDir, 'CLAUDE.md'), '# Project\nBuild things.');
    const instructions = await discoverInstructions({ rootDir: tempDir });
    expect(instructions.length).toBe(1);
    expect(instructions[0].content).toContain('Build things');
  });

  it('fuzzy edit handles real-world code edits', () => {
    const code = `function greet(name) {
  console.log("Hello " + name);
  return true;
}
`;
    const result = fuzzyMatch(
      code,
      'console.log("Hello " + name);',
      'console.log(\`Hello \${name}\`);',
    );
    expect(result).not.toBeNull();
    expect(result!.replaced).toContain('`Hello ${name}`');
  });

  it('bash tool runs in working directory', async () => {
    const registry = createToolRegistry({});
    registerBashTool(registry, { workingDirectory: tempDir });
    await writeFile(join(tempDir, 'marker.txt'), 'found');

    const result = await registry.execute({
      id: '1', name: 'bash',
      arguments: { command: 'cat marker.txt' },
    });
    expect(result.isError).toBe(false);
    expect(result.output).toContain('found');
  });

  it('end-to-end: write, edit, read a file through tools', async () => {
    const registry = createToolRegistry({});
    registerFilesystemTools(registry, { workingDirectory: tempDir });

    // Write
    await registry.execute({
      id: '1', name: 'fs_write',
      arguments: { path: 'hello.ts', content: 'const msg = "hello";\nconsole.log(msg);\n' },
    });

    // Edit
    await registry.execute({
      id: '2', name: 'fs_edit',
      arguments: { path: 'hello.ts', old_string: '"hello"', new_string: '"world"' },
    });

    // Read
    const result = await registry.execute({
      id: '3', name: 'fs_read',
      arguments: { path: 'hello.ts' },
    });
    expect(result.output).toContain('"world"');
    expect(result.output).not.toContain('"hello"');
  });
});
```

**Step 2: Run test to verify it passes**

Run: `bun test tests/agent-server-integration.test.ts`
Expected: PASS (9/9)

**Step 3: Run all tests**

Run: `bun test`
Expected: All tests pass (previous 1193 + new tests)

**Step 4: Run typecheck**

Run: `bun x tsc --noEmit`
Expected: 0 errors

**Step 5: Commit**

```bash
git add tests/agent-server-integration.test.ts
git commit -m "test: add agent server integration tests"
```
