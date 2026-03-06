# Librarian ACP Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend the Librarian to default to simse-engine ACP, support model switching for optimization, and auto-escalate to a powerful model (Opus 4.6) when library thresholds are exceeded.

**Architecture:** Add `generateWithModel()` to `TextGenerationProvider`, implement it in `createACPGenerator` via ACP `setSessionModel`, add `optimize()` to the Librarian, and wire auto-escalation into the CirculationDesk with dual thresholds (per-topic + global).

**Tech Stack:** TypeScript, Bun, ACP JSON-RPC 2.0, simse-engine

---

### Task 1: Extend TextGenerationProvider with generateWithModel

**Files:**
- Modify: `src/ai/library/types.ts:315-317`

**Context:** The `TextGenerationProvider` interface at line 315 currently only has `generate()`. We add an optional `generateWithModel()` method so providers that support model switching can expose it. Making it optional preserves backward compatibility — existing providers continue to work unchanged.

**Step 1: Add generateWithModel to TextGenerationProvider**

In `src/ai/library/types.ts`, replace the current `TextGenerationProvider` interface (lines 315-317):

```typescript
export interface TextGenerationProvider {
	readonly generate: (prompt: string, systemPrompt?: string) => Promise<string>;
	readonly generateWithModel?: (
		prompt: string,
		modelId: string,
		systemPrompt?: string,
	) => Promise<string>;
}
```

**Step 2: Run typecheck to verify no breakage**

Run: `bun run typecheck`
Expected: PASS — the new property is optional, so all existing providers still satisfy the interface.

**Step 3: Commit**

```bash
git add src/ai/library/types.ts
git commit -m "feat(library): add optional generateWithModel to TextGenerationProvider"
```

---

### Task 2: Implement generateWithModel in createACPGenerator

**Files:**
- Modify: `src/ai/acp/acp-adapters.ts:74-102`
- Test: `tests/acp-adapters.test.ts`

**Context:** `createACPGenerator` (line 74) currently returns `{ generate }`. The ACP client already has `setSessionModel(sessionId, modelId)` (see `acp-client.ts:1035`). The `generate` method creates a fresh session per call (`createSession(connection)` at line 583 of acp-client.ts). We need `generateWithModel` to do the same thing but call `setSessionModel` on the session before prompting.

However, `createACPGenerator` doesn't have direct access to `setSessionModel` — it uses `client.generate()` which internally creates a session. For `generateWithModel`, we need to use a lower-level flow: create a session, set the model, then prompt. But `client.generate()` already wraps all of this.

The simplest approach: `client.generate()` already creates a session and returns `sessionId` in the result. But we need to set the model *before* prompting. Looking at the `ACPClient` interface, the `generate` method handles session creation internally. So we need a two-step flow using `setSessionModel` on a newly created session.

The cleanest solution is to just expose the model switching through the existing `generate()` by calling `setSessionModel` after getting the sessionId from a first generate call... but that's wasteful. Instead, let's use a separate generate call per model request, setting the model on the session before the prompt.

Actually, looking more carefully: `client.generate()` creates a session, prompts it, and returns the result. There's no way to inject a model switch between session creation and prompting with the current `generate()` API.

The cleanest approach: add a `modelId` option to the `generate()` options object on `ACPClient`, which calls `setSessionModel` before prompting. But modifying `ACPClient.generate()` is out of scope for this plan.

**Simpler approach:** Since `client.generate()` returns a `sessionId`, and `setSessionModel` is best-effort, we can't use it to pre-switch before the first prompt. Instead, we'll:
1. Use `client.chat()` with the model concept, or
2. Add a simple approach: create a temporary session via a cheap initial prompt, set the model, then prompt with the real content.

**Actually the simplest approach:** The `ACPGenerateOptions` has a `config` field. We can check if the server supports model selection through that. But the canonical ACP way is `setSessionModel`.

**Final decision:** We'll extend `ACPGeneratorOptions` to accept the full `ACPClient` (it already does), and the `generateWithModel` implementation will:
1. Call `client.generate()` with a minimal placeholder to get a `sessionId`... No, that's wasteful.

**Best approach:** We'll modify `createACPGenerator` to directly call the lower-level ACP methods. But `ACPClient` doesn't expose `createSession` or `sendPrompt` directly.

**Pragmatic solution:** Add a `modelId` option to `ACPGenerateOptions` in `acp-client.ts` that triggers `setSessionModel` before prompting. This is a small, clean change to the ACP client that makes model switching available to all callers.

Wait — let me re-read the client. `generate` calls `createSession(connection)` → gets `sessionId` → `sendPrompt()`. We can insert `setSessionModel` between those steps. This would be a new optional field in the generate options.

**Step 1: Write the failing test for generateWithModel**

Add to `tests/acp-adapters.test.ts`:

```typescript
describe('createACPGenerator generateWithModel', () => {
	it('returns a provider with generateWithModel', () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });
		expect(typeof generator.generateWithModel).toBe('function');
	});

	it('delegates to client.generate with modelId option', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });
		const result = await generator.generateWithModel!(
			'optimize this',
			'claude-opus-4-6',
		);
		expect(result).toBe('generated text');
		expect(client.generate).toHaveBeenCalledWith('optimize this', {
			agentId: undefined,
			serverName: undefined,
			systemPrompt: undefined,
			modelId: 'claude-opus-4-6',
		});
	});

	it('passes systemPrompt through generateWithModel', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({
			client,
			systemPromptPrefix: 'prefix',
		});
		await generator.generateWithModel!(
			'prompt',
			'model-id',
			'system',
		);
		expect(client.generate).toHaveBeenCalledWith('prompt', {
			agentId: undefined,
			serverName: undefined,
			systemPrompt: 'prefix\n\nsystem',
			modelId: 'model-id',
		});
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/acp-adapters.test.ts`
Expected: FAIL — `generateWithModel` is not yet defined on the returned object.

**Step 3: Add modelId to ACPClient.generate options**

In `src/ai/acp/acp-client.ts`, modify the `generate` method's options type (around line 110) to include `modelId?: string`:

Find the `readonly generate:` interface declaration (line 108-116):

```typescript
	readonly generate: (
		prompt: string,
		options?: {
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
			config?: Readonly<Record<string, unknown>>;
			sampling?: ACPSamplingParams;
			modelId?: string;
		},
	) => Promise<ACPGenerateResult>;
```

And in the implementation (line 560-602), after `const sessionId = await createSession(connection);` (line 583), add the model switch:

```typescript
		return withResilience(name, 'generate', async () => {
			const sessionId = await createSession(connection);

			if (generateOptions?.modelId) {
				await setSessionModel(sessionId, generateOptions.modelId, name);
			}

			const content = buildTextContent(prompt, generateOptions?.systemPrompt);
```

Also update the generate implementation's options type at line 562-568 to include `modelId?: string`:

```typescript
		generateOptions?: {
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
			config?: Record<string, unknown>;
			sampling?: ACPSamplingParams;
			modelId?: string;
		},
```

**Step 4: Implement generateWithModel in createACPGenerator**

In `src/ai/acp/acp-adapters.ts`, replace the `return Object.freeze({...})` block (lines 79-101) with:

```typescript
	return Object.freeze({
		generate: async (prompt: string, systemPrompt?: string) => {
			try {
				const fullSystemPrompt =
					[systemPromptPrefix, systemPrompt].filter(Boolean).join('\n\n') ||
					undefined;

				const result = await client.generate(prompt, {
					agentId,
					serverName,
					systemPrompt: fullSystemPrompt,
				});
				return result.content;
			} catch (err) {
				const error = toError(err);
				throw createProviderGenerationError(
					agentId ?? 'default',
					`Generation failed: ${error.message}`,
					{ cause: err },
				);
			}
		},
		generateWithModel: async (
			prompt: string,
			modelId: string,
			systemPrompt?: string,
		) => {
			try {
				const fullSystemPrompt =
					[systemPromptPrefix, systemPrompt].filter(Boolean).join('\n\n') ||
					undefined;

				const result = await client.generate(prompt, {
					agentId,
					serverName,
					systemPrompt: fullSystemPrompt,
					modelId,
				});
				return result.content;
			} catch (err) {
				const error = toError(err);
				throw createProviderGenerationError(
					agentId ?? 'default',
					`Generation with model ${modelId} failed: ${error.message}`,
					{ cause: err },
				);
			}
		},
	});
```

**Step 5: Run tests to verify they pass**

Run: `bun test tests/acp-adapters.test.ts`
Expected: PASS

**Step 6: Run full typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 7: Commit**

```bash
git add src/ai/acp/acp-adapters.ts src/ai/acp/acp-client.ts tests/acp-adapters.test.ts
git commit -m "feat(acp): implement generateWithModel in ACP generator adapter"
```

---

### Task 3: Add OptimizationResult type and update Librarian interface

**Files:**
- Modify: `src/ai/library/types.ts:474-500`

**Context:** The `Librarian` interface (line 486) needs a new `optimize()` method. We also need an `OptimizationResult` type for the return value. The `optimize` method takes volumes, a topic, and a model ID, returning pruned IDs, a summary, and a reorganization plan.

**Step 1: Add OptimizationResult and update Librarian interface**

In `src/ai/library/types.ts`, after the `ReorganizationPlan` interface (line 484), add `OptimizationResult`, and update the `Librarian` interface:

```typescript
export interface OptimizationResult {
	readonly pruned: readonly string[];
	readonly summary: string;
	readonly reorganization: ReorganizationPlan;
	readonly modelUsed: string;
}

export interface Librarian {
	readonly extract: (turn: TurnContext) => Promise<ExtractionResult>;
	readonly summarize: (
		volumes: readonly Volume[],
		topic: string,
	) => Promise<{ text: string; sourceIds: readonly string[] }>;
	readonly classifyTopic: (
		text: string,
		existingTopics: readonly string[],
	) => Promise<ClassificationResult>;
	readonly reorganize: (
		topic: string,
		volumes: readonly Volume[],
	) => Promise<ReorganizationPlan>;
	readonly optimize: (
		volumes: readonly Volume[],
		topic: string,
		modelId: string,
	) => Promise<OptimizationResult>;
}
```

**Step 2: Run typecheck — expect failure**

Run: `bun run typecheck`
Expected: FAIL — `createLibrarian` in `librarian.ts` no longer satisfies the `Librarian` interface (missing `optimize`). Also, mock librarians in tests will fail.

This is expected. We'll fix it in the next task.

**Step 3: Commit the type changes**

```bash
git add src/ai/library/types.ts
git commit -m "feat(library): add OptimizationResult type and optimize to Librarian interface"
```

---

### Task 4: Implement optimize() in Librarian

**Files:**
- Modify: `src/ai/library/librarian.ts`
- Test: `tests/librarian.test.ts`

**Context:** `createLibrarian` (line 17 of `librarian.ts`) takes a `TextGenerationProvider`. The new `optimize()` method should use `generateWithModel()` when available, falling back to `generate()` if not. The LLM prompt asks the powerful model to identify volumes to prune, produce a summary, and suggest reorganization — all in one JSON response.

**Step 1: Write the failing tests**

Add to `tests/librarian.test.ts`:

```typescript
import type {
	TextGenerationProvider,
	Volume,
	OptimizationResult,
} from '../src/ai/library/types.js';

// Add a new mock that supports generateWithModel
function createMockGeneratorWithModel(
	defaultResponse: string,
	modelResponse: string,
): TextGenerationProvider {
	return {
		generate: mock(async () => defaultResponse),
		generateWithModel: mock(async () => modelResponse),
	};
}

describe('Librarian optimize', () => {
	it('optimize() uses generateWithModel when available', async () => {
		const optimizationResponse = JSON.stringify({
			pruned: ['v2'],
			summary: 'Condensed summary of database architecture.',
			reorganization: {
				moves: [],
				newSubtopics: [],
				merges: [],
			},
		});
		const generator = createMockGeneratorWithModel(
			'unused',
			optimizationResponse,
		);
		const librarian = createLibrarian(generator);
		const volumes: Volume[] = [
			{
				id: 'v1',
				text: 'Users table uses UUID PKs',
				embedding: [0.1],
				metadata: {},
				timestamp: 1,
			},
			{
				id: 'v2',
				text: 'Users table has UUID primary keys',
				embedding: [0.2],
				metadata: {},
				timestamp: 2,
			},
		];
		const result = await librarian.optimize(
			volumes,
			'architecture/database',
			'claude-opus-4-6',
		);
		expect(result.pruned).toEqual(['v2']);
		expect(result.summary.length).toBeGreaterThan(0);
		expect(result.modelUsed).toBe('claude-opus-4-6');
		expect(generator.generateWithModel).toHaveBeenCalled();
		expect(generator.generate).not.toHaveBeenCalled();
	});

	it('optimize() falls back to generate when generateWithModel is absent', async () => {
		const optimizationResponse = JSON.stringify({
			pruned: [],
			summary: 'Summary using default model.',
			reorganization: { moves: [], newSubtopics: [], merges: [] },
		});
		const generator = createMockGenerator(optimizationResponse);
		const librarian = createLibrarian(generator);
		const volumes: Volume[] = [
			{
				id: 'v1',
				text: 'Some fact',
				embedding: [0.1],
				metadata: {},
				timestamp: 1,
			},
		];
		const result = await librarian.optimize(volumes, 'test', 'any-model');
		expect(result.pruned).toEqual([]);
		expect(result.modelUsed).toBe('any-model');
		expect(generator.generate).toHaveBeenCalled();
	});

	it('optimize() returns safe defaults on LLM garbage', async () => {
		const generator = createMockGeneratorWithModel(
			'unused',
			'not valid json at all',
		);
		const librarian = createLibrarian(generator);
		const result = await librarian.optimize(
			[
				{
					id: 'v1',
					text: 'fact',
					embedding: [0.1],
					metadata: {},
					timestamp: 1,
				},
			],
			'test',
			'model-id',
		);
		expect(result.pruned).toEqual([]);
		expect(result.summary).toBe('');
		expect(result.reorganization.moves).toEqual([]);
		expect(result.modelUsed).toBe('model-id');
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test tests/librarian.test.ts`
Expected: FAIL — `optimize` is not defined on the returned object.

**Step 3: Implement optimize() in createLibrarian**

In `src/ai/library/librarian.ts`, add `OptimizationResult` to the imports:

```typescript
import type {
	ClassificationResult,
	ExtractionMemory,
	ExtractionResult,
	Librarian,
	OptimizationResult,
	ReorganizationPlan,
	TextGenerationProvider,
	TurnContext,
	Volume,
} from './types.js';
```

Then add the `optimize` method before the `return Object.freeze(...)` call (before line 156):

```typescript
	const optimize = async (
		volumes: readonly Volume[],
		topic: string,
		modelId: string,
	): Promise<OptimizationResult> => {
		const volumeList = volumes
			.map((v) => `- [${v.id}] ${v.text}`)
			.join('\n');

		const prompt = `You are a memory optimization agent. Analyze the following volumes in topic "${topic}" and perform maintenance.

Volumes:
${volumeList}

Tasks:
1. PRUNE: Identify volume IDs that are redundant, outdated, or low-value. List their IDs.
2. SUMMARIZE: Write a single concise summary that preserves all important information from the remaining (non-pruned) volumes.
3. REORGANIZE: Suggest any topic restructuring (moves, new subtopics, merges).

Return a JSON object:
{
  "pruned": ["id1", "id2"],
  "summary": "concise summary text",
  "reorganization": {
    "moves": [{"volumeId": "id", "newTopic": "new/topic"}],
    "newSubtopics": ["new/subtopic"],
    "merges": [{"source": "topic/a", "target": "topic/b"}]
  }
}

Respond with ONLY valid JSON.`;

		try {
			const response = textGenerator.generateWithModel
				? await textGenerator.generateWithModel(prompt, modelId)
				: await textGenerator.generate(prompt);
			const parsed = JSON.parse(response);
			return {
				pruned: Array.isArray(parsed.pruned) ? parsed.pruned : [],
				summary: typeof parsed.summary === 'string' ? parsed.summary : '',
				reorganization: {
					moves: Array.isArray(parsed.reorganization?.moves)
						? parsed.reorganization.moves
						: [],
					newSubtopics: Array.isArray(parsed.reorganization?.newSubtopics)
						? parsed.reorganization.newSubtopics
						: [],
					merges: Array.isArray(parsed.reorganization?.merges)
						? parsed.reorganization.merges
						: [],
				},
				modelUsed: modelId,
			};
		} catch {
			return {
				pruned: [],
				summary: '',
				reorganization: { moves: [], newSubtopics: [], merges: [] },
				modelUsed: modelId,
			};
		}
	};
```

Update the return to include `optimize`:

```typescript
	return Object.freeze({
		extract,
		summarize,
		classifyTopic,
		reorganize,
		optimize,
	});
```

**Step 4: Run tests to verify they pass**

Run: `bun test tests/librarian.test.ts`
Expected: PASS

**Step 5: Run typecheck**

Run: `bun run typecheck`
Expected: FAIL — mock librarians in test files (`tests/circulation-desk.test.ts`, `tests/library-services.test.ts`, etc.) are missing `optimize`. We'll fix those in the next task.

**Step 6: Commit**

```bash
git add src/ai/library/librarian.ts tests/librarian.test.ts
git commit -m "feat(library): implement optimize() in Librarian with model switching"
```

---

### Task 5: Fix mock Librarians in existing tests

**Files:**
- Modify: `tests/circulation-desk.test.ts:7-29`
- Modify: Any other test files with `Librarian` mocks (check `tests/library-services.test.ts`, `tests/builtin-tools.test.ts`)

**Context:** After adding `optimize` to the `Librarian` interface, all mock librarians in tests need to include the new method to satisfy TypeScript. Add `optimize: mock(async () => ({ pruned: [], summary: '', reorganization: { moves: [], newSubtopics: [], merges: [] }, modelUsed: '' }))` to each mock.

**Step 1: Update mock librarians in all test files**

Search for `createMockLibrarian` or inline `Librarian` mocks and add the `optimize` method.

In `tests/circulation-desk.test.ts`, update `createMockLibrarian()` (lines 8-29):

```typescript
function createMockLibrarian(): Librarian {
	return {
		extract: mock(async () => ({
			memories: [
				{
					text: 'Important fact',
					topic: 'test/topic',
					tags: ['important'],
					entryType: 'fact' as const,
				},
			],
		})),
		summarize: mock(async (volumes, topic) => ({
			text: 'Summarized content',
			sourceIds: volumes.map((v) => v.id),
		})),
		classifyTopic: mock(async () => ({ topic: 'test', confidence: 0.9 })),
		reorganize: mock(async () => ({
			moves: [],
			newSubtopics: [],
			merges: [],
		})),
		optimize: mock(async () => ({
			pruned: [],
			summary: '',
			reorganization: { moves: [], newSubtopics: [], merges: [] },
			modelUsed: '',
		})),
	};
}
```

Do the same for any other test files that mock the `Librarian` interface.

**Step 2: Run full test suite and typecheck**

Run: `bun run typecheck && bun test`
Expected: PASS — all tests pass, no type errors.

**Step 3: Commit**

```bash
git add tests/circulation-desk.test.ts tests/library-services.test.ts tests/builtin-tools.test.ts
git commit -m "fix(tests): add optimize to mock Librarians"
```

---

### Task 6: Add optimization thresholds and enqueueOptimization to CirculationDesk types

**Files:**
- Modify: `src/ai/library/types.ts:506-526`

**Context:** `CirculationDeskThresholds` (line 506) needs an `optimization` section with `topicThreshold` and `globalThreshold`. The `CirculationDesk` interface (line 517) needs `enqueueOptimization`.

**Step 1: Update CirculationDeskThresholds**

Replace lines 506-515:

```typescript
export interface CirculationDeskThresholds {
	readonly compendium?: {
		readonly minEntries?: number;
		readonly minAgeMs?: number;
		readonly deleteOriginals?: boolean;
	};
	readonly reorganization?: {
		readonly maxVolumesPerTopic?: number;
	};
	readonly optimization?: {
		readonly topicThreshold?: number;
		readonly globalThreshold?: number;
		readonly modelId: string;
	};
}
```

**Step 2: Update CirculationDesk interface**

Replace lines 517-526:

```typescript
export interface CirculationDesk {
	readonly enqueueExtraction: (turn: TurnContext) => void;
	readonly enqueueCompendium: (topic: string) => void;
	readonly enqueueReorganization: (topic: string) => void;
	readonly enqueueOptimization: (topic: string) => void;
	readonly drain: () => Promise<void>;
	readonly flush: () => Promise<void>;
	readonly dispose: () => void;
	readonly pending: number;
	readonly processing: boolean;
}
```

**Step 3: Run typecheck — expect failure**

Run: `bun run typecheck`
Expected: FAIL — `createCirculationDesk` doesn't return `enqueueOptimization` yet.

**Step 4: Commit**

```bash
git add src/ai/library/types.ts
git commit -m "feat(library): add optimization thresholds and enqueueOptimization to types"
```

---

### Task 7: Implement optimization in CirculationDesk

**Files:**
- Modify: `src/ai/library/circulation-desk.ts`
- Test: `tests/circulation-desk.test.ts`

**Context:** The `CirculationDesk` (line 27 of `circulation-desk.ts`) needs:
1. A new `optimization` job type
2. An `enqueueOptimization` method
3. Processing logic that calls `librarian.optimize()`, deletes pruned volumes, adds the summary as a new volume, and applies the reorganization plan
4. Auto-escalation: after extraction, check if any topic exceeds `topicThreshold` or total volumes exceed `globalThreshold`

The `CirculationDeskOptions` needs:
- `deleteVolume: (id: string) => Promise<void>` — to remove pruned volumes
- `getTotalVolumeCount: () => number` — to check global threshold
- `getAllTopics: () => string[]` — to iterate topics for global optimization

**Step 1: Write the failing tests**

Add to `tests/circulation-desk.test.ts`:

```typescript
describe('CirculationDesk optimization', () => {
	it('enqueueOptimization adds an optimization job', async () => {
		const librarian = createMockLibrarian();
		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
			deleteVolume: async () => {},
			getTotalVolumeCount: () => 0,
			getAllTopics: () => [],
			thresholds: {
				optimization: { modelId: 'claude-opus-4-6' },
			},
		});

		desk.enqueueOptimization('test/topic');
		expect(desk.pending).toBe(1);
		await desk.drain();
		expect(desk.pending).toBe(0);
	});

	it('optimization job calls librarian.optimize and deletes pruned volumes', async () => {
		const librarian = createMockLibrarian();
		(librarian.optimize as any).mockImplementation(async () => ({
			pruned: ['v2'],
			summary: 'Optimized summary',
			reorganization: { moves: [], newSubtopics: [], merges: [] },
			modelUsed: 'claude-opus-4-6',
		}));

		const deleteFn = mock(async () => {});
		const addFn = mock(async () => 'new-id');
		const volumes = [
			{
				id: 'v1',
				text: 'fact 1',
				embedding: [0.1],
				metadata: { topic: 'test' },
				timestamp: 1,
			},
			{
				id: 'v2',
				text: 'fact 2 (duplicate)',
				embedding: [0.2],
				metadata: { topic: 'test' },
				timestamp: 2,
			},
		];

		const desk = createCirculationDesk({
			librarian,
			addVolume: addFn,
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => volumes,
			deleteVolume: deleteFn,
			getTotalVolumeCount: () => 2,
			getAllTopics: () => ['test'],
			thresholds: {
				optimization: { modelId: 'claude-opus-4-6' },
			},
		});

		desk.enqueueOptimization('test');
		await desk.drain();
		expect(librarian.optimize).toHaveBeenCalled();
		expect(deleteFn).toHaveBeenCalledWith('v2');
		expect(addFn).toHaveBeenCalledWith('Optimized summary', {
			topic: 'test',
			entryType: 'compendium',
		});
	});

	it('auto-escalates when topic threshold is exceeded after extraction', async () => {
		const librarian = createMockLibrarian();
		const volumes = Array.from({ length: 51 }, (_, i) => ({
			id: `v${i}`,
			text: `fact ${i}`,
			embedding: [0.1],
			metadata: { topic: 'test/topic' },
			timestamp: i,
		}));

		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => volumes,
			deleteVolume: async () => {},
			getTotalVolumeCount: () => 51,
			getAllTopics: () => ['test/topic'],
			thresholds: {
				optimization: {
					topicThreshold: 50,
					modelId: 'claude-opus-4-6',
				},
			},
		});

		desk.enqueueExtraction({ userInput: 'x', response: 'y' });
		await desk.drain();
		// After extraction, auto-escalation should have enqueued an optimization job
		// which also gets drained
		expect(librarian.optimize).toHaveBeenCalled();
	});

	it('does not auto-escalate when thresholds are not exceeded', async () => {
		const librarian = createMockLibrarian();
		const desk = createCirculationDesk({
			librarian,
			addVolume: async () => 'id',
			checkDuplicate: async () => ({ isDuplicate: false }),
			getVolumesForTopic: () => [],
			deleteVolume: async () => {},
			getTotalVolumeCount: () => 5,
			getAllTopics: () => ['test'],
			thresholds: {
				optimization: {
					topicThreshold: 50,
					globalThreshold: 500,
					modelId: 'claude-opus-4-6',
				},
			},
		});

		desk.enqueueExtraction({ userInput: 'x', response: 'y' });
		await desk.drain();
		expect(librarian.optimize).not.toHaveBeenCalled();
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test tests/circulation-desk.test.ts`
Expected: FAIL — `enqueueOptimization` doesn't exist, `CirculationDeskOptions` doesn't accept `deleteVolume`/`getTotalVolumeCount`/`getAllTopics`.

**Step 3: Implement the changes**

Replace `src/ai/library/circulation-desk.ts` entirely:

```typescript
import type {
	CirculationDesk,
	CirculationDeskThresholds,
	DuplicateCheckResult,
	Librarian,
	TurnContext,
	Volume,
} from './types.js';

export interface CirculationDeskOptions {
	readonly librarian: Librarian;
	readonly addVolume: (
		text: string,
		metadata?: Record<string, string>,
	) => Promise<string>;
	readonly checkDuplicate: (text: string) => Promise<DuplicateCheckResult>;
	readonly getVolumesForTopic: (topic: string) => Volume[];
	readonly deleteVolume?: (id: string) => Promise<void>;
	readonly getTotalVolumeCount?: () => number;
	readonly getAllTopics?: () => string[];
	readonly thresholds?: CirculationDeskThresholds;
	readonly catalog?: import('./types.js').TopicCatalog;
}

type Job =
	| { type: 'extraction'; turn: TurnContext }
	| { type: 'compendium'; topic: string }
	| { type: 'reorganization'; topic: string }
	| { type: 'optimization'; topic: string };

export function createCirculationDesk(
	options: CirculationDeskOptions,
): CirculationDesk {
	const {
		librarian,
		addVolume,
		checkDuplicate,
		getVolumesForTopic,
		deleteVolume,
		getTotalVolumeCount,
		getAllTopics,
		catalog,
	} = options;
	const minEntries = options.thresholds?.compendium?.minEntries ?? 10;
	const maxVolumesPerTopic =
		options.thresholds?.reorganization?.maxVolumesPerTopic ?? 30;
	const optimizationConfig = options.thresholds?.optimization;
	const topicThreshold = optimizationConfig?.topicThreshold ?? 50;
	const globalThreshold = optimizationConfig?.globalThreshold ?? 500;

	const queue: Job[] = [];
	let isProcessing = false;
	let disposed = false;

	const checkEscalation = (topic: string): void => {
		if (!optimizationConfig || !deleteVolume) return;

		const topicVolumes = getVolumesForTopic(topic);
		if (topicVolumes.length >= topicThreshold) {
			queue.push({ type: 'optimization', topic });
			return;
		}

		if (getTotalVolumeCount && getAllTopics) {
			const total = getTotalVolumeCount();
			if (total >= globalThreshold) {
				for (const t of getAllTopics()) {
					queue.push({ type: 'optimization', topic: t });
				}
			}
		}
	};

	const processJob = async (job: Job): Promise<void> => {
		try {
			switch (job.type) {
				case 'extraction': {
					const result = await librarian.extract(job.turn);
					const extractedTopics = new Set<string>();
					for (const mem of result.memories) {
						const dup = await checkDuplicate(mem.text);
						if (dup.isDuplicate) continue;

						const topic = catalog
							? catalog.resolve(mem.topic)
							: mem.topic;

						await addVolume(mem.text, {
							topic,
							tags: mem.tags.join(','),
							entryType: mem.entryType,
						});
						extractedTopics.add(topic);
					}
					for (const topic of extractedTopics) {
						checkEscalation(topic);
					}
					break;
				}
				case 'compendium': {
					const volumes = getVolumesForTopic(job.topic);
					if (volumes.length >= minEntries) {
						await librarian.summarize(volumes, job.topic);
					}
					break;
				}
				case 'reorganization': {
					const volumes = getVolumesForTopic(job.topic);
					if (volumes.length >= maxVolumesPerTopic) {
						const plan = await librarian.reorganize(
							job.topic,
							volumes,
						);
						if (catalog) {
							for (const move of plan.moves) {
								catalog.relocate(move.volumeId, move.newTopic);
							}
							for (const merge of plan.merges) {
								catalog.merge(merge.source, merge.target);
							}
						}
					}
					break;
				}
				case 'optimization': {
					if (!deleteVolume || !optimizationConfig) break;
					const volumes = getVolumesForTopic(job.topic);
					if (volumes.length === 0) break;
					const result = await librarian.optimize(
						volumes,
						job.topic,
						optimizationConfig.modelId,
					);
					for (const id of result.pruned) {
						await deleteVolume(id);
					}
					if (result.summary) {
						await addVolume(result.summary, {
							topic: job.topic,
							entryType: 'compendium',
						});
					}
					if (catalog) {
						for (const move of result.reorganization.moves) {
							catalog.relocate(move.volumeId, move.newTopic);
						}
						for (const merge of result.reorganization.merges) {
							catalog.merge(merge.source, merge.target);
						}
					}
					break;
				}
			}
		} catch {
			// Failed jobs are logged and dropped (fire-and-forget)
		}
	};

	const drain = async (): Promise<void> => {
		if (isProcessing || disposed) return;
		isProcessing = true;
		try {
			while (queue.length > 0) {
				const job = queue.shift()!;
				await processJob(job);
			}
		} finally {
			isProcessing = false;
		}
	};

	const flush = async (): Promise<void> => {
		queue.length = 0;
	};

	const dispose = (): void => {
		disposed = true;
		queue.length = 0;
	};

	return Object.freeze({
		enqueueExtraction: (turn: TurnContext) => {
			if (disposed) return;
			queue.push({ type: 'extraction', turn });
		},
		enqueueCompendium: (topic: string) => {
			if (disposed) return;
			queue.push({ type: 'compendium', topic });
		},
		enqueueReorganization: (topic: string) => {
			if (disposed) return;
			queue.push({ type: 'reorganization', topic });
		},
		enqueueOptimization: (topic: string) => {
			if (disposed) return;
			queue.push({ type: 'optimization', topic });
		},
		drain,
		flush,
		dispose,
		get pending() {
			return queue.length;
		},
		get processing() {
			return isProcessing;
		},
	});
}
```

**Step 4: Run tests to verify they pass**

Run: `bun test tests/circulation-desk.test.ts`
Expected: PASS

**Step 5: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 6: Commit**

```bash
git add src/ai/library/circulation-desk.ts tests/circulation-desk.test.ts
git commit -m "feat(library): implement optimization jobs and auto-escalation in CirculationDesk"
```

---

### Task 8: Add createDefaultLibrarian convenience factory

**Files:**
- Modify: `src/ai/library/librarian.ts`
- Test: `tests/librarian.test.ts`

**Context:** `createDefaultLibrarian(acpClient)` is a convenience that wraps an ACP client as a `TextGenerationProvider` and passes it to `createLibrarian`. Import `createACPGenerator` from the ACP adapters module.

**Step 1: Write the failing test**

Add to `tests/librarian.test.ts`:

```typescript
import { createDefaultLibrarian } from '../src/ai/library/librarian.js';
import type { ACPClient } from '../src/ai/acp/acp-client.js';

// Reuse the mock ACP client helper pattern from acp-adapters.test.ts
function createMockACPClientForLibrarian(): ACPClient {
	return {
		initialize: mock(() => Promise.resolve()),
		dispose: mock(() => Promise.resolve()),
		listAgents: mock(() => Promise.resolve([])),
		getAgent: mock(() =>
			Promise.resolve({ id: 'test', name: 'test' }),
		),
		generate: mock(() =>
			Promise.resolve({
				content: JSON.stringify({ memories: [] }),
				agentId: 'agent-1',
				serverName: 'server-1',
				sessionId: 'sess-1',
			}),
		),
		chat: mock(() =>
			Promise.resolve({
				content: 'chat',
				agentId: 'agent-1',
				serverName: 'server-1',
				sessionId: 'sess-1',
			}),
		),
		generateStream: mock(async function* () {
			yield { type: 'delta' as const, text: 'chunk' };
		}),
		embed: mock(() =>
			Promise.resolve({
				embeddings: [[0.1]],
				agentId: 'agent-1',
				serverName: 'server-1',
			}),
		),
		isAvailable: mock(() => Promise.resolve(true)),
		setPermissionPolicy: mock(() => {}),
		listSessions: mock(() => Promise.resolve([])),
		loadSession: mock(() => Promise.resolve({} as any)),
		deleteSession: mock(() => Promise.resolve()),
		setSessionMode: mock(() => Promise.resolve()),
		setSessionModel: mock(() => Promise.resolve()),
		serverNames: ['server-1'],
		serverCount: 1,
		defaultServerName: 'server-1',
		defaultAgent: 'agent-1',
	} as ACPClient;
}

describe('createDefaultLibrarian', () => {
	it('creates a Librarian from an ACP client', async () => {
		const client = createMockACPClientForLibrarian();
		const librarian = createDefaultLibrarian(client);
		expect(typeof librarian.extract).toBe('function');
		expect(typeof librarian.optimize).toBe('function');
		const result = await librarian.extract({
			userInput: 'hello',
			response: 'world',
		});
		expect(result.memories).toEqual([]);
		expect(client.generate).toHaveBeenCalled();
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/librarian.test.ts`
Expected: FAIL — `createDefaultLibrarian` is not exported.

**Step 3: Implement createDefaultLibrarian**

In `src/ai/library/librarian.ts`, add the import and factory:

```typescript
import { createACPGenerator } from '../acp/acp-adapters.js';
import type { ACPClient } from '../acp/acp-client.js';
```

At the end of the file, add:

```typescript
export function createDefaultLibrarian(acpClient: ACPClient): Librarian {
	const provider = createACPGenerator({ client: acpClient });
	return createLibrarian(provider);
}
```

**Step 4: Run tests to verify they pass**

Run: `bun test tests/librarian.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/ai/library/librarian.ts tests/librarian.test.ts
git commit -m "feat(library): add createDefaultLibrarian convenience factory"
```

---

### Task 9: Update lib.ts exports and CLAUDE.md

**Files:**
- Modify: `src/lib.ts`
- Modify: `CLAUDE.md`

**Context:** The public API surface needs to export the new types and factory. Also update CLAUDE.md to document the new capabilities.

**Step 1: Update lib.ts exports**

In `src/lib.ts`, add `createDefaultLibrarian` to the librarian export (around line 203):

```typescript
export { createLibrarian, createDefaultLibrarian } from './ai/library/librarian.js';
```

Add `OptimizationResult` to the type exports from `types.ts` (around line 210):

```typescript
	OptimizationResult,
```

**Step 2: Update CLAUDE.md**

In the `librarian.ts` description in the Module Layout section, update to mention `optimize()` and `createDefaultLibrarian`:

```
      librarian.ts            # Librarian (createLibrarian, createDefaultLibrarian):
                               # extract, summarize, classifyTopic, reorganize, optimize
                               # createDefaultLibrarian wraps ACPClient for convenience
```

In the Library System section under "Higher-level services", update:

```
- **Librarian** (`librarian.ts`): LLM-driven memory extraction, summarization, classification, reorganization, and optimization. `createDefaultLibrarian(acpClient)` wraps any ACP client. `optimize()` uses `generateWithModel()` for powerful-model maintenance (prune, summarize, reorganize).
- **CirculationDesk** (`circulation-desk.ts`): Async background job queue for extraction, compendium, reorganization, and optimization. Dual-threshold auto-escalation: per-topic (`topicThreshold`, default 50) and global (`globalThreshold`, default 500) trigger optimization with a powerful model.
```

**Step 3: Run typecheck and tests**

Run: `bun run typecheck && bun test`
Expected: PASS

**Step 4: Commit**

```bash
git add src/lib.ts CLAUDE.md
git commit -m "docs: update exports and CLAUDE.md for Librarian ACP integration"
```

---

### Task 10: Final verification

**Step 1: Run full test suite**

Run: `bun test`
Expected: All tests pass.

**Step 2: Run lint**

Run: `bun run lint`
Expected: No errors.

**Step 3: Run typecheck**

Run: `bun run typecheck`
Expected: No errors.

**Step 4: Push**

```bash
git push
```
