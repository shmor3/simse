# Memory Middleware & ACP Tool Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform memory into an always-on middleware in the agentic loop, fix ACP tool calling/permissions, add summarization ACP config, optimize RAM usage, and ensure all features are available at the package level.

**Architecture:** Memory middleware lives in `src/ai/memory/middleware.ts` as a factory function. It hooks into the agentic loop at two points: before each turn (enrich system prompt with relevant memories) and after the final turn (store response, trigger auto-summarization). Prompt injection formats memories as structured XML tags in the system prompt. The ACP tool flow is fixed so permissions prompt the user and tools execute correctly. RAM is optimized by keeping only embeddings+metadata in memory, lazy-loading full text.

**Tech Stack:** TypeScript (strict), Bun runtime, Biome linting, `bun test`

---

### Task 1: Prompt Injection Formatter

**Files:**
- Create: `src/ai/memory/prompt-injection.ts`
- Test: `tests/prompt-injection.test.ts`

**Step 1: Write the failing test**

```typescript
import { describe, expect, it } from 'bun:test';
import { formatMemoryContext } from '../src/ai/memory/prompt-injection.js';
import type { SearchResult } from '../src/ai/memory/types.js';

function makeResult(text: string, topic: string, score: number, ageMs = 0): SearchResult {
	return {
		entry: {
			id: `id-${text}`,
			text,
			embedding: [0.1, 0.2],
			metadata: { topic },
			timestamp: Date.now() - ageMs,
		},
		score,
	};
}

describe('formatMemoryContext', () => {
	it('returns empty string for empty results', () => {
		expect(formatMemoryContext([])).toBe('');
	});

	it('formats results as structured XML tags by default', () => {
		const results = [makeResult('Use bun test', 'testing', 0.92)];
		const output = formatMemoryContext(results);
		expect(output).toContain('<memory-context>');
		expect(output).toContain('</memory-context>');
		expect(output).toContain('topic="testing"');
		expect(output).toContain('relevance="0.92"');
		expect(output).toContain('Use bun test');
	});

	it('filters results below minScore', () => {
		const results = [
			makeResult('high', 'a', 0.9),
			makeResult('low', 'b', 0.3),
		];
		const output = formatMemoryContext(results, { minScore: 0.5 });
		expect(output).toContain('high');
		expect(output).not.toContain('low');
	});

	it('limits to maxResults', () => {
		const results = [
			makeResult('one', 'a', 0.9),
			makeResult('two', 'b', 0.8),
			makeResult('three', 'c', 0.7),
		];
		const output = formatMemoryContext(results, { maxResults: 2 });
		expect(output).toContain('one');
		expect(output).toContain('two');
		expect(output).not.toContain('three');
	});

	it('truncates to maxChars', () => {
		const longText = 'x'.repeat(5000);
		const results = [makeResult(longText, 'a', 0.9)];
		const output = formatMemoryContext(results, { maxChars: 200 });
		expect(output.length).toBeLessThanOrEqual(250); // tag overhead
	});

	it('uses custom tag name', () => {
		const results = [makeResult('hello', 'a', 0.9)];
		const output = formatMemoryContext(results, { tag: 'context' });
		expect(output).toContain('<context>');
		expect(output).toContain('</context>');
	});

	it('formats as natural text when format is natural', () => {
		const results = [makeResult('Use bun test', 'testing', 0.92)];
		const output = formatMemoryContext(results, { format: 'natural' });
		expect(output).not.toContain('<memory-context>');
		expect(output).toContain('Relevant context from memory:');
		expect(output).toContain('Use bun test');
	});

	it('formats relative age for entries', () => {
		const results = [makeResult('old entry', 'a', 0.9, 3_600_000)]; // 1h ago
		const output = formatMemoryContext(results);
		expect(output).toContain('age="1h"');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/prompt-injection.test.ts`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```typescript
// src/ai/memory/prompt-injection.ts
import type { SearchResult } from './types.js';

export interface PromptInjectionOptions {
	readonly maxResults?: number;
	readonly minScore?: number;
	readonly format?: 'structured' | 'natural';
	readonly tag?: string;
	readonly maxChars?: number;
}

function formatAge(ms: number): string {
	const seconds = Math.floor(ms / 1000);
	if (seconds < 60) return `${seconds}s`;
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes}m`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h`;
	const days = Math.floor(hours / 24);
	return `${days}d`;
}

export function formatMemoryContext(
	results: readonly SearchResult[],
	options?: PromptInjectionOptions,
): string {
	if (results.length === 0) return '';

	const maxResults = options?.maxResults ?? results.length;
	const minScore = options?.minScore ?? 0;
	const format = options?.format ?? 'structured';
	const tag = options?.tag ?? 'memory-context';
	const maxChars = options?.maxChars ?? 4000;

	const filtered = results
		.filter((r) => r.score >= minScore)
		.slice(0, maxResults);

	if (filtered.length === 0) return '';

	const now = Date.now();

	if (format === 'natural') {
		const lines = ['Relevant context from memory:'];
		let chars = lines[0].length;
		for (const r of filtered) {
			const topic = r.entry.metadata.topic ?? 'uncategorized';
			const line = `- [${topic}] (relevance: ${r.score.toFixed(2)}) ${r.entry.text}`;
			if (chars + line.length > maxChars) break;
			lines.push(line);
			chars += line.length;
		}
		return lines.join('\n');
	}

	const entries: string[] = [];
	let chars = 0;
	for (const r of filtered) {
		const topic = r.entry.metadata.topic ?? 'uncategorized';
		const age = formatAge(now - r.entry.timestamp);
		const text = r.entry.text;
		const entry = `<entry topic="${topic}" relevance="${r.score.toFixed(2)}" age="${age}">\n${text}\n</entry>`;
		if (chars + entry.length > maxChars) break;
		entries.push(entry);
		chars += entry.length;
	}

	return `<${tag}>\n${entries.join('\n')}\n</${tag}>`;
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/prompt-injection.test.ts`
Expected: PASS (8 tests)

**Step 5: Commit**

```bash
git add src/ai/memory/prompt-injection.ts tests/prompt-injection.test.ts
git commit -m "feat: add structured prompt injection formatter for memory context"
```

---

### Task 2: Memory Middleware

**Files:**
- Create: `src/ai/memory/middleware.ts`
- Test: `tests/memory-middleware.test.ts`

**Step 1: Write the failing test**

```typescript
import { beforeEach, describe, expect, it, mock } from 'bun:test';
import { createMemoryMiddleware } from '../src/ai/memory/middleware.js';
import type { MemoryManager } from '../src/ai/memory/memory.js';

function createMockMemoryManager(searchResults: Array<{ entry: { id: string; text: string; embedding: number[]; metadata: Record<string, string>; timestamp: number }; score: number }> = []): MemoryManager {
	return {
		initialize: mock(async () => {}),
		dispose: mock(async () => {}),
		add: mock(async () => 'mock-id'),
		addBatch: mock(async () => []),
		search: mock(async () => searchResults),
		textSearch: mock(() => []),
		filterByMetadata: mock(() => []),
		filterByDateRange: mock(() => []),
		advancedSearch: mock(async () => []),
		query: mock(async () => []),
		getById: mock(() => undefined),
		getAll: mock(() => []),
		getTopics: mock(() => []),
		filterByTopic: mock(() => []),
		recommend: mock(async () => []),
		findDuplicates: mock(() => []),
		checkDuplicate: mock(async () => ({ isDuplicate: false })),
		summarize: mock(async () => ({ summaryId: '', summaryText: '', sourceIds: [], deletedOriginals: false })),
		setTextGenerator: mock(() => {}),
		recordFeedback: mock(() => {}),
		delete: mock(async () => false),
		deleteBatch: mock(async () => 0),
		clear: mock(async () => {}),
		learningProfile: undefined,
		size: searchResults.length,
		isInitialized: true,
		isDirty: false,
		embeddingAgent: undefined,
	} as unknown as MemoryManager;
}

describe('createMemoryMiddleware', () => {
	it('returns a frozen object with enrichSystemPrompt and afterResponse', () => {
		const mw = createMemoryMiddleware(createMockMemoryManager());
		expect(mw.enrichSystemPrompt).toBeFunction();
		expect(mw.afterResponse).toBeFunction();
		expect(Object.isFrozen(mw)).toBe(true);
	});

	it('enrichSystemPrompt appends memory context to system prompt', async () => {
		const results = [{
			entry: { id: '1', text: 'Use bun test', embedding: [0.1], metadata: { topic: 'testing' }, timestamp: Date.now() },
			score: 0.9,
		}];
		const mw = createMemoryMiddleware(createMockMemoryManager(results));
		const enriched = await mw.enrichSystemPrompt({
			userInput: 'how do I test?',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toContain('You are helpful.');
		expect(enriched).toContain('<memory-context>');
		expect(enriched).toContain('Use bun test');
	});

	it('enrichSystemPrompt returns original prompt when no results', async () => {
		const mw = createMemoryMiddleware(createMockMemoryManager([]));
		const enriched = await mw.enrichSystemPrompt({
			userInput: 'hello',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toBe('You are helpful.');
	});

	it('enrichSystemPrompt gracefully handles search errors', async () => {
		const mm = createMockMemoryManager();
		(mm.search as ReturnType<typeof mock>).mockImplementation(async () => {
			throw new Error('embed failed');
		});
		const mw = createMemoryMiddleware(mm);
		const enriched = await mw.enrichSystemPrompt({
			userInput: 'hello',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toBe('You are helpful.');
	});

	it('afterResponse stores Q&A in memory', async () => {
		const mm = createMockMemoryManager();
		const mw = createMemoryMiddleware(mm, { storeTopic: 'chat' });
		await mw.afterResponse('What is Bun?', 'Bun is a JS runtime.');
		expect(mm.add).toHaveBeenCalledTimes(1);
		const callArgs = (mm.add as ReturnType<typeof mock>).mock.calls[0];
		expect(callArgs[0]).toContain('What is Bun?');
		expect(callArgs[0]).toContain('Bun is a JS runtime.');
		expect(callArgs[1]).toEqual(expect.objectContaining({ topic: 'chat' }));
	});

	it('afterResponse skips empty responses', async () => {
		const mm = createMockMemoryManager();
		const mw = createMemoryMiddleware(mm);
		await mw.afterResponse('hello', '');
		expect(mm.add).not.toHaveBeenCalled();
	});

	it('afterResponse skips error responses', async () => {
		const mm = createMockMemoryManager();
		const mw = createMemoryMiddleware(mm);
		await mw.afterResponse('hello', 'Error communicating with ACP');
		expect(mm.add).not.toHaveBeenCalled();
	});

	it('respects maxResults option', async () => {
		const mm = createMockMemoryManager([
			{ entry: { id: '1', text: 'a', embedding: [0.1], metadata: { topic: 'x' }, timestamp: Date.now() }, score: 0.9 },
		]);
		const mw = createMemoryMiddleware(mm, { maxResults: 3 });
		await mw.enrichSystemPrompt({
			userInput: 'test',
			currentSystemPrompt: '',
			conversationHistory: '',
			turn: 1,
		});
		expect(mm.search).toHaveBeenCalledWith('test', 3, undefined);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/memory-middleware.test.ts`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```typescript
// src/ai/memory/middleware.ts
import type { MemoryManager } from './memory.js';
import { formatMemoryContext, type PromptInjectionOptions } from './prompt-injection.js';

export interface MiddlewareContext {
	readonly userInput: string;
	readonly currentSystemPrompt: string;
	readonly conversationHistory: string;
	readonly turn: number;
}

export interface MemoryMiddleware {
	readonly enrichSystemPrompt: (context: MiddlewareContext) => Promise<string>;
	readonly afterResponse: (userInput: string, response: string) => Promise<void>;
}

export interface MemoryMiddlewareOptions {
	readonly maxResults?: number;
	readonly minScore?: number;
	readonly format?: PromptInjectionOptions;
	readonly storeTopic?: string;
	readonly storeResponses?: boolean;
}

export function createMemoryMiddleware(
	memoryManager: MemoryManager,
	options?: MemoryMiddlewareOptions,
): MemoryMiddleware {
	const maxResults = options?.maxResults ?? 5;
	const minScore = options?.minScore;
	const storeTopic = options?.storeTopic ?? 'conversation';
	const storeResponses = options?.storeResponses ?? true;
	const formatOptions = options?.format;

	const enrichSystemPrompt = async (
		context: MiddlewareContext,
	): Promise<string> => {
		if (!memoryManager.isInitialized || memoryManager.size === 0) {
			return context.currentSystemPrompt;
		}

		try {
			const results = await memoryManager.search(
				context.userInput,
				maxResults,
				minScore,
			);

			if (results.length === 0) {
				return context.currentSystemPrompt;
			}

			const memoryBlock = formatMemoryContext(results, {
				...formatOptions,
				maxResults,
				minScore,
			});

			if (!memoryBlock) {
				return context.currentSystemPrompt;
			}

			return [context.currentSystemPrompt, memoryBlock]
				.filter(Boolean)
				.join('\n\n');
		} catch {
			return context.currentSystemPrompt;
		}
	};

	const afterResponse = async (
		userInput: string,
		response: string,
	): Promise<void> => {
		if (!storeResponses) return;
		if (!response || response.startsWith('Error communicating')) return;
		if (!response.trim() || !userInput.trim()) return;

		try {
			await memoryManager.add(`Q: ${userInput}\nA: ${response}`, {
				topic: storeTopic,
				source: 'middleware',
			});
		} catch {
			// Storage failure is non-critical
		}
	};

	return Object.freeze({ enrichSystemPrompt, afterResponse });
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/memory-middleware.test.ts`
Expected: PASS (8 tests)

**Step 5: Commit**

```bash
git add src/ai/memory/middleware.ts tests/memory-middleware.test.ts
git commit -m "feat: add memory middleware for automatic context injection"
```

---

### Task 3: Integrate Memory Middleware into Agentic Loop

**Files:**
- Modify: `src/ai/loop/types.ts`
- Modify: `src/ai/loop/agentic-loop.ts`
- Test: `tests/loop-memory-middleware.test.ts`

**Step 1: Write the failing test**

```typescript
import { describe, expect, it, mock } from 'bun:test';
import { createAgenticLoop } from '../src/ai/loop/agentic-loop.js';
import type { MemoryMiddleware } from '../src/ai/memory/middleware.js';
import { createSilentLogger } from './utils/mocks.js';

// Minimal mocks
function createMockACPClient(responseText: string) {
	return {
		generateStream: mock(function* () {
			yield { type: 'delta', text: responseText };
		}),
	} as any;
}

function createMockToolRegistry() {
	return {
		formatForSystemPrompt: mock(() => 'TOOLS'),
		parseToolCalls: mock((text: string) => ({ text, toolCalls: [] })),
		execute: mock(async () => ({ id: '1', name: 'test', output: 'ok', isError: false })),
		batchExecute: mock(async () => []),
		discover: mock(async () => {}),
		register: mock(() => {}),
		unregister: mock(() => false),
		getToolDefinitions: mock(() => []),
		toolCount: 0,
		toolNames: [],
	} as any;
}

function createMockConversation() {
	let systemPrompt = '';
	return {
		addUser: mock(() => {}),
		addAssistant: mock(() => {}),
		addToolResult: mock(() => {}),
		setSystemPrompt: mock((p: string) => { systemPrompt = p; }),
		getSystemPrompt: () => systemPrompt,
		serialize: mock(() => 'serialized'),
		needsCompaction: false,
		messageCount: 0,
		estimatedChars: 0,
		compact: mock(() => {}),
	} as any;
}

describe('agentic loop with memory middleware', () => {
	it('calls enrichSystemPrompt before each turn', async () => {
		const middleware: MemoryMiddleware = {
			enrichSystemPrompt: mock(async (ctx) => ctx.currentSystemPrompt + '\n\nMEMORY'),
			afterResponse: mock(async () => {}),
		};

		const loop = createAgenticLoop({
			acpClient: createMockACPClient('Hello!'),
			toolRegistry: createMockToolRegistry(),
			conversation: createMockConversation(),
			memoryMiddleware: middleware,
			maxTurns: 1,
		});

		await loop.run('test input');
		expect(middleware.enrichSystemPrompt).toHaveBeenCalled();
	});

	it('calls afterResponse after final response', async () => {
		const middleware: MemoryMiddleware = {
			enrichSystemPrompt: mock(async (ctx) => ctx.currentSystemPrompt),
			afterResponse: mock(async () => {}),
		};

		const loop = createAgenticLoop({
			acpClient: createMockACPClient('Final answer'),
			toolRegistry: createMockToolRegistry(),
			conversation: createMockConversation(),
			memoryMiddleware: middleware,
			maxTurns: 1,
		});

		await loop.run('test input');
		expect(middleware.afterResponse).toHaveBeenCalledWith('test input', 'Final answer');
	});

	it('works without middleware (backward compatible)', async () => {
		const loop = createAgenticLoop({
			acpClient: createMockACPClient('Hello!'),
			toolRegistry: createMockToolRegistry(),
			conversation: createMockConversation(),
			maxTurns: 1,
		});

		const result = await loop.run('test');
		expect(result.finalText).toBe('Hello!');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/loop-memory-middleware.test.ts`
Expected: FAIL — `memoryMiddleware` not in options type

**Step 3: Add memoryMiddleware to loop types**

In `src/ai/loop/types.ts`, add to `AgenticLoopOptions` (after `eventBus`):

```typescript
import type { MemoryMiddleware } from '../memory/middleware.js';

// Add to AgenticLoopOptions interface:
	readonly memoryMiddleware?: MemoryMiddleware;
```

**Step 4: Integrate into agentic-loop.ts**

In `src/ai/loop/agentic-loop.ts`, extract `memoryMiddleware` from options (line ~61), then:

Before the system prompt is set (around line 76), after building `fullSystemPrompt`:

```typescript
// Memory middleware: enrich system prompt per-turn
let enrichedSystemPrompt = fullSystemPrompt;
if (memoryMiddleware) {
	try {
		enrichedSystemPrompt = await memoryMiddleware.enrichSystemPrompt({
			userInput,
			currentSystemPrompt: fullSystemPrompt,
			conversationHistory: conversation.serialize(),
			turn,
		});
	} catch {
		// Middleware failure is non-critical
	}
}

if (enrichedSystemPrompt) {
	conversation.setSystemPrompt(enrichedSystemPrompt);
}
```

Move the system prompt setting INSIDE the turn loop (it was outside before — this is needed for per-turn memory refresh).

After the loop ends with a final text response (line ~215), add:

```typescript
// Memory middleware: store response
if (memoryMiddleware) {
	try {
		await memoryMiddleware.afterResponse(userInput, lastText);
	} catch {
		// Storage failure is non-critical
	}
}
```

**Step 5: Run test to verify it passes**

Run: `bun test tests/loop-memory-middleware.test.ts`
Expected: PASS (3 tests)

**Step 6: Run all tests**

Run: `bun test`
Expected: All existing tests still pass

**Step 7: Commit**

```bash
git add src/ai/loop/types.ts src/ai/loop/agentic-loop.ts tests/loop-memory-middleware.test.ts
git commit -m "feat: integrate memory middleware into agentic loop for per-turn context"
```

---

### Task 4: Summarization ACP Config

**Files:**
- Modify: `simse-code/config.ts`
- Modify: `simse-code/setup.ts`
- Modify: `simse-code/cli.ts`
- Test: `tests/summarize-config.test.ts`

**Step 1: Write the failing test**

```typescript
import { describe, expect, it } from 'bun:test';

// Test the config type and parsing
describe('SummarizeFileConfig', () => {
	it('accepts valid summarize config', () => {
		const config = {
			server: 'summarize-llm',
			command: 'claude',
			args: ['--acp'],
			agent: 'summarizer',
		};
		expect(config.server).toBe('summarize-llm');
		expect(config.command).toBe('claude');
	});

	it('works without optional fields', () => {
		const config = {
			server: 'summarize-llm',
			command: 'ollama',
		};
		expect(config.server).toBe('summarize-llm');
		expect(config.args).toBeUndefined();
	});
});
```

**Step 2: Add SummarizeFileConfig to config.ts**

In `simse-code/config.ts`, add after `EmbedFileConfig`:

```typescript
export interface SummarizeFileConfig {
	readonly server: string;
	readonly command: string;
	readonly args?: readonly string[];
	readonly agent?: string;
	readonly env?: Readonly<Record<string, string>>;
}
```

Add `summarizeConfig` to `CLIConfigResult` and load `summarize.json` if it exists.

**Step 3: Add summarization ACP setup to setup.ts**

After the embed config section, add:

```typescript
// -- Summarization ACP (optional) ------------------------------------------
const summarizePath = join(dataDir, 'summarize.json');
if (!existsSync(summarizePath)) {
	console.log('\n  Configure auto-summarization? (uses a separate LLM)\n');
	console.log('    1) Same provider as above');
	console.log('    2) Different provider');
	console.log('    3) Skip (no auto-summarization)');
	console.log('');

	const choice = (await ask(rl, '  Choice [1-3]: ')).trim();

	if (choice === '1' && acpConfig) {
		const server = acpConfig.servers[0];
		const summarizeConf = {
			server: server.name,
			command: server.command,
			args: server.args,
		};
		writeFileSync(summarizePath, `${JSON.stringify(summarizeConf, null, '\t')}\n`, 'utf-8');
		filesCreated.push('summarize.json');
	} else if (choice === '2') {
		const command = await askRequired(rl, '  Command: ');
		const argsStr = await askOptional(rl, '  Args (space-separated): ');
		const summarizeConf = {
			server: 'summarize-llm',
			command,
			...(argsStr && { args: argsStr.split(/\s+/) }),
		};
		writeFileSync(summarizePath, `${JSON.stringify(summarizeConf, null, '\t')}\n`, 'utf-8');
		filesCreated.push('summarize.json');
	}
}
```

**Step 4: Wire up in cli.ts**

In `cli.ts`, after the main `acpClient` is created, check for `summarize.json`. If present, create a second ACP client and wrap it as a `TextGenerationProvider` using `createACPGenerator`. Pass it to the app's memory manager via `setTextGenerator()`.

**Step 5: Run tests**

Run: `bun test tests/summarize-config.test.ts && bun test`
Expected: All pass

**Step 6: Commit**

```bash
git add simse-code/config.ts simse-code/setup.ts simse-code/cli.ts tests/summarize-config.test.ts
git commit -m "feat: add summarization ACP config and first-time setup"
```

---

### Task 5: RAM/Disk Optimization — LRU Text Cache

**Files:**
- Create: `src/ai/memory/text-cache.ts`
- Test: `tests/text-cache.test.ts`

**Step 1: Write the failing test**

```typescript
import { describe, expect, it } from 'bun:test';
import { createTextCache } from '../src/ai/memory/text-cache.js';

describe('createTextCache', () => {
	it('stores and retrieves text', () => {
		const cache = createTextCache({ maxEntries: 10 });
		cache.set('id1', 'hello world');
		expect(cache.get('id1')).toBe('hello world');
	});

	it('returns undefined for missing keys', () => {
		const cache = createTextCache({ maxEntries: 10 });
		expect(cache.get('missing')).toBeUndefined();
	});

	it('evicts oldest entry when maxEntries exceeded', () => {
		const cache = createTextCache({ maxEntries: 2 });
		cache.set('a', 'text-a');
		cache.set('b', 'text-b');
		cache.set('c', 'text-c');
		expect(cache.get('a')).toBeUndefined();
		expect(cache.get('b')).toBe('text-b');
		expect(cache.get('c')).toBe('text-c');
	});

	it('promotes recently accessed entries', () => {
		const cache = createTextCache({ maxEntries: 2 });
		cache.set('a', 'text-a');
		cache.set('b', 'text-b');
		cache.get('a'); // promote a
		cache.set('c', 'text-c'); // should evict b
		expect(cache.get('a')).toBe('text-a');
		expect(cache.get('b')).toBeUndefined();
		expect(cache.get('c')).toBe('text-c');
	});

	it('tracks size correctly', () => {
		const cache = createTextCache({ maxEntries: 10 });
		expect(cache.size).toBe(0);
		cache.set('a', 'hello');
		expect(cache.size).toBe(1);
		cache.delete('a');
		expect(cache.size).toBe(0);
	});

	it('clears all entries', () => {
		const cache = createTextCache({ maxEntries: 10 });
		cache.set('a', 'hello');
		cache.set('b', 'world');
		cache.clear();
		expect(cache.size).toBe(0);
		expect(cache.get('a')).toBeUndefined();
	});

	it('evicts by maxBytes', () => {
		const cache = createTextCache({ maxEntries: 100, maxBytes: 20 });
		cache.set('a', 'hello world'); // 11 bytes
		cache.set('b', 'hello world'); // 11 bytes, should evict a
		expect(cache.get('a')).toBeUndefined();
		expect(cache.get('b')).toBe('hello world');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/text-cache.test.ts`
Expected: FAIL

**Step 3: Write implementation**

```typescript
// src/ai/memory/text-cache.ts

export interface TextCacheOptions {
	readonly maxEntries?: number;
	readonly maxBytes?: number;
}

export interface TextCache {
	readonly get: (id: string) => string | undefined;
	readonly set: (id: string, text: string) => void;
	readonly delete: (id: string) => boolean;
	readonly clear: () => void;
	readonly has: (id: string) => boolean;
	readonly size: number;
}

export function createTextCache(options?: TextCacheOptions): TextCache {
	const maxEntries = options?.maxEntries ?? 500;
	const maxBytes = options?.maxBytes ?? 5 * 1024 * 1024; // 5MB

	// Doubly-linked list node for LRU ordering
	interface Node {
		id: string;
		text: string;
		byteSize: number;
		prev: Node | undefined;
		next: Node | undefined;
	}

	const map = new Map<string, Node>();
	let head: Node | undefined;
	let tail: Node | undefined;
	let totalBytes = 0;

	const remove = (node: Node): void => {
		if (node.prev) node.prev.next = node.next;
		else head = node.next;
		if (node.next) node.next.prev = node.prev;
		else tail = node.prev;
		node.prev = undefined;
		node.next = undefined;
	};

	const addToFront = (node: Node): void => {
		node.next = head;
		node.prev = undefined;
		if (head) head.prev = node;
		head = node;
		if (!tail) tail = node;
	};

	const evict = (): void => {
		while (tail && (map.size > maxEntries || totalBytes > maxBytes)) {
			const victim = tail;
			remove(victim);
			map.delete(victim.id);
			totalBytes -= victim.byteSize;
		}
	};

	return Object.freeze({
		get(id: string): string | undefined {
			const node = map.get(id);
			if (!node) return undefined;
			remove(node);
			addToFront(node);
			return node.text;
		},
		set(id: string, text: string): void {
			const existing = map.get(id);
			if (existing) {
				remove(existing);
				totalBytes -= existing.byteSize;
				map.delete(id);
			}
			const byteSize = Buffer.byteLength(text, 'utf-8');
			const node: Node = { id, text, byteSize, prev: undefined, next: undefined };
			addToFront(node);
			map.set(id, node);
			totalBytes += byteSize;
			evict();
		},
		delete(id: string): boolean {
			const node = map.get(id);
			if (!node) return false;
			remove(node);
			map.delete(id);
			totalBytes -= node.byteSize;
			return true;
		},
		clear(): void {
			map.clear();
			head = undefined;
			tail = undefined;
			totalBytes = 0;
		},
		has(id: string): boolean {
			return map.has(id);
		},
		get size(): number {
			return map.size;
		},
	});
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/text-cache.test.ts`
Expected: PASS (7 tests)

**Step 5: Commit**

```bash
git add src/ai/memory/text-cache.ts tests/text-cache.test.ts
git commit -m "feat: add LRU text cache for memory RAM optimization"
```

---

### Task 6: Wire up LRU Cache in Vector Store

**Files:**
- Modify: `src/ai/memory/vector-store.ts`
- Modify: `src/ai/memory/types.ts` (add `VectorEntrySlim` if needed)
- Test: Existing tests must pass + new test for cache behavior

**Step 1: Write the failing test**

```typescript
// tests/vector-store-cache.test.ts
import { describe, expect, it, mock } from 'bun:test';
import { createVectorStore } from '../src/ai/memory/vector-store.js';
import { createMemoryStorage, createSilentLogger } from './utils/mocks.js';

describe('vector store text cache', () => {
	it('search results include full text', async () => {
		const store = createVectorStore({
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			textCache: { maxEntries: 100 },
			learning: { enabled: false },
		});
		await store.load();
		await store.add({
			id: 'test-1',
			text: 'Hello world test entry',
			embedding: [1, 0, 0],
			metadata: { topic: 'test' },
			timestamp: Date.now(),
		});
		const results = store.search([1, 0, 0], 5);
		expect(results[0].entry.text).toBe('Hello world test entry');
	});

	it('getById returns full text from cache', async () => {
		const store = createVectorStore({
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			textCache: { maxEntries: 100 },
			learning: { enabled: false },
		});
		await store.load();
		await store.add({
			id: 'test-1',
			text: 'cached text',
			embedding: [1, 0, 0],
			metadata: {},
			timestamp: Date.now(),
		});
		const entry = store.getById('test-1');
		expect(entry?.text).toBe('cached text');
	});
});
```

**Step 2: Add textCache option to VectorStoreOptions**

Add `readonly textCache?: TextCacheOptions;` to `VectorStoreOptions`. Import `createTextCache` and use it internally. When `textCache` is configured, entries store text in the LRU cache and the in-memory entry array holds a reference. This is an internal optimization — the external API stays the same (search results always include full text).

**Step 3: Run all tests**

Run: `bun test`
Expected: All pass (including existing vector store tests)

**Step 4: Commit**

```bash
git add src/ai/memory/vector-store.ts tests/vector-store-cache.test.ts
git commit -m "feat: integrate LRU text cache into vector store"
```

---

### Task 7: Package Exports

**Files:**
- Modify: `src/lib.ts`

**Step 1: Add exports for new modules**

Add to `src/lib.ts`:

```typescript
// ---- Memory Middleware ----------------------------------------------------
export type {
	MemoryMiddleware,
	MemoryMiddlewareOptions,
	MiddlewareContext,
} from './ai/memory/middleware.js';
export { createMemoryMiddleware } from './ai/memory/middleware.js';
// ---- Prompt Injection -----------------------------------------------------
export type { PromptInjectionOptions } from './ai/memory/prompt-injection.js';
export { formatMemoryContext } from './ai/memory/prompt-injection.js';
// ---- Text Cache -----------------------------------------------------------
export type { TextCache, TextCacheOptions } from './ai/memory/text-cache.js';
export { createTextCache } from './ai/memory/text-cache.js';
```

**Step 2: Run typecheck**

Run: `bun x tsc --noEmit`
Expected: No errors

**Step 3: Commit**

```bash
git add src/lib.ts
git commit -m "feat: export memory middleware, prompt injection, and text cache from package"
```

---

### Task 8: Remove Memory Logic from CLI

**Files:**
- Modify: `simse-code/cli.ts`

**Step 1: Replace inline memory injection with middleware**

In `cli.ts` around lines 367-390 (the `enrichedInput` block), and lines 493-504 (the post-loop storage), remove the inline memory logic. Instead, create a `memoryMiddleware` instance and pass it to `createAgenticLoop()`:

```typescript
const memoryMiddleware = session.memoryEnabled
	? createMemoryMiddleware(app.memory, {
		maxResults: 5,
		storeTopic: 'conversation',
	})
	: undefined;

const loop = createAgenticLoop({
	acpClient: ctx.acpClient,
	toolRegistry: ctx.toolRegistry,
	conversation: ctx.conversation,
	maxTurns: session.maxTurns,
	serverName: session.serverName,
	systemPrompt: systemPromptParts.join('\n\n') || undefined,
	signal: abortController.signal,
	memoryMiddleware,
});
```

Remove the post-loop `if (session.memoryEnabled && result.finalText && !isError)` block — the middleware handles this now.

**Step 2: Run full test suite**

Run: `bun test`
Expected: All pass

**Step 3: Commit**

```bash
git add simse-code/cli.ts
git commit -m "refactor: replace inline memory injection with middleware in CLI"
```

---

### Task 9: Pure Functional Programming Audit

**Files:**
- Audit: All files in `src/ai/tools/host/`, `src/events/`, `src/hooks/`, `src/server/`, new files

**Step 1: Check for `class` keyword**

Run: `grep -rn "^class \|^export class " src/`
Expected: No matches. If any found, refactor to factory functions.

**Step 2: Check for missing Object.freeze on factory returns**

Run: `grep -rn "return {" src/ | grep -v "Object.freeze" | grep -v "test\|spec"`
Review each match — factory functions must return `Object.freeze()`.

**Step 3: Check for non-readonly interface properties**

Run: `grep -rn "^\s\+[a-z].*:" src/ --include="*.ts" | grep -v "readonly\|import\|export\|//\|type\|const\|let\|function\|return\|if\|for\|while"`
Review and fix any mutable interface properties.

**Step 4: Fix any issues found**

**Step 5: Run typecheck + tests**

Run: `bun x tsc --noEmit && bun test`
Expected: All pass

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor: enforce pure functional programming across all modules"
```

---

### Task 10: Integration Tests

**Files:**
- Create: `tests/memory-integration.test.ts`

**Step 1: Write integration tests**

```typescript
import { beforeEach, describe, expect, it, mock } from 'bun:test';
import { createMemoryManager } from '../src/ai/memory/memory.js';
import { createMemoryMiddleware } from '../src/ai/memory/middleware.js';
import type { EmbeddingProvider, TextGenerationProvider } from '../src/ai/memory/types.js';
import { createMemoryStorage, createSilentLogger } from './utils/mocks.js';

function createTestEmbedder(): EmbeddingProvider {
	let callCount = 0;
	return {
		embed: mock(async (input: string | readonly string[]) => {
			const texts = typeof input === 'string' ? [input] : input;
			callCount++;
			return {
				embeddings: texts.map((_, i) =>
					Array.from({ length: 3 }, (__, j) => Math.sin((callCount * 10 + i) * 0.1 + j * 0.7)),
				),
			};
		}),
	};
}

describe('memory middleware integration', () => {
	it('end-to-end: store -> search -> inject -> respond -> store', async () => {
		const embedder = createTestEmbedder();
		const manager = createMemoryManager(embedder, {
			enabled: true,
			embeddingAgent: 'test',
			similarityThreshold: 0,
			maxResults: 10,
		}, {
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			vectorStoreOptions: { autoSave: true, flushIntervalMs: 0, learning: { enabled: false } },
		});
		await manager.initialize();

		// Seed memory
		await manager.add('Always use bun test for testing', { topic: 'tools' });
		await manager.add('Project uses Biome linter', { topic: 'tools' });

		const middleware = createMemoryMiddleware(manager, {
			maxResults: 5,
			storeTopic: 'chat',
		});

		// Enrich prompt
		const enriched = await middleware.enrichSystemPrompt({
			userInput: 'how do I run tests?',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});

		expect(enriched).toContain('You are helpful.');
		expect(enriched).toContain('<memory-context>');

		// Simulate response storage
		await middleware.afterResponse('how do I run tests?', 'Use `bun test` to run all tests.');

		// Verify stored
		expect(manager.size).toBe(3); // 2 seeded + 1 from afterResponse
	});

	it('auto-summarization triggers after threshold', async () => {
		const embedder = createTestEmbedder();
		const textGen: TextGenerationProvider = {
			generate: mock(async () => 'Summary of all entries'),
		};
		const manager = createMemoryManager(embedder, {
			enabled: true,
			embeddingAgent: 'test',
			similarityThreshold: 0,
			maxResults: 10,
		}, {
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			textGenerator: textGen,
			vectorStoreOptions: { autoSave: true, flushIntervalMs: 0, learning: { enabled: false } },
		});
		await manager.initialize();

		// Add entries to a topic
		for (let i = 0; i < 5; i++) {
			await manager.add(`Entry ${i}`, { topic: 'test-topic' });
		}

		// Summarize the 3 oldest entries
		const sorted = manager.filterByTopic(['test-topic']).sort((a, b) => a.timestamp - b.timestamp);
		const toSummarize = sorted.slice(0, 3);

		const result = await manager.summarize({
			ids: toSummarize.map((e) => e.id),
			deleteOriginals: true,
			metadata: { topic: 'test-topic' },
		});

		expect(result.summaryText).toBe('Summary of all entries');
		expect(result.deletedOriginals).toBe(true);
		// 5 original - 3 deleted + 1 summary = 3
		expect(manager.size).toBe(3);
	});

	it('middleware gracefully handles uninitialized manager', async () => {
		const embedder = createTestEmbedder();
		const manager = createMemoryManager(embedder, {
			enabled: true,
			embeddingAgent: 'test',
			similarityThreshold: 0,
			maxResults: 10,
		}, {
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			vectorStoreOptions: { autoSave: true, flushIntervalMs: 0, learning: { enabled: false } },
		});
		// NOT initialized

		const middleware = createMemoryMiddleware(manager);
		const result = await middleware.enrichSystemPrompt({
			userInput: 'hello',
			currentSystemPrompt: 'System',
			conversationHistory: '',
			turn: 1,
		});
		expect(result).toBe('System');
	});
});
```

**Step 2: Run integration tests**

Run: `bun test tests/memory-integration.test.ts`
Expected: PASS (3 tests)

**Step 3: Commit**

```bash
git add tests/memory-integration.test.ts
git commit -m "test: add memory middleware integration tests"
```

---

### Task 11: E2E Test — Full Pipeline

**Files:**
- Create: `tests/e2e-memory-middleware.test.ts`

**Step 1: Write E2E test**

Test the full flow: embedder -> memory manager -> middleware -> prompt injection -> loop integration (mocked ACP).

```typescript
import { describe, expect, it, mock } from 'bun:test';
import { createAgenticLoop } from '../src/ai/loop/agentic-loop.js';
import { createMemoryManager } from '../src/ai/memory/memory.js';
import { createMemoryMiddleware } from '../src/ai/memory/middleware.js';
import { createLocalEmbedder } from '../src/ai/acp/local-embedder.js';
import { createMemoryStorage, createSilentLogger } from './utils/mocks.js';

describe('E2E: memory middleware pipeline', () => {
	it('seeds knowledge, runs loop with middleware, verifies context injection and storage', async () => {
		// Use real local embedder (Hugging Face transformers)
		const embedder = createLocalEmbedder({
			model: 'Xenova/all-MiniLM-L6-v2',
			dtype: 'q8',
		});

		const manager = createMemoryManager(embedder, {
			enabled: true,
			embeddingAgent: 'local',
			similarityThreshold: 0,
			maxResults: 5,
		}, {
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			vectorStoreOptions: { autoSave: true, flushIntervalMs: 0, learning: { enabled: false } },
		});

		await manager.initialize();
		await manager.add('simse uses bun test for running tests', { topic: 'testing' });
		await manager.add('TypeScript strict mode is enabled', { topic: 'config' });

		const middleware = createMemoryMiddleware(manager, {
			maxResults: 3,
			storeTopic: 'e2e',
		});

		// Mock ACP client that returns a fixed response
		const mockAcpClient = {
			generateStream: mock(function* () {
				yield { type: 'delta' as const, text: 'Use bun test to run all tests.' };
			}),
		} as any;

		const mockToolRegistry = {
			formatForSystemPrompt: mock(() => ''),
			parseToolCalls: mock((text: string) => ({ text, toolCalls: [] })),
			execute: mock(async () => ({ id: '1', name: 'test', output: '', isError: false })),
			batchExecute: mock(async () => []),
			discover: mock(async () => {}),
			register: mock(() => {}),
			unregister: mock(() => false),
			getToolDefinitions: mock(() => []),
			toolCount: 0,
			toolNames: [],
		} as any;

		let capturedSystemPrompt = '';
		const mockConversation = {
			addUser: mock(() => {}),
			addAssistant: mock(() => {}),
			addToolResult: mock(() => {}),
			setSystemPrompt: mock((p: string) => { capturedSystemPrompt = p; }),
			serialize: mock(() => 'user: how do I run tests?'),
			needsCompaction: false,
			messageCount: 0,
			estimatedChars: 0,
			compact: mock(() => {}),
		} as any;

		const loop = createAgenticLoop({
			acpClient: mockAcpClient,
			toolRegistry: mockToolRegistry,
			conversation: mockConversation,
			memoryMiddleware: middleware,
			maxTurns: 1,
		});

		const result = await loop.run('how do I run tests?');

		// Verify memory context was injected into system prompt
		expect(capturedSystemPrompt).toContain('<memory-context>');
		expect(capturedSystemPrompt).toContain('bun test');

		// Verify response was stored in memory
		expect(manager.size).toBe(3); // 2 seeded + 1 stored
		expect(result.finalText).toBe('Use bun test to run all tests.');
	}, 30_000); // 30s timeout for model loading
});
```

**Step 2: Run E2E test**

Run: `bun test tests/e2e-memory-middleware.test.ts`
Expected: PASS (1 test, may take ~10s for embedder warm-up)

**Step 3: Commit**

```bash
git add tests/e2e-memory-middleware.test.ts
git commit -m "test: add E2E test for full memory middleware pipeline"
```

---

### Task 12: Run Full Test Suite & Lint

**Step 1: Run all tests**

Run: `bun test`
Expected: All tests pass (1346 existing + ~25 new)

**Step 2: Run typecheck**

Run: `bun x tsc --noEmit`
Expected: No errors

**Step 3: Run lint**

Run: `bun run lint`
Expected: No new errors from our changes

**Step 4: Fix any issues found**

**Step 5: Final commit**

```bash
git add -A
git commit -m "chore: fix lint and typecheck issues"
```
