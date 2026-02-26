# Loop/Stream UX Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add token tracking, tool timeouts, tool metrics, stream cancellation, category-based permissions, usage callbacks, and context window usage percentage to simse's agentic loop and tool infrastructure.

**Architecture:** Seven additive changes to existing modules. No new files except tests. Each part extends existing types and implementations. All changes are non-breaking — new fields are optional, new methods are additive.

**Tech Stack:** TypeScript, Bun test runner, Biome linter. Factory pattern with `Object.freeze()`. ESM-only with `.js` extensions and `import type` for types.

---

### Task 1: Context Window Usage Percentage

**Files:**
- Modify: `src/ai/conversation/types.ts:15-24` (ConversationOptions)
- Modify: `src/ai/conversation/types.ts:26-47` (Conversation interface)
- Modify: `src/ai/conversation/conversation.ts:19-157`
- Modify: `tests/conversation-replace.test.ts`

**Step 1: Add tests**

Add to `tests/conversation-replace.test.ts`:

```typescript
describe('conversation contextUsagePercent', () => {
	it('returns 0 when contextWindowTokens is not configured', () => {
		const conv = createConversation();
		conv.addUser('hello');
		expect(conv.contextUsagePercent).toBe(0);
	});

	it('returns percentage based on estimatedTokens and contextWindowTokens', () => {
		const conv = createConversation({ contextWindowTokens: 100 });
		// 400 chars = ~100 tokens at default estimator
		conv.addUser('x'.repeat(400));
		expect(conv.contextUsagePercent).toBe(100);
	});

	it('caps at 100 percent', () => {
		const conv = createConversation({ contextWindowTokens: 10 });
		conv.addUser('x'.repeat(1000));
		expect(conv.contextUsagePercent).toBe(100);
	});

	it('tracks partial usage', () => {
		const conv = createConversation({ contextWindowTokens: 1000 });
		// 100 chars = ~25 tokens => 25/1000 = 3%
		conv.addUser('x'.repeat(100));
		expect(conv.contextUsagePercent).toBe(3);
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test tests/conversation-replace.test.ts`
Expected: FAIL — `contextUsagePercent` is not a property

**Step 3: Implement**

In `src/ai/conversation/types.ts`, add to `ConversationOptions`:
```typescript
readonly contextWindowTokens?: number;
```

Add to `Conversation` interface:
```typescript
readonly contextUsagePercent: number;
```

In `src/ai/conversation/conversation.ts`, capture the option:
```typescript
const contextWindowTokens = options?.contextWindowTokens;
```

Add getter to frozen return:
```typescript
get contextUsagePercent() {
	if (!contextWindowTokens || contextWindowTokens <= 0) return 0;
	const tokens = tokenEstimator
		? (() => {
				let total = 0;
				if (systemPrompt) total += tokenEstimator(systemPrompt);
				for (const msg of messages) total += tokenEstimator(msg.content);
				return total;
			})()
		: Math.ceil(estimateChars() / 4);
	return Math.min(100, Math.round((tokens / contextWindowTokens) * 100));
},
```

Note: to avoid duplicating the token estimation logic, refactor the existing `estimatedTokens` getter and this new getter to share a common `estimateTokens()` internal function.

**Step 4: Run tests**

Run: `bun test tests/conversation-replace.test.ts`
Expected: All PASS

**Step 5: Typecheck and lint**

Run: `bun x tsc --noEmit && bun run lint`

**Step 6: Commit**

```
feat: add contextUsagePercent to Conversation
```

---

### Task 2: Tool Execution Timeout

**Files:**
- Modify: `src/ai/tools/types.ts:39-45` (ToolDefinition)
- Modify: `src/ai/tools/types.ts:88-94` (ToolRegistryOptions)
- Modify: `src/ai/tools/tool-registry.ts:201-245` (execute method)
- Create: `tests/tool-timeout.test.ts`

**Step 1: Write tests**

Create `tests/tool-timeout.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { createSilentLogger } from './utils/mocks.js';

describe('tool execution timeout', () => {
	it('times out a slow tool with per-tool timeout', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registry.register(
			{
				name: 'slow_tool',
				description: 'A slow tool',
				parameters: {},
				timeoutMs: 50,
			},
			async () => {
				await new Promise((r) => setTimeout(r, 5000));
				return 'done';
			},
		);

		const result = await registry.execute({
			id: 'c1',
			name: 'slow_tool',
			arguments: {},
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('timed out');
	});

	it('times out with global default timeout', async () => {
		const registry = createToolRegistry({
			logger: createSilentLogger(),
			defaultToolTimeoutMs: 50,
		});
		registry.register(
			{
				name: 'slow_tool',
				description: 'A slow tool',
				parameters: {},
			},
			async () => {
				await new Promise((r) => setTimeout(r, 5000));
				return 'done';
			},
		);

		const result = await registry.execute({
			id: 'c2',
			name: 'slow_tool',
			arguments: {},
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('timed out');
	});

	it('per-tool timeout overrides global default', async () => {
		const registry = createToolRegistry({
			logger: createSilentLogger(),
			defaultToolTimeoutMs: 5000,
		});
		registry.register(
			{
				name: 'slow_tool',
				description: 'A slow tool',
				parameters: {},
				timeoutMs: 50,
			},
			async () => {
				await new Promise((r) => setTimeout(r, 5000));
				return 'done';
			},
		);

		const result = await registry.execute({
			id: 'c3',
			name: 'slow_tool',
			arguments: {},
		});

		expect(result.isError).toBe(true);
	});

	it('fast tool completes normally with timeout configured', async () => {
		const registry = createToolRegistry({
			logger: createSilentLogger(),
			defaultToolTimeoutMs: 5000,
		});
		registry.register(
			{
				name: 'fast_tool',
				description: 'A fast tool',
				parameters: {},
			},
			async () => 'quick result',
		);

		const result = await registry.execute({
			id: 'c4',
			name: 'fast_tool',
			arguments: {},
		});

		expect(result.isError).toBe(false);
		expect(result.output).toBe('quick result');
	});
});
```

**Step 2: Run tests to verify failure**

Run: `bun test tests/tool-timeout.test.ts`
Expected: FAIL — `timeoutMs` not recognized, no timeout behavior

**Step 3: Implement**

In `src/ai/tools/types.ts`, add to `ToolDefinition`:
```typescript
readonly timeoutMs?: number;
```

Add to `ToolRegistryOptions`:
```typescript
readonly defaultToolTimeoutMs?: number;
```

In `src/ai/tools/tool-registry.ts`, add import:
```typescript
import { withTimeout } from '../../utils/timeout.js';
```

Capture the default:
```typescript
const { mcpClient, memoryManager, vfs, permissionResolver, defaultToolTimeoutMs } = options;
```

In `execute()`, wrap the handler call:
```typescript
const timeoutMs = registered.definition.timeoutMs ?? defaultToolTimeoutMs;
const handlerPromise = () => registered.handler(call.arguments);
const output = timeoutMs
	? await withTimeout(handlerPromise, timeoutMs, { operation: `tool:${call.name}` })
	: await registered.handler(call.arguments);
```

The existing catch block already handles errors from `withTimeout` (it throws `createTimeoutError` which extends Error).

**Step 4: Run tests**

Run: `bun test tests/tool-timeout.test.ts`
Expected: All PASS

**Step 5: Run full suite, typecheck, lint**

Run: `bun x tsc --noEmit && bun test && bun run lint`

**Step 6: Commit**

```
feat: add tool execution timeout (per-tool and global default)
```

---

### Task 3: Tool Execution Metrics

**Files:**
- Modify: `src/ai/tools/types.ts` (add ToolMetrics, extend ToolRegistry)
- Modify: `src/ai/tools/tool-registry.ts` (track metrics in execute)
- Modify: `src/lib.ts` (export ToolMetrics)
- Create: `tests/tool-metrics.test.ts`

**Step 1: Write tests**

Create `tests/tool-metrics.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { createSilentLogger } from './utils/mocks.js';

describe('tool execution metrics', () => {
	it('returns empty metrics for unknown tool', () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		const metrics = registry.getToolMetrics('nonexistent');
		expect(metrics).toBeUndefined();
	});

	it('tracks call count after successful execution', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registry.register(
			{ name: 'test_tool', description: 'test', parameters: {} },
			async () => 'ok',
		);

		await registry.execute({ id: 'c1', name: 'test_tool', arguments: {} });
		await registry.execute({ id: 'c2', name: 'test_tool', arguments: {} });

		const metrics = registry.getToolMetrics('test_tool');
		expect(metrics).toBeDefined();
		expect(metrics!.callCount).toBe(2);
		expect(metrics!.errorCount).toBe(0);
	});

	it('tracks error count', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registry.register(
			{ name: 'fail_tool', description: 'fails', parameters: {} },
			async () => { throw new Error('boom'); },
		);

		await registry.execute({ id: 'c1', name: 'fail_tool', arguments: {} });

		const metrics = registry.getToolMetrics('fail_tool');
		expect(metrics!.callCount).toBe(1);
		expect(metrics!.errorCount).toBe(1);
	});

	it('tracks duration', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registry.register(
			{ name: 'timed_tool', description: 'timed', parameters: {} },
			async () => {
				await new Promise((r) => setTimeout(r, 10));
				return 'ok';
			},
		);

		await registry.execute({ id: 'c1', name: 'timed_tool', arguments: {} });

		const metrics = registry.getToolMetrics('timed_tool');
		expect(metrics!.totalDurationMs).toBeGreaterThan(0);
		expect(metrics!.avgDurationMs).toBeGreaterThan(0);
	});

	it('getAllToolMetrics returns all tracked tools', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		registry.register(
			{ name: 'tool_a', description: 'a', parameters: {} },
			async () => 'a',
		);
		registry.register(
			{ name: 'tool_b', description: 'b', parameters: {} },
			async () => 'b',
		);

		await registry.execute({ id: 'c1', name: 'tool_a', arguments: {} });
		await registry.execute({ id: 'c2', name: 'tool_b', arguments: {} });

		const allMetrics = registry.getAllToolMetrics();
		expect(allMetrics).toHaveLength(2);
	});
});
```

**Step 2: Run tests to verify failure**

Run: `bun test tests/tool-metrics.test.ts`
Expected: FAIL

**Step 3: Implement**

In `src/ai/tools/types.ts`, add:
```typescript
export interface ToolMetrics {
	readonly name: string;
	readonly callCount: number;
	readonly errorCount: number;
	readonly totalDurationMs: number;
	readonly avgDurationMs: number;
	readonly lastCalledAt: number;
}
```

Add to `ToolRegistry`:
```typescript
readonly getToolMetrics: (name: string) => ToolMetrics | undefined;
readonly getAllToolMetrics: () => readonly ToolMetrics[];
```

In `src/ai/tools/tool-registry.ts`, add a metrics Map and update in `execute()`:
```typescript
const metrics = new Map<string, { calls: number; errors: number; totalMs: number; lastAt: number }>();
```

After each `execute()` call, update the map. Build frozen `ToolMetrics` objects when queried.

Export `ToolMetrics` from `src/lib.ts`.

**Step 4: Run tests, typecheck, lint**

Run: `bun x tsc --noEmit && bun test && bun run lint`

**Step 5: Commit**

```
feat: add tool execution metrics tracking
```

---

### Task 4: Streaming Cancellation via AbortSignal

**Files:**
- Modify: `src/ai/acp/acp-client.ts:78-86` (ACPStreamOptions — already has type import)
- Modify: `src/ai/acp/acp-client.ts:615-811` (generateStream)

**Step 1: Implement**

In `ACPStreamOptions`, signal already lives on the loop level. Add to `ACPStreamOptions`:
```typescript
readonly signal?: AbortSignal;
```

In `generateStream()`, after the chunk is consumed from `chunks[idx++]`:
```typescript
// Check abort signal
if (streamOptions?.signal?.aborted) {
	yield { type: 'complete', usage: streamUsage };
	return;
}
```

Add the check in the `else` branch (waiting for chunks) as well, right before the timeout check.

**Step 2: Write test**

Add a test in `tests/acp-stream-chunks.test.ts`:

```typescript
it('ACPStreamOptions accepts signal field', () => {
	const controller = new AbortController();
	const options: import('../src/ai/acp/acp-client.js').ACPStreamOptions = {
		signal: controller.signal,
	};
	expect(options.signal).toBeDefined();
});
```

**Step 3: Run tests, typecheck, lint**

Run: `bun x tsc --noEmit && bun test && bun run lint`

**Step 4: Commit**

```
feat: add AbortSignal support to generateStream
```

---

### Task 5: Category-Based Permission Filtering

**Files:**
- Modify: `src/ai/tools/types.ts:80-82` (ToolPermissionResolver)
- Modify: `src/ai/tools/permissions.ts`
- Modify: `src/ai/tools/tool-registry.ts:201-245` (execute passes definition)
- Modify: `tests/tool-permissions.test.ts`

**Step 1: Write tests**

Add to `tests/tool-permissions.test.ts`:

```typescript
describe('category-based permission rules', () => {
	it('allows all tools in a category', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'deny',
			rules: [{ tool: '*', category: 'read', policy: 'allow' }],
		});

		const readDef = { name: 'fs_read', description: '', parameters: {}, category: 'read' as const };
		const editDef = { name: 'fs_write', description: '', parameters: {}, category: 'edit' as const };

		expect(await resolver.check(makeRequest('fs_read'), readDef)).toBe(true);
		expect(await resolver.check(makeRequest('fs_write'), editDef)).toBe(false);
	});

	it('blocks destructive tools via annotation', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'allow',
			rules: [{ tool: '*', annotations: { destructive: true }, policy: 'deny' }],
		});

		const safeDef = { name: 'fs_read', description: '', parameters: {}, annotations: { readOnly: true } };
		const dangerDef = { name: 'fs_delete', description: '', parameters: {}, annotations: { destructive: true } };

		expect(await resolver.check(makeRequest('fs_read'), safeDef)).toBe(true);
		expect(await resolver.check(makeRequest('fs_delete'), dangerDef)).toBe(false);
	});

	it('works without definition (backwards compatible)', async () => {
		const resolver = createToolPermissionResolver({
			defaultPolicy: 'allow',
			rules: [{ tool: 'bash', policy: 'deny' }],
		});

		expect(await resolver.check(makeRequest('bash'))).toBe(false);
		expect(await resolver.check(makeRequest('fs_read'))).toBe(true);
	});
});
```

**Step 2: Run tests to verify failure**

**Step 3: Implement**

Extend `ToolPermissionResolver.check()` signature to accept optional definition:
```typescript
readonly check: (request: ToolCallRequest, definition?: ToolDefinition) => Promise<boolean>;
```

Add to `ToolPermissionRule`:
```typescript
readonly category?: ToolCategory | readonly ToolCategory[];
readonly annotations?: Partial<ToolAnnotations>;
```

In `createToolPermissionResolver`, extend rule matching:
- If rule has `category`, check `definition?.category` against it
- If rule has `annotations`, check each annotation key matches
- Skip rule if category/annotation doesn't match

In `tool-registry.ts` `execute()`, pass definition to resolver:
```typescript
const allowed = await permissionResolver.check(call, registered.definition);
```

**Step 4: Run tests, typecheck, lint**

**Step 5: Commit**

```
feat: add category and annotation-based permission rules
```

---

### Task 6: Token Usage Accumulator

**Files:**
- Modify: `src/ai/loop/types.ts:76-83` (LoopTurn — add usage)
- Modify: `src/ai/loop/types.ts:105-126` (LoopCallbacks — add onUsageUpdate)
- Modify: `src/ai/loop/types.ts:132-139` (AgenticLoopResult — add totalUsage)
- Modify: `src/ai/loop/agentic-loop.ts`
- Modify: `tests/agentic-loop.test.ts`

**Step 1: Write tests**

Add to `tests/agentic-loop.test.ts`. Update the mock to yield usage:

```typescript
it('accumulates token usage across turns', async () => {
	const acpClient = createMockACPClient(['Final response.']);
	// Override generateStream to yield usage
	acpClient.generateStream = mock(async function* () {
		yield { type: 'delta' as const, text: 'Final response.' };
		yield {
			type: 'complete' as const,
			usage: { promptTokens: 100, completionTokens: 50, totalTokens: 150 },
		};
	});

	const result = await createAgenticLoop({
		acpClient,
		toolRegistry: createToolRegistry({}),
		conversation: createConversation(),
	}).run('Hi');

	expect(result.totalUsage).toBeDefined();
	expect(result.totalUsage!.promptTokens).toBe(100);
	expect(result.totalUsage!.completionTokens).toBe(50);
	expect(result.totalUsage!.totalTokens).toBe(150);
});

it('fires onUsageUpdate callback after each turn', async () => {
	const usageUpdates: any[] = [];
	const acpClient = createMockACPClient(['Final response.']);
	acpClient.generateStream = mock(async function* () {
		yield { type: 'delta' as const, text: 'Final response.' };
		yield {
			type: 'complete' as const,
			usage: { promptTokens: 50, completionTokens: 25, totalTokens: 75 },
		};
	});

	await createAgenticLoop({
		acpClient,
		toolRegistry: createToolRegistry({}),
		conversation: createConversation(),
	}).run('Hi', {
		onUsageUpdate: (usage) => usageUpdates.push(usage),
	});

	expect(usageUpdates).toHaveLength(1);
	expect(usageUpdates[0].totalTokens).toBe(75);
});
```

**Step 2: Run tests to verify failure**

**Step 3: Implement**

In `src/ai/loop/types.ts`:
- Add `readonly usage?: ACPTokenUsage` to `LoopTurn` (import `ACPTokenUsage` from acp types)
- Add `readonly onUsageUpdate?: (accumulated: ACPTokenUsage) => void` to `LoopCallbacks`
- Add `readonly totalUsage?: ACPTokenUsage` to `AgenticLoopResult`

In `src/ai/loop/agentic-loop.ts`:
- Import `ACPTokenUsage`
- Add accumulator before the loop: `let accumulatedUsage: ACPTokenUsage | undefined`
- In the stream consumption, capture usage from `complete` chunk
- Add helper: `const addUsage = (usage: ACPTokenUsage) => { ... }`
- Include usage in `LoopTurn`, fire `onUsageUpdate` after each turn
- Include `totalUsage` in `AgenticLoopResult`

**Step 4: Run tests, typecheck, lint**

**Step 5: Commit**

```
feat: add token usage accumulation and onUsageUpdate callback
```

---

### Task 7: Final Verification & Push

**Step 1: Full test suite**

Run: `bun test`
Expected: All tests pass (existing + ~25 new tests)

**Step 2: Typecheck**

Run: `bun x tsc --noEmit`
Expected: Clean

**Step 3: Lint**

Run: `bun run lint`
Expected: Clean

**Step 4: Push**

Run: `git push`

---

## Summary

| Task | What | Lines of Code (est.) |
|------|------|---------------------|
| 1 | Context window usage % | ~15 |
| 2 | Tool execution timeout | ~20 |
| 3 | Tool execution metrics | ~50 |
| 4 | Stream cancellation | ~10 |
| 5 | Category-based permissions | ~40 |
| 6 | Token usage accumulator + callback | ~50 |
| 7 | Final verification | 0 |
