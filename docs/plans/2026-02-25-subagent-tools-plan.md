# Subagent Tools Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable the agentic loop to spawn child agent loops (subagents) with Claude Code-style progress callbacks.

**Architecture:** Two new tools (`subagent_spawn`, `subagent_delegate`) registered via `registerSubagentTools()` following the existing `registerMemoryTools()` pattern. Child loops inherit parent tools, use fresh conversations, and fire nested callbacks for display. Recursion is depth-limited.

**Tech Stack:** TypeScript, Bun test runner, Biome linter. No new dependencies.

---

### Task 1: Add 'subagent' to ToolCategory and subagent callbacks to LoopCallbacks

**Files:**
- Modify: `src/ai/tools/types.ts:14` — add `'subagent'` to ToolCategory union
- Modify: `src/ai/loop/types.ts:62-70` — add subagent callback fields to LoopCallbacks

**Step 1: Add 'subagent' to ToolCategory**

In `src/ai/tools/types.ts`, change the ToolCategory union from:

```ts
export type ToolCategory =
	| 'read'
	| 'edit'
	| 'search'
	| 'execute'
	| 'memory'
	| 'vfs'
	| 'task'
	| 'other';
```

to:

```ts
export type ToolCategory =
	| 'read'
	| 'edit'
	| 'search'
	| 'execute'
	| 'memory'
	| 'vfs'
	| 'task'
	| 'subagent'
	| 'other';
```

**Step 2: Add subagent callback fields to LoopCallbacks**

In `src/ai/loop/types.ts`, add imports for `ToolCallRequest` and `ToolCallResult` (already imported), then add after the existing callback fields:

```ts
export interface SubagentInfo {
	readonly id: string;
	readonly description: string;
	readonly mode: 'spawn' | 'delegate';
}

export interface SubagentResult {
	readonly text: string;
	readonly turns: number;
	readonly durationMs: number;
}
```

Add to `LoopCallbacks` (after `onError`):

```ts
	readonly onSubagentStart?: (info: SubagentInfo) => void;
	readonly onSubagentStreamDelta?: (id: string, text: string) => void;
	readonly onSubagentToolCallStart?: (id: string, call: ToolCallRequest) => void;
	readonly onSubagentToolCallEnd?: (id: string, result: ToolCallResult) => void;
	readonly onSubagentComplete?: (id: string, result: SubagentResult) => void;
	readonly onSubagentError?: (id: string, error: Error) => void;
```

**Step 3: Typecheck**

Run: `bun x tsc --noEmit`
Expected: no errors

**Step 4: Commit**

```bash
git add src/ai/tools/types.ts src/ai/loop/types.ts
git commit -m "Add subagent category and callback types to loop/tool interfaces"
```

---

### Task 2: Create subagent-tools.ts with registerSubagentTools()

**Files:**
- Create: `src/ai/tools/subagent-tools.ts`

**Step 1: Write the test file first**

Create `tests/subagent-tools.test.ts` with tests for:
1. `registerSubagentTools()` registers both tools
2. `subagent_spawn` creates a child loop, runs it, returns finalText
3. `subagent_delegate` calls acpClient.generate() and returns content
4. `onSubagentStart` callback fires with correct info
5. `onSubagentComplete` callback fires with result
6. `onSubagentStreamDelta` fires during child loop streaming
7. Depth limit prevents spawning when maxDepth reached
8. Child inherits parent tools (minus subagent tools at max depth)

The test needs a mock ACP client (reuse the pattern from `tests/agentic-loop.test.ts`):

```ts
import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import { createConversation } from '../src/ai/conversation/conversation.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { registerSubagentTools } from '../src/ai/tools/subagent-tools.js';
import type { SubagentInfo, SubagentResult } from '../src/ai/loop/types.js';

function createMockACPClient(responses: string[] = ['Final response.']): ACPClient {
	let callIdx = 0;
	return {
		initialize: mock(() => Promise.resolve()),
		dispose: mock(() => Promise.resolve()),
		listAgents: mock(() => Promise.resolve([])),
		getAgent: mock(() => Promise.resolve({ id: 'test', name: 'test' })),
		generate: mock(() => {
			const content = responses[callIdx++] ?? 'done';
			return Promise.resolve({
				content,
				agentId: 'test',
				serverName: 'test',
				sessionId: 'sess',
			});
		}),
		chat: mock(() => Promise.resolve({ content: 'chat', agentId: 'test', serverName: 'test', sessionId: 'sess' })),
		generateStream: mock(async function* () {
			const response = responses[callIdx++] ?? 'done';
			yield { type: 'delta' as const, text: response };
			yield { type: 'complete' as const, usage: undefined };
		}),
		embed: mock(() => Promise.resolve({ embeddings: [[]], agentId: 'test', serverName: 'test' })),
		isAvailable: mock(() => Promise.resolve(true)),
		setPermissionPolicy: mock(() => {}),
		getServerHealth: mock(() => undefined),
		listSessions: mock(() => Promise.resolve([])),
		loadSession: mock(() => Promise.resolve({} as any)),
		deleteSession: mock(() => Promise.resolve()),
		setSessionMode: mock(() => Promise.resolve()),
		setSessionModel: mock(() => Promise.resolve()),
		serverNames: ['test'],
		serverCount: 1,
		defaultServerName: 'test',
		defaultAgent: 'test',
	} as ACPClient;
}
```

Tests:
- `registerSubagentTools registers both tools`
- `subagent_spawn runs child loop and returns result`
- `subagent_delegate calls generate and returns content`
- `callbacks fire on subagent lifecycle`
- `depth limit prevents further nesting`

**Step 2: Run tests to verify they fail**

Run: `bun test tests/subagent-tools.test.ts`
Expected: FAIL (module not found)

**Step 3: Create subagent-tools.ts**

Create `src/ai/tools/subagent-tools.ts`:

```ts
// ---------------------------------------------------------------------------
// Subagent Tool Registration
//
// Registers subagent_spawn and subagent_delegate tools with a ToolRegistry.
// Follows the same pattern as registerMemoryTools / registerTaskTools.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import type { ACPClient } from '../acp/acp-client.js';
import { createConversation } from '../conversation/conversation.js';
import { createAgenticLoop } from '../loop/agentic-loop.js';
import type {
	SubagentInfo,
	SubagentResult,
} from '../loop/types.js';
import type {
	ToolCallRequest,
	ToolCallResult,
	ToolDefinition,
	ToolHandler,
	ToolRegistry,
} from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SubagentCallbacks {
	readonly onSubagentStart?: (info: SubagentInfo) => void;
	readonly onSubagentStreamDelta?: (id: string, text: string) => void;
	readonly onSubagentToolCallStart?: (
		id: string,
		call: ToolCallRequest,
	) => void;
	readonly onSubagentToolCallEnd?: (
		id: string,
		result: ToolCallResult,
	) => void;
	readonly onSubagentComplete?: (
		id: string,
		result: SubagentResult,
	) => void;
	readonly onSubagentError?: (id: string, error: Error) => void;
}

export interface SubagentToolsOptions {
	readonly acpClient: ACPClient;
	readonly toolRegistry: ToolRegistry;
	readonly callbacks?: SubagentCallbacks;
	readonly defaultMaxTurns?: number;
	readonly maxDepth?: number;
	readonly serverName?: string;
	readonly agentId?: string;
	readonly systemPrompt?: string;
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

let subagentCounter = 0;

const nextSubagentId = (): string => `sub_${++subagentCounter}`;

const registerTool = (
	registry: ToolRegistry,
	definition: ToolDefinition,
	handler: ToolHandler,
): void => {
	registry.register(definition, handler);
};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

export function registerSubagentTools(
	registry: ToolRegistry,
	options: SubagentToolsOptions,
	depth = 0,
): void {
	const {
		acpClient,
		toolRegistry,
		callbacks,
		defaultMaxTurns = 10,
		maxDepth = 2,
		serverName,
		agentId,
		systemPrompt,
	} = options;

	// If we've hit max depth, don't register subagent tools
	if (depth >= maxDepth) return;

	// subagent_spawn — nested agentic loop
	registerTool(
		registry,
		{
			name: 'subagent_spawn',
			description:
				'Spawn a subagent to handle a complex, multi-step task autonomously. The subagent runs in its own conversation context with access to all tools and returns its final result.',
			parameters: {
				task: {
					type: 'string',
					description: 'The task/prompt for the subagent to work on',
					required: true,
				},
				description: {
					type: 'string',
					description:
						'Short label describing what the subagent will do (e.g. "Researching API endpoints")',
					required: true,
				},
				maxTurns: {
					type: 'number',
					description: `Maximum turns the subagent can take (default: ${defaultMaxTurns})`,
				},
				systemPrompt: {
					type: 'string',
					description: 'Optional system prompt override for the subagent',
				},
			},
			category: 'subagent',
		},
		async (args) => {
			const id = nextSubagentId();
			const task = String(args.task ?? '');
			const desc = String(args.description ?? 'Subagent task');
			const turns =
				typeof args.maxTurns === 'number'
					? args.maxTurns
					: defaultMaxTurns;
			const childSystemPrompt =
				typeof args.systemPrompt === 'string'
					? args.systemPrompt
					: systemPrompt;

			const info: SubagentInfo = Object.freeze({
				id,
				description: desc,
				mode: 'spawn' as const,
			});

			callbacks?.onSubagentStart?.(info);

			try {
				// Create a child tool registry that inherits parent tools
				// and registers subagent tools at depth + 1
				const childRegistry = createChildRegistry(
					toolRegistry,
					options,
					depth + 1,
				);

				const childConversation = createConversation();
				const childLoop = createAgenticLoop({
					acpClient,
					toolRegistry: childRegistry,
					conversation: childConversation,
					maxTurns: turns,
					serverName,
					agentId,
					systemPrompt: childSystemPrompt,
				});

				const start = Date.now();
				const result = await childLoop.run(task, {
					onStreamDelta: (text) => {
						callbacks?.onSubagentStreamDelta?.(id, text);
					},
					onToolCallStart: (call) => {
						callbacks?.onSubagentToolCallStart?.(id, call);
					},
					onToolCallEnd: (toolResult) => {
						callbacks?.onSubagentToolCallEnd?.(id, toolResult);
					},
					onError: (error) => {
						callbacks?.onSubagentError?.(id, error);
					},
				});

				const subResult: SubagentResult = Object.freeze({
					text: result.finalText,
					turns: result.totalTurns,
					durationMs: Date.now() - start,
				});

				callbacks?.onSubagentComplete?.(id, subResult);
				return result.finalText;
			} catch (err) {
				const error = toError(err);
				callbacks?.onSubagentError?.(id, error);
				throw error;
			}
		},
	);

	// subagent_delegate — single-shot ACP generation
	registerTool(
		registry,
		{
			name: 'subagent_delegate',
			description:
				'Delegate a simple task to an ACP agent for a single-shot response. Use for tasks that do not require multi-step tool use.',
			parameters: {
				task: {
					type: 'string',
					description: 'The task/prompt to delegate',
					required: true,
				},
				description: {
					type: 'string',
					description:
						'Short label describing the delegation (e.g. "Summarizing document")',
					required: true,
				},
				serverName: {
					type: 'string',
					description: 'Target ACP server name (optional)',
				},
				agentId: {
					type: 'string',
					description: 'Target agent ID (optional)',
				},
			},
			category: 'subagent',
		},
		async (args) => {
			const id = nextSubagentId();
			const task = String(args.task ?? '');
			const desc = String(args.description ?? 'Delegated task');
			const targetServer =
				typeof args.serverName === 'string'
					? args.serverName
					: serverName;
			const targetAgent =
				typeof args.agentId === 'string' ? args.agentId : agentId;

			const info: SubagentInfo = Object.freeze({
				id,
				description: desc,
				mode: 'delegate' as const,
			});

			callbacks?.onSubagentStart?.(info);

			try {
				const start = Date.now();
				const result = await acpClient.generate(task, {
					serverName: targetServer,
					agentId: targetAgent,
				});

				const subResult: SubagentResult = Object.freeze({
					text: result.content,
					turns: 1,
					durationMs: Date.now() - start,
				});

				callbacks?.onSubagentComplete?.(id, subResult);
				return result.content;
			} catch (err) {
				const error = toError(err);
				callbacks?.onSubagentError?.(id, error);
				throw error;
			}
		},
	);
}

// ---------------------------------------------------------------------------
// Child registry construction
// ---------------------------------------------------------------------------

function createChildRegistry(
	parentRegistry: ToolRegistry,
	options: SubagentToolsOptions,
	childDepth: number,
): ToolRegistry {
	// Import dynamically to avoid circular dependency at module level
	// Actually we can import createToolRegistry since it's in the same package
	const { createToolRegistry } = require('./tool-registry.js');

	// Create a fresh registry with the same options shape
	const childRegistry = createToolRegistry({});

	// Copy all tools from parent except subagent tools
	for (const def of parentRegistry.getToolDefinitions()) {
		if (def.name === 'subagent_spawn' || def.name === 'subagent_delegate') {
			continue;
		}
		// Re-register by executing the parent tool through the parent registry
		childRegistry.register(def, async (args) => {
			const result = await parentRegistry.execute({
				id: `child_call_${Date.now()}`,
				name: def.name,
				arguments: args,
			});
			if (result.isError) throw new Error(result.output);
			return result.output;
		});
	}

	// Register subagent tools at the next depth level
	registerSubagentTools(childRegistry, options, childDepth);

	return childRegistry;
}
```

Wait — the `require()` call won't work in ESM. Let me rethink this. The `createToolRegistry` is needed for child registries, but importing it at the top of `subagent-tools.ts` is fine since it's a sibling file with no circular dep.

Actually, the child doesn't need a full `createToolRegistry()` — it just needs to be a `ToolRegistry`-shaped object. But the simplest approach is to just use `createToolRegistry({})` with an import. Let me check if there's a circular dependency issue.

`subagent-tools.ts` imports from `tool-registry.ts` (for `createToolRegistry`), and `tool-registry.ts` imports from `builtin-tools.ts`. `subagent-tools.ts` is a separate file from `builtin-tools.ts`, so no circular dependency.

The implementation in Step 3 should use a top-level import of `createToolRegistry` instead of `require()`.

**Step 4: Run tests**

Run: `bun test tests/subagent-tools.test.ts`
Expected: all tests pass

**Step 5: Typecheck**

Run: `bun x tsc --noEmit`
Expected: no errors

**Step 6: Commit**

```bash
git add src/ai/tools/subagent-tools.ts tests/subagent-tools.test.ts
git commit -m "Add registerSubagentTools with spawn and delegate modes"
```

---

### Task 3: Update exports and re-exports

**Files:**
- Modify: `src/ai/tools/index.ts:1-5` — add registerSubagentTools export
- Modify: `src/lib.ts:197-215` — add new type and function exports

**Step 1: Update tools/index.ts**

Add to the exports from `builtin-tools.js` (or add a new export line):

```ts
export { registerSubagentTools } from './subagent-tools.js';
export type {
	SubagentCallbacks,
	SubagentToolsOptions,
} from './subagent-tools.js';
```

**Step 2: Update lib.ts**

In the Tool Registry section, add to the type exports:

```ts
	SubagentCallbacks,
	SubagentToolsOptions,
```

Add to the value exports:

```ts
	registerSubagentTools,
```

In the loop types section, add:

```ts
	SubagentInfo,
	SubagentResult,
```

**Step 3: Typecheck and test**

Run: `bun x tsc --noEmit && bun test`
Expected: all pass, no errors

**Step 4: Lint**

Run: `bun run lint:fix`

**Step 5: Commit**

```bash
git add src/ai/tools/index.ts src/lib.ts
git commit -m "Export subagent tools types and registration function"
```

---

### Task 4: Final verification and comprehensive tests

**Files:**
- Modify: `tests/subagent-tools.test.ts` — add edge case tests

**Step 1: Add edge case tests**

Add tests for:
- Subagent spawn with custom systemPrompt
- Subagent delegate to a different server/agent
- Error propagation from child loop to parent callback
- Child tool registry inherits memory/vfs/task tools from parent
- subagent_spawn tool is excluded at max depth
- Counter generates unique IDs across multiple spawns

**Step 2: Run full test suite**

Run: `bun test`
Expected: all pass

**Step 3: Typecheck and lint**

Run: `bun x tsc --noEmit && bun run lint`
Expected: clean

**Step 4: Final commit**

```bash
git add tests/subagent-tools.test.ts
git commit -m "Add comprehensive subagent tools tests"
```

---

## Verification

After all tasks:
1. `bun test` — all tests pass
2. `bun x tsc --noEmit` — no type errors
3. `bun run lint` — no new warnings
4. `bun run build` — builds cleanly
