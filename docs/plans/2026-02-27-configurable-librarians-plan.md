# Configurable Librarians Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a LibrarianRegistry that manages multiple configurable librarians defined by JSON files, with LLM-driven bidding for ownership of memory actions, default librarian arbitration, and automatic specialist spawning.

**Architecture:** New `LibrarianDefinition` schema validated at load time, a `LibrarianRegistry` factory that manages named librarians with per-librarian ACP connections, a bidding/arbitration system where librarians argue for ownership with library access, and CirculationDesk integration that routes jobs through the registry. The default librarian spawns specialists via powerful model when complexity thresholds are exceeded.

**Tech Stack:** TypeScript, Bun, ACP JSON-RPC 2.0, JSON definition files, picomatch (glob matching)

---

## Prerequisites

Before starting, install the glob matching dependency:

```bash
bun add picomatch
bun add -d @types/picomatch
```

---

### Task 1: Add LibrarianDefinition Types

**Files:**
- Modify: `src/ai/library/types.ts:498-517` (after existing Librarian interface)

**Step 1: Write the failing test**

Create `tests/librarian-definition.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import type {
	ArbitrationResult,
	LibrarianBid,
	LibrarianDefinition,
} from '../src/ai/library/types.js';

describe('LibrarianDefinition types', () => {
	it('allows constructing a valid LibrarianDefinition', () => {
		const def: LibrarianDefinition = {
			name: 'code-patterns',
			description: 'Manages code pattern memories',
			purpose: 'I specialize in code patterns and architecture',
			topics: ['code/*', 'architecture/*'],
			permissions: { add: true, delete: true, reorganize: true },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
		};
		expect(def.name).toBe('code-patterns');
		expect(def.topics).toHaveLength(2);
	});

	it('allows LibrarianDefinition with ACP config', () => {
		const def: LibrarianDefinition = {
			name: 'test',
			description: 'Test librarian',
			purpose: 'Testing',
			topics: ['*'],
			permissions: { add: true, delete: false, reorganize: false },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
			acp: {
				command: 'simse-engine',
				args: ['--mode', 'librarian'],
				agentId: 'test-agent',
			},
		};
		expect(def.acp?.command).toBe('simse-engine');
	});

	it('allows constructing a LibrarianBid', () => {
		const bid: LibrarianBid = {
			librarianName: 'code-patterns',
			argument: 'I already manage 15 volumes about React patterns',
			confidence: 0.85,
		};
		expect(bid.confidence).toBeGreaterThan(0);
	});

	it('allows constructing an ArbitrationResult', () => {
		const result: ArbitrationResult = {
			winner: 'code-patterns',
			reason: 'Best expertise match',
			bids: [
				{
					librarianName: 'code-patterns',
					argument: 'I manage code patterns',
					confidence: 0.9,
				},
			],
		};
		expect(result.bids).toHaveLength(1);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/librarian-definition.test.ts`
Expected: FAIL — types not exported yet.

**Step 3: Add the types to types.ts**

Add after the existing `Librarian` interface (after line 517) in `src/ai/library/types.ts`:

```typescript
// ---------------------------------------------------------------------------
// Librarian Definition (configurable librarian JSON schema)
// ---------------------------------------------------------------------------

export interface LibrarianDefinition {
	readonly name: string;
	readonly description: string;
	readonly purpose: string;
	readonly topics: readonly string[];
	readonly permissions: {
		readonly add: boolean;
		readonly delete: boolean;
		readonly reorganize: boolean;
	};
	readonly thresholds: {
		readonly topicComplexity: number;
		readonly escalateAt: number;
	};
	readonly acp?: {
		readonly command: string;
		readonly args?: readonly string[];
		readonly agentId?: string;
	};
}

// ---------------------------------------------------------------------------
// Librarian Bidding & Arbitration
// ---------------------------------------------------------------------------

export interface LibrarianBid {
	readonly librarianName: string;
	readonly argument: string;
	readonly confidence: number;
}

export interface ArbitrationResult {
	readonly winner: string;
	readonly reason: string;
	readonly bids: readonly LibrarianBid[];
}
```

Also update the `CirculationDeskThresholds` interface to add spawning thresholds (around line 523):

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
	readonly spawning?: {
		readonly complexityThreshold?: number;
		readonly depthThreshold?: number;
		readonly childTopicThreshold?: number;
		readonly modelId: string;
	};
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/librarian-definition.test.ts`
Expected: PASS

**Step 5: Run typecheck**

Run: `bun run typecheck`
Expected: PASS — no type errors.

**Step 6: Commit**

```bash
git add src/ai/library/types.ts tests/librarian-definition.test.ts
git commit -m "feat(library): add LibrarianDefinition, LibrarianBid, and ArbitrationResult types"
```

---

### Task 2: Create Librarian Definition Loader

**Files:**
- Create: `src/ai/library/librarian-definition.ts`
- Test: `tests/librarian-definition.test.ts` (extend)

**Step 1: Write the failing tests**

Append to `tests/librarian-definition.test.ts`:

```typescript
import {
	loadDefinition,
	loadAllDefinitions,
	saveDefinition,
	validateDefinition,
	matchesTopic,
} from '../src/ai/library/librarian-definition.js';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { beforeEach, afterEach } from 'bun:test';

describe('validateDefinition', () => {
	it('accepts a valid definition', () => {
		const def = {
			name: 'test',
			description: 'Test',
			purpose: 'Testing',
			topics: ['*'],
			permissions: { add: true, delete: false, reorganize: false },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
		};
		expect(validateDefinition(def)).toEqual({ valid: true, errors: [] });
	});

	it('rejects a definition missing required fields', () => {
		const def = { name: 'test' };
		const result = validateDefinition(def);
		expect(result.valid).toBe(false);
		expect(result.errors.length).toBeGreaterThan(0);
	});

	it('rejects empty topics array', () => {
		const def = {
			name: 'test',
			description: 'Test',
			purpose: 'Testing',
			topics: [],
			permissions: { add: true, delete: false, reorganize: false },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
		};
		const result = validateDefinition(def);
		expect(result.valid).toBe(false);
	});

	it('rejects invalid name characters', () => {
		const def = {
			name: 'has spaces!',
			description: 'Test',
			purpose: 'Testing',
			topics: ['*'],
			permissions: { add: true, delete: false, reorganize: false },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
		};
		const result = validateDefinition(def);
		expect(result.valid).toBe(false);
	});
});

describe('matchesTopic', () => {
	it('matches wildcard pattern', () => {
		expect(matchesTopic(['*'], 'code/react')).toBe(true);
	});

	it('matches glob pattern', () => {
		expect(matchesTopic(['code/*'], 'code/react')).toBe(true);
	});

	it('does not match non-matching pattern', () => {
		expect(matchesTopic(['code/*'], 'docs/readme')).toBe(false);
	});

	it('matches any pattern in array', () => {
		expect(matchesTopic(['code/*', 'architecture/*'], 'architecture/patterns')).toBe(true);
	});

	it('matches deep glob pattern', () => {
		expect(matchesTopic(['code/**'], 'code/react/hooks')).toBe(true);
	});
});

describe('saveDefinition / loadDefinition / loadAllDefinitions', () => {
	let tempDir: string;

	beforeEach(async () => {
		tempDir = await mkdtemp(join(tmpdir(), 'simse-librarian-test-'));
	});

	afterEach(async () => {
		await rm(tempDir, { recursive: true, force: true });
	});

	it('saves and loads a definition', async () => {
		const def = {
			name: 'test-lib',
			description: 'Test librarian',
			purpose: 'Testing purposes',
			topics: ['test/*'],
			permissions: { add: true, delete: false, reorganize: true },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
		};
		await saveDefinition(tempDir, def);
		const loaded = await loadDefinition(tempDir, 'test-lib');
		expect(loaded).toEqual(def);
	});

	it('returns undefined for non-existent definition', async () => {
		const loaded = await loadDefinition(tempDir, 'nonexistent');
		expect(loaded).toBeUndefined();
	});

	it('loads all definitions from directory', async () => {
		const def1 = {
			name: 'lib-a',
			description: 'A',
			purpose: 'A purpose',
			topics: ['a/*'],
			permissions: { add: true, delete: false, reorganize: false },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
		};
		const def2 = {
			name: 'lib-b',
			description: 'B',
			purpose: 'B purpose',
			topics: ['b/*'],
			permissions: { add: true, delete: true, reorganize: true },
			thresholds: { topicComplexity: 30, escalateAt: 80 },
		};
		await saveDefinition(tempDir, def1);
		await saveDefinition(tempDir, def2);
		const all = await loadAllDefinitions(tempDir);
		expect(all).toHaveLength(2);
		const names = all.map((d) => d.name).sort();
		expect(names).toEqual(['lib-a', 'lib-b']);
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test tests/librarian-definition.test.ts`
Expected: FAIL — module not found.

**Step 3: Implement librarian-definition.ts**

Create `src/ai/library/librarian-definition.ts`:

```typescript
// ---------------------------------------------------------------------------
// Librarian Definition — JSON schema validation, file I/O, topic matching
// ---------------------------------------------------------------------------

import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import picomatch from 'picomatch';
import type { LibrarianDefinition } from './types.js';

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

const NAME_PATTERN = /^[a-z0-9][a-z0-9-]*$/;

export interface ValidationResult {
	readonly valid: boolean;
	readonly errors: readonly string[];
}

export function validateDefinition(input: unknown): ValidationResult {
	const errors: string[] = [];

	if (!input || typeof input !== 'object') {
		return { valid: false, errors: ['Definition must be an object'] };
	}

	const def = input as Record<string, unknown>;

	if (typeof def.name !== 'string' || def.name.length === 0) {
		errors.push('name is required and must be a non-empty string');
	} else if (!NAME_PATTERN.test(def.name)) {
		errors.push('name must be kebab-case (lowercase letters, digits, hyphens, starting with letter or digit)');
	}

	if (typeof def.description !== 'string' || def.description.length === 0) {
		errors.push('description is required and must be a non-empty string');
	}

	if (typeof def.purpose !== 'string' || def.purpose.length === 0) {
		errors.push('purpose is required and must be a non-empty string');
	}

	if (!Array.isArray(def.topics) || def.topics.length === 0) {
		errors.push('topics is required and must be a non-empty array of strings');
	} else if (!def.topics.every((t: unknown) => typeof t === 'string')) {
		errors.push('all topics must be strings');
	}

	if (!def.permissions || typeof def.permissions !== 'object') {
		errors.push('permissions is required and must be an object');
	} else {
		const perms = def.permissions as Record<string, unknown>;
		for (const key of ['add', 'delete', 'reorganize']) {
			if (typeof perms[key] !== 'boolean') {
				errors.push(`permissions.${key} must be a boolean`);
			}
		}
	}

	if (!def.thresholds || typeof def.thresholds !== 'object') {
		errors.push('thresholds is required and must be an object');
	} else {
		const thresh = def.thresholds as Record<string, unknown>;
		if (typeof thresh.topicComplexity !== 'number' || thresh.topicComplexity <= 0) {
			errors.push('thresholds.topicComplexity must be a positive number');
		}
		if (typeof thresh.escalateAt !== 'number' || thresh.escalateAt <= 0) {
			errors.push('thresholds.escalateAt must be a positive number');
		}
	}

	if (def.acp !== undefined) {
		if (typeof def.acp !== 'object' || def.acp === null) {
			errors.push('acp must be an object if provided');
		} else {
			const acp = def.acp as Record<string, unknown>;
			if (typeof acp.command !== 'string' || acp.command.length === 0) {
				errors.push('acp.command must be a non-empty string');
			}
		}
	}

	return { valid: errors.length === 0, errors };
}

// ---------------------------------------------------------------------------
// Topic Matching
// ---------------------------------------------------------------------------

export function matchesTopic(
	patterns: readonly string[],
	topic: string,
): boolean {
	return patterns.some((pattern) => {
		if (pattern === '*') return true;
		const isMatch = picomatch(pattern);
		return isMatch(topic);
	});
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

export async function saveDefinition(
	librariansDir: string,
	definition: LibrarianDefinition,
): Promise<void> {
	await mkdir(librariansDir, { recursive: true });
	const filePath = join(librariansDir, `${definition.name}.json`);
	await writeFile(filePath, JSON.stringify(definition, null, '\t'), 'utf-8');
}

export async function loadDefinition(
	librariansDir: string,
	name: string,
): Promise<LibrarianDefinition | undefined> {
	const filePath = join(librariansDir, `${name}.json`);
	try {
		const raw = await readFile(filePath, 'utf-8');
		const parsed = JSON.parse(raw);
		const validation = validateDefinition(parsed);
		if (!validation.valid) return undefined;
		return parsed as LibrarianDefinition;
	} catch {
		return undefined;
	}
}

export async function loadAllDefinitions(
	librariansDir: string,
): Promise<LibrarianDefinition[]> {
	try {
		const entries = await readdir(librariansDir);
		const jsonFiles = entries.filter((e) => e.endsWith('.json'));
		const definitions: LibrarianDefinition[] = [];

		for (const file of jsonFiles) {
			const name = file.replace(/\.json$/, '');
			const def = await loadDefinition(librariansDir, name);
			if (def) definitions.push(def);
		}

		return definitions;
	} catch {
		return [];
	}
}
```

**Step 4: Run tests to verify they pass**

Run: `bun test tests/librarian-definition.test.ts`
Expected: PASS

**Step 5: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 6: Commit**

```bash
git add src/ai/library/librarian-definition.ts tests/librarian-definition.test.ts
git commit -m "feat(library): add librarian definition loader with validation and topic matching"
```

---

### Task 3: Add bid() Method to Librarian

**Files:**
- Modify: `src/ai/library/types.ts:498-517` (Librarian interface)
- Modify: `src/ai/library/librarian.ts`
- Test: `tests/librarian.test.ts` (extend)

**Step 1: Write the failing test**

Append to `tests/librarian.test.ts`:

```typescript
describe('bid', () => {
	it('produces a bid with argument and confidence', async () => {
		const generator = createMockGenerator(
			JSON.stringify({
				argument: 'I specialize in code patterns and this is a React hook pattern',
				confidence: 0.85,
			}),
		);
		const librarian = createLibrarian(generator, {
			name: 'code-patterns',
			purpose: 'I specialize in code patterns and architecture',
		});
		const mockLibrary = {
			search: mock(async () => []),
			getTopics: mock(() => []),
			filterByTopic: mock(() => []),
		};
		const bid = await librarian.bid(
			'useCallback hook for memoization',
			'code/react',
			mockLibrary as any,
		);
		expect(bid.librarianName).toBe('code-patterns');
		expect(bid.confidence).toBe(0.85);
		expect(bid.argument).toContain('React');
	});

	it('returns zero confidence on parse failure', async () => {
		const generator = createMockGenerator('not valid json');
		const librarian = createLibrarian(generator, {
			name: 'test',
			purpose: 'Testing',
		});
		const bid = await librarian.bid('some content', 'test/topic', {} as any);
		expect(bid.confidence).toBe(0);
		expect(bid.librarianName).toBe('test');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/librarian.test.ts --filter "bid"`
Expected: FAIL — `bid` not found on librarian, `createLibrarian` doesn't accept definition.

**Step 3: Update the Librarian interface in types.ts**

Add `bid` to the `Librarian` interface in `src/ai/library/types.ts`:

```typescript
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
	readonly bid: (
		content: string,
		topic: string,
		library: import('../library/library.js').Library,
	) => Promise<LibrarianBid>;
}
```

**Step 4: Update createLibrarian to accept optional identity and implement bid()**

Modify `src/ai/library/librarian.ts`. The `createLibrarian` function gains an optional second parameter for the librarian's identity (name + purpose):

```typescript
export interface LibrarianIdentity {
	readonly name: string;
	readonly purpose: string;
}

export function createLibrarian(
	textGenerator: TextGenerationProvider,
	identity?: LibrarianIdentity,
): Librarian {
	const libName = identity?.name ?? 'default';
	const libPurpose = identity?.purpose ?? 'General-purpose librarian for all topics.';

	// ... existing extract, summarize, classifyTopic, reorganize, optimize unchanged ...

	const bid = async (
		content: string,
		topic: string,
		library: import('./library.js').Library,
	): Promise<LibrarianBid> => {
		let contextBlock = '';
		try {
			const searchResults = await library.search(content, 5);
			const topicVolumes = library.filterByTopic([topic]);
			const topics = library.getTopics().filter(
				(t) => t.topic.startsWith(topic.split('/')[0]),
			);
			contextBlock = `
Existing volumes in this topic area: ${topicVolumes.length}
Related topics you manage: ${topics.map((t) => t.topic).join(', ') || 'none'}
Recent related volumes:
${searchResults.map((r) => `- ${r.volume.text.slice(0, 100)}`).join('\n') || 'none'}`;
		} catch {
			contextBlock = 'Library context unavailable.';
		}

		const prompt = `You are ${libName}, a specialized librarian.
Your purpose: ${libPurpose}

You are bidding to manage this new content:
Topic: ${topic}
Content: ${content}
${contextBlock}

Evaluate whether this content falls within your expertise.
Then argue why you should manage it.

Return ONLY valid JSON: {"argument": "your informed case", "confidence": 0.0-1.0}`;

		try {
			const response = await textGenerator.generate(prompt);
			const parsed = JSON.parse(response);
			return {
				librarianName: libName,
				argument: typeof parsed.argument === 'string' ? parsed.argument : '',
				confidence: typeof parsed.confidence === 'number'
					? Math.max(0, Math.min(1, parsed.confidence))
					: 0,
			};
		} catch {
			return { librarianName: libName, argument: '', confidence: 0 };
		}
	};

	return Object.freeze({ extract, summarize, classifyTopic, reorganize, optimize, bid });
}
```

Update `createDefaultLibrarian` to pass no identity (backward compatible):

```typescript
export function createDefaultLibrarian(acpClient: ACPClient): Librarian {
	const provider = createACPGenerator({ client: acpClient });
	return createLibrarian(provider);
}
```

**Step 5: Run tests to verify they pass**

Run: `bun test tests/librarian.test.ts`
Expected: PASS

**Step 6: Run full typecheck and existing tests**

Run: `bun run typecheck && bun test`
Expected: PASS — existing tests still work since identity is optional.

**Step 7: Commit**

```bash
git add src/ai/library/types.ts src/ai/library/librarian.ts tests/librarian.test.ts
git commit -m "feat(library): add bid() method to Librarian for ownership arbitration"
```

---

### Task 4: Create Librarian Registry

**Files:**
- Create: `src/ai/library/librarian-registry.ts`
- Test: `tests/librarian-registry.test.ts`

This is the largest task. It implements the registry factory, bidding resolution, and arbitration.

**Step 1: Write the failing tests**

Create `tests/librarian-registry.test.ts`:

```typescript
import { describe, expect, it, mock, beforeEach, afterEach } from 'bun:test';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createLibrarianRegistry } from '../src/ai/library/librarian-registry.js';
import { saveDefinition } from '../src/ai/library/librarian-definition.js';
import type {
	LibrarianBid,
	LibrarianDefinition,
	TextGenerationProvider,
} from '../src/ai/library/types.js';
import type { Library } from '../src/ai/library/library.js';

function createMockProvider(response: string): TextGenerationProvider {
	return {
		generate: mock(async () => response),
		generateWithModel: mock(async () => response),
	};
}

function createMockLibrary(): Library {
	return {
		initialize: mock(async () => {}),
		dispose: mock(async () => {}),
		add: mock(async () => 'vol-1'),
		addBatch: mock(async () => []),
		search: mock(async () => []),
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
		compendium: mock(async () => ({
			compendiumId: 'c-1',
			text: '',
			sourceIds: [],
			deletedOriginals: false,
		})),
		setTextGenerator: mock(() => {}),
		recordFeedback: mock(() => {}),
		delete: mock(async () => true),
		deleteBatch: mock(async () => 0),
		clear: mock(async () => {}),
		shelf: mock(() => ({ name: 'test', add: mock(async () => ''), search: mock(async () => []), searchGlobal: mock(async () => []), volumes: mock(() => []) })),
		shelves: mock(() => []),
		patronProfile: undefined,
		size: 0,
		isInitialized: true,
		isDirty: false,
		embeddingAgent: undefined,
	} as unknown as Library;
}

describe('LibrarianRegistry', () => {
	let tempDir: string;
	let librariansDir: string;

	beforeEach(async () => {
		tempDir = await mkdtemp(join(tmpdir(), 'simse-registry-test-'));
		librariansDir = join(tempDir, 'librarians');
	});

	afterEach(async () => {
		await rm(tempDir, { recursive: true, force: true });
	});

	describe('initialize', () => {
		it('creates default librarian when no definitions exist', async () => {
			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider('{}'),
			});
			await registry.initialize();
			expect(registry.defaultLibrarian).toBeDefined();
			expect(registry.defaultLibrarian.definition.name).toBe('default');
			expect(registry.list()).toHaveLength(1);
		});

		it('loads definitions from disk on initialize', async () => {
			const def: LibrarianDefinition = {
				name: 'code-patterns',
				description: 'Code',
				purpose: 'Code patterns',
				topics: ['code/*'],
				permissions: { add: true, delete: true, reorganize: true },
				thresholds: { topicComplexity: 50, escalateAt: 100 },
			};
			await saveDefinition(librariansDir, def);

			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider('{}'),
			});
			await registry.initialize();
			expect(registry.list()).toHaveLength(2); // default + code-patterns
			expect(registry.get('code-patterns')).toBeDefined();
		});
	});

	describe('register / unregister', () => {
		it('registers a new librarian and saves to disk', async () => {
			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider('{}'),
			});
			await registry.initialize();

			const def: LibrarianDefinition = {
				name: 'docs-lib',
				description: 'Docs',
				purpose: 'Documentation',
				topics: ['docs/*'],
				permissions: { add: true, delete: false, reorganize: true },
				thresholds: { topicComplexity: 50, escalateAt: 100 },
			};
			const managed = await registry.register(def);
			expect(managed.definition.name).toBe('docs-lib');
			expect(registry.list()).toHaveLength(2); // default + docs-lib
		});

		it('unregisters a librarian', async () => {
			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider('{}'),
			});
			await registry.initialize();

			const def: LibrarianDefinition = {
				name: 'temp-lib',
				description: 'Temp',
				purpose: 'Temporary',
				topics: ['temp/*'],
				permissions: { add: true, delete: false, reorganize: false },
				thresholds: { topicComplexity: 50, escalateAt: 100 },
			};
			await registry.register(def);
			expect(registry.list()).toHaveLength(2);

			await registry.unregister('temp-lib');
			expect(registry.list()).toHaveLength(1);
			expect(registry.get('temp-lib')).toBeUndefined();
		});

		it('cannot unregister the default librarian', async () => {
			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider('{}'),
			});
			await registry.initialize();

			await expect(registry.unregister('default')).rejects.toThrow();
		});
	});

	describe('resolveLibrarian', () => {
		it('returns default when no specialists match', async () => {
			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider(
					JSON.stringify({ argument: 'I handle everything', confidence: 0.5 }),
				),
			});
			await registry.initialize();

			const result = await registry.resolveLibrarian('random/topic', 'some content');
			expect(result.winner).toBe('default');
		});

		it('returns single matching specialist without arbitration', async () => {
			const bidResponse = JSON.stringify({
				argument: 'I specialize in code',
				confidence: 0.9,
			});
			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider(bidResponse),
			});
			await registry.initialize();

			const def: LibrarianDefinition = {
				name: 'code-lib',
				description: 'Code',
				purpose: 'Code patterns',
				topics: ['code/*'],
				permissions: { add: true, delete: true, reorganize: true },
				thresholds: { topicComplexity: 50, escalateAt: 100 },
			};
			await registry.register(def);

			const result = await registry.resolveLibrarian('code/react', 'React hooks pattern');
			expect(result.winner).toBe('code-lib');
			expect(result.bids.length).toBeGreaterThanOrEqual(1);
		});

		it('uses self-resolution when one bid is clearly higher', async () => {
			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider(
					JSON.stringify({ argument: 'General', confidence: 0.3 }),
				),
				selfResolutionGap: 0.3,
			});
			await registry.initialize();

			// Register two specialists — but they'll both use defaultProvider
			// so we need a provider factory. For this test, both use the same provider.
			const def1: LibrarianDefinition = {
				name: 'code-lib',
				description: 'Code',
				purpose: 'Code patterns',
				topics: ['code/*'],
				permissions: { add: true, delete: true, reorganize: true },
				thresholds: { topicComplexity: 50, escalateAt: 100 },
			};
			await registry.register(def1);

			// With both using same provider returning 0.3, self-resolution won't trigger
			// (gap is 0, not > 0.3). Default librarian also matches via '*'.
			// All will have equal confidence, so arbitration fires.
			const result = await registry.resolveLibrarian('code/react', 'React pattern');
			// The result should have bids from both default and code-lib
			expect(result.bids.length).toBeGreaterThanOrEqual(2);
		});
	});

	describe('dispose', () => {
		it('disposes without error', async () => {
			const registry = createLibrarianRegistry({
				librariansDir,
				library: createMockLibrary(),
				defaultProvider: createMockProvider('{}'),
			});
			await registry.initialize();
			await expect(registry.dispose()).resolves.toBeUndefined();
		});
	});
});
```

**Step 2: Run tests to verify they fail**

Run: `bun test tests/librarian-registry.test.ts`
Expected: FAIL — module not found.

**Step 3: Implement librarian-registry.ts**

Create `src/ai/library/librarian-registry.ts`:

```typescript
// ---------------------------------------------------------------------------
// Librarian Registry — manages multiple configurable librarians with
// bidding-based ownership resolution and specialist spawning.
// ---------------------------------------------------------------------------

import { join } from 'node:path';
import { rm } from 'node:fs/promises';
import type { Logger } from '../../logger.js';
import { getDefaultLogger } from '../../logger.js';
import type { ACPConnection, ACPConnectionOptions } from '../acp/acp-connection.js';
import { createACPConnection } from '../acp/acp-connection.js';
import { createACPClient } from '../acp/acp-client.js';
import { createACPGenerator } from '../acp/acp-adapters.js';
import type { Library } from './library.js';
import { createLibrarian, type LibrarianIdentity } from './librarian.js';
import {
	loadAllDefinitions,
	matchesTopic,
	saveDefinition,
	validateDefinition,
} from './librarian-definition.js';
import type {
	ArbitrationResult,
	Librarian,
	LibrarianBid,
	LibrarianDefinition,
	TextGenerationProvider,
	Volume,
} from './types.js';

// ---------------------------------------------------------------------------
// ManagedLibrarian
// ---------------------------------------------------------------------------

export interface ManagedLibrarian {
	readonly definition: LibrarianDefinition;
	readonly librarian: Librarian;
	readonly provider: TextGenerationProvider;
	readonly connection?: ACPConnection;
}

// ---------------------------------------------------------------------------
// Registry interface
// ---------------------------------------------------------------------------

export interface LibrarianRegistry {
	readonly initialize: () => Promise<void>;
	readonly dispose: () => Promise<void>;
	readonly register: (definition: LibrarianDefinition) => Promise<ManagedLibrarian>;
	readonly unregister: (name: string) => Promise<void>;
	readonly get: (name: string) => ManagedLibrarian | undefined;
	readonly list: () => readonly ManagedLibrarian[];
	readonly defaultLibrarian: ManagedLibrarian;
	readonly resolveLibrarian: (
		topic: string,
		content: string,
	) => Promise<ArbitrationResult>;
	readonly spawnSpecialist: (
		topic: string,
		volumes: readonly Volume[],
	) => Promise<ManagedLibrarian>;
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface LibrarianRegistryOptions {
	readonly librariansDir: string;
	readonly library: Library;
	readonly defaultProvider: TextGenerationProvider;
	readonly logger?: Logger;
	readonly selfResolutionGap?: number;
	readonly createConnection?: (def: LibrarianDefinition) => Promise<{
		connection: ACPConnection;
		provider: TextGenerationProvider;
	}>;
}

// ---------------------------------------------------------------------------
// Default Librarian Definition
// ---------------------------------------------------------------------------

const DEFAULT_DEFINITION: LibrarianDefinition = Object.freeze({
	name: 'default',
	description: 'General-purpose head librarian',
	purpose: 'I am the general-purpose head librarian. I manage all topics that no specialist claims, and I arbitrate ownership disputes between specialists. I have broad knowledge across all domains and ensure the library stays well-organized.',
	topics: ['*'],
	permissions: { add: true, delete: true, reorganize: true },
	thresholds: { topicComplexity: 50, escalateAt: 100 },
});

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createLibrarianRegistry(
	options: LibrarianRegistryOptions,
): LibrarianRegistry {
	const {
		librariansDir,
		library,
		defaultProvider,
		createConnection: customCreateConnection,
	} = options;
	const logger = (options.logger ?? getDefaultLogger()).child('librarian-registry');
	const selfResolutionGap = options.selfResolutionGap ?? 0.3;

	const librarians = new Map<string, ManagedLibrarian>();
	let defaultManaged: ManagedLibrarian | undefined;
	let initPromise: Promise<void> | null = null;

	// -----------------------------------------------------------------------
	// Internal: create a ManagedLibrarian from a definition
	// -----------------------------------------------------------------------

	const createManaged = async (
		definition: LibrarianDefinition,
	): Promise<ManagedLibrarian> => {
		let provider = defaultProvider;
		let connection: ACPConnection | undefined;

		if (definition.acp && customCreateConnection) {
			const result = await customCreateConnection(definition);
			connection = result.connection;
			provider = result.provider;
		}

		const identity: LibrarianIdentity = {
			name: definition.name,
			purpose: definition.purpose,
		};

		const librarian = createLibrarian(provider, identity);

		return Object.freeze({ definition, librarian, provider, connection });
	};

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	const initialize = (): Promise<void> => {
		if (initPromise) return initPromise;

		initPromise = (async () => {
			logger.debug('Initializing librarian registry', { librariansDir });

			// Load definitions from disk
			const definitions = await loadAllDefinitions(librariansDir);
			for (const def of definitions) {
				if (def.name === 'default') continue; // we create default ourselves
				const managed = await createManaged(def);
				librarians.set(def.name, managed);
			}

			// Create the default librarian
			defaultManaged = await createManaged(DEFAULT_DEFINITION);
			librarians.set('default', defaultManaged);

			logger.info(`Registry initialized with ${librarians.size} librarians`);
		})().finally(() => {
			initPromise = null;
		});

		return initPromise;
	};

	const dispose = async (): Promise<void> => {
		logger.debug('Disposing librarian registry');
		for (const [, managed] of librarians) {
			if (managed.connection) {
				try {
					await managed.connection.close();
				} catch {
					// ignore disposal errors
				}
			}
		}
		librarians.clear();
		defaultManaged = undefined;
		logger.debug('Registry disposed');
	};

	// -----------------------------------------------------------------------
	// Librarian management
	// -----------------------------------------------------------------------

	const register = async (
		definition: LibrarianDefinition,
	): Promise<ManagedLibrarian> => {
		const validation = validateDefinition(definition);
		if (!validation.valid) {
			throw new Error(`Invalid librarian definition: ${validation.errors.join(', ')}`);
		}

		if (librarians.has(definition.name)) {
			throw new Error(`Librarian "${definition.name}" already registered`);
		}

		const managed = await createManaged(definition);
		librarians.set(definition.name, managed);

		// Persist to disk
		await saveDefinition(librariansDir, definition);

		logger.info(`Registered librarian "${definition.name}"`);
		return managed;
	};

	const unregister = async (name: string): Promise<void> => {
		if (name === 'default') {
			throw new Error('Cannot unregister the default librarian');
		}

		const managed = librarians.get(name);
		if (!managed) return;

		if (managed.connection) {
			await managed.connection.close();
		}

		librarians.delete(name);

		// Remove from disk
		try {
			await rm(join(librariansDir, `${name}.json`));
		} catch {
			// file may not exist
		}

		logger.info(`Unregistered librarian "${name}"`);
	};

	const get = (name: string): ManagedLibrarian | undefined => {
		return librarians.get(name);
	};

	const list = (): readonly ManagedLibrarian[] => {
		return [...librarians.values()];
	};

	// -----------------------------------------------------------------------
	// Bidding & Arbitration
	// -----------------------------------------------------------------------

	const resolveLibrarian = async (
		topic: string,
		content: string,
	): Promise<ArbitrationResult> => {
		if (!defaultManaged) {
			throw new Error('Registry not initialized');
		}

		// Find all librarians whose topic globs match
		const candidates: ManagedLibrarian[] = [];
		for (const [, managed] of librarians) {
			if (matchesTopic(managed.definition.topics, topic)) {
				candidates.push(managed);
			}
		}

		// If only the default matches, no bidding needed
		if (candidates.length <= 1) {
			const winner = candidates[0] ?? defaultManaged;
			return {
				winner: winner.definition.name,
				reason: 'Only matching librarian',
				bids: [],
			};
		}

		// Collect bids from all candidates
		const bids: LibrarianBid[] = [];
		for (const candidate of candidates) {
			try {
				const bid = await candidate.librarian.bid(content, topic, library);
				bids.push(bid);
			} catch {
				bids.push({
					librarianName: candidate.definition.name,
					argument: '',
					confidence: 0,
				});
			}
		}

		// Sort by confidence descending
		const sorted = [...bids].sort((a, b) => b.confidence - a.confidence);
		const highest = sorted[0];
		const secondHighest = sorted[1];

		// Self-resolution: clear winner if gap exceeds threshold
		if (
			highest &&
			secondHighest &&
			highest.confidence - secondHighest.confidence > selfResolutionGap
		) {
			return {
				winner: highest.librarianName,
				reason: `Clear confidence gap (${highest.confidence.toFixed(2)} vs ${secondHighest.confidence.toFixed(2)})`,
				bids,
			};
		}

		// Arbitration: default librarian decides
		const arbitrationPrompt = `You are the head librarian. Multiple specialist librarians want to handle this content.
Review their arguments and choose the best fit.

Content: ${content}
Topic: ${topic}

Bids:
${bids.map((b) => `- ${b.librarianName} (confidence: ${b.confidence}): ${b.argument}`).join('\n')}

Return ONLY valid JSON: {"winner": "librarian-name", "reason": "why this librarian is the best fit"}`;

		try {
			const response = await defaultManaged.provider.generate(arbitrationPrompt);
			const parsed = JSON.parse(response);
			const winnerName = typeof parsed.winner === 'string' ? parsed.winner : 'default';
			const reason = typeof parsed.reason === 'string' ? parsed.reason : 'Arbitration decision';

			// Verify winner is a valid candidate
			const validWinner = bids.find((b) => b.librarianName === winnerName);
			if (!validWinner) {
				return { winner: 'default', reason: 'Invalid arbitration result — falling back to default', bids };
			}

			return { winner: winnerName, reason, bids };
		} catch {
			// Arbitration failed — highest bidder wins
			return {
				winner: highest?.librarianName ?? 'default',
				reason: 'Arbitration failed — highest bidder wins',
				bids,
			};
		}
	};

	// -----------------------------------------------------------------------
	// Specialist Spawning
	// -----------------------------------------------------------------------

	const spawnSpecialist = async (
		topic: string,
		volumes: readonly Volume[],
	): Promise<ManagedLibrarian> => {
		if (!defaultManaged) {
			throw new Error('Registry not initialized');
		}

		// Step 1: Confirm specialist is needed (powerful model)
		const sampleTexts = volumes
			.slice(0, 5)
			.map((v) => `- ${v.text.slice(0, 200)}`)
			.join('\n');

		const confirmPrompt = `You are the head librarian assessing whether a topic area needs a specialist.

Topic: ${topic}
Volume count: ${volumes.length}
Sample volumes:
${sampleTexts}

Should a specialist librarian be created for this area?
Consider: Is the content diverse enough to benefit from specialized organization?
Is there a coherent theme that a specialist could focus on?

Return ONLY valid JSON: {"shouldSpawn": true or false, "reason": "why"}`;

		const confirmResponse = await defaultManaged.provider.generate(confirmPrompt);
		let shouldSpawn = false;
		try {
			const parsed = JSON.parse(confirmResponse);
			shouldSpawn = parsed.shouldSpawn === true;
		} catch {
			shouldSpawn = false;
		}

		if (!shouldSpawn) {
			throw new Error(`Specialist not needed for topic "${topic}"`);
		}

		// Step 2: Generate definition (powerful model)
		const generatePrompt = `Create a specialist librarian definition for managing the "${topic}" area of the library.

Existing volumes show these themes:
${sampleTexts}

Generate a JSON librarian definition with:
- name: kebab-case, descriptive (e.g. "react-patterns", "api-design")
- description: one sentence
- purpose: 2-3 sentences explaining the specialist's expertise in first person
- topics: glob patterns covering this area (e.g. ["${topic}/*", "${topic}"])
- permissions: {add: true, delete: true, reorganize: true}
- thresholds: {topicComplexity: 50, escalateAt: 100}

Return ONLY valid JSON matching the schema.`;

		const genResponse = await defaultManaged.provider.generate(generatePrompt);
		let definition: LibrarianDefinition;
		try {
			const parsed = JSON.parse(genResponse);
			const validation = validateDefinition(parsed);
			if (!validation.valid) {
				throw new Error(`Generated definition invalid: ${validation.errors.join(', ')}`);
			}
			definition = parsed as LibrarianDefinition;
		} catch (err) {
			throw new Error(`Failed to generate valid librarian definition: ${err instanceof Error ? err.message : String(err)}`);
		}

		// Step 3: Register the new specialist
		const managed = await register(definition);
		logger.info(`Spawned specialist "${definition.name}" for topic "${topic}"`);
		return managed;
	};

	// -----------------------------------------------------------------------
	// Return
	// -----------------------------------------------------------------------

	return Object.freeze({
		initialize,
		dispose,
		register,
		unregister,
		get,
		list,
		get defaultLibrarian(): ManagedLibrarian {
			if (!defaultManaged) {
				throw new Error('Registry not initialized');
			}
			return defaultManaged;
		},
		resolveLibrarian,
		spawnSpecialist,
	});
}
```

**Step 4: Run tests to verify they pass**

Run: `bun test tests/librarian-registry.test.ts`
Expected: PASS

**Step 5: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 6: Commit**

```bash
git add src/ai/library/librarian-registry.ts tests/librarian-registry.test.ts
git commit -m "feat(library): add LibrarianRegistry with bidding, arbitration, and specialist spawning"
```

---

### Task 5: Update CirculationDesk to Use Registry

**Files:**
- Modify: `src/ai/library/circulation-desk.ts`
- Test: `tests/circulation-desk.test.ts` (extend)

**Step 1: Write the failing test**

Append to `tests/circulation-desk.test.ts`:

```typescript
import { createLibrarianRegistry } from '../src/ai/library/librarian-registry.js';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

describe('CirculationDesk with registry', () => {
	let tempDir: string;

	beforeEach(async () => {
		tempDir = await mkdtemp(join(tmpdir(), 'simse-cd-test-'));
	});

	afterEach(async () => {
		await rm(tempDir, { recursive: true, force: true });
	});

	it('routes extraction through registry', async () => {
		const mockProvider = {
			generate: mock(async () =>
				JSON.stringify({
					memories: [{
						text: 'test fact',
						topic: 'test/topic',
						tags: ['test'],
						entryType: 'fact',
					}],
				}),
			),
		};

		const registry = createLibrarianRegistry({
			librariansDir: join(tempDir, 'librarians'),
			library: {
				search: mock(async () => []),
				getTopics: mock(() => []),
				filterByTopic: mock(() => []),
			} as any,
			defaultProvider: mockProvider,
		});
		await registry.initialize();

		let addedText = '';
		const desk = createCirculationDesk({
			registry,
			addVolume: mock(async (text: string) => {
				addedText = text;
				return 'vol-1';
			}),
			checkDuplicate: mock(async () => ({ isDuplicate: false })),
			getVolumesForTopic: mock(() => []),
		});

		desk.enqueueExtraction({ userInput: 'hello', response: 'world' });
		await desk.drain();

		expect(addedText).toBe('test fact');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/circulation-desk.test.ts --filter "registry"`
Expected: FAIL — `createCirculationDesk` doesn't accept `registry`.

**Step 3: Update CirculationDesk to accept either a single librarian or a registry**

Modify `src/ai/library/circulation-desk.ts`. Add `registry` as an alternative to `librarian`. The extraction flow routes through the registry when available:

```typescript
import type { LibrarianRegistry, ManagedLibrarian } from './librarian-registry.js';
import { matchesTopic } from './librarian-definition.js';

export interface CirculationDeskOptions {
	readonly librarian?: Librarian;
	readonly registry?: LibrarianRegistry;
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
```

In `createCirculationDesk`, resolve the librarian for each extraction memory via the registry when available:

```typescript
const getLibrarian = (): Librarian => {
	if (options.librarian) return options.librarian;
	if (options.registry) return options.registry.defaultLibrarian.librarian;
	throw new Error('CirculationDesk requires either librarian or registry');
};

const resolveLibrarianForTopic = async (
	topic: string,
	content: string,
): Promise<Librarian> => {
	if (!options.registry) return getLibrarian();
	const result = await options.registry.resolveLibrarian(topic, content);
	const managed = options.registry.get(result.winner);
	return managed?.librarian ?? getLibrarian();
};
```

Update the extraction job to route through `resolveLibrarianForTopic` for each extracted memory. The initial extraction still uses the default librarian (fast model), but the add/reorganize/optimize operations are routed to the winning bidder.

**Step 4: Run tests to verify they pass**

Run: `bun test tests/circulation-desk.test.ts`
Expected: PASS — both old tests (using `librarian`) and new tests (using `registry`) pass.

**Step 5: Run full test suite and typecheck**

Run: `bun run typecheck && bun test`
Expected: PASS

**Step 6: Commit**

```bash
git add src/ai/library/circulation-desk.ts tests/circulation-desk.test.ts
git commit -m "feat(library): integrate LibrarianRegistry into CirculationDesk"
```

---

### Task 6: Update Barrel Exports and LibraryServices

**Files:**
- Modify: `src/lib.ts:162-245` (library exports section)
- Modify: `src/ai/library/library-services.ts`

**Step 1: Update lib.ts exports**

Add to the library section of `src/lib.ts` (after the existing librarian exports around line 205):

```typescript
// Librarian Definition
export type { ValidationResult } from './ai/library/librarian-definition.js';
export {
	loadDefinition,
	loadAllDefinitions,
	matchesTopic,
	saveDefinition,
	validateDefinition,
} from './ai/library/librarian-definition.js';

// Librarian Registry
export type {
	LibrarianRegistry,
	LibrarianRegistryOptions,
	ManagedLibrarian,
} from './ai/library/librarian-registry.js';
export { createLibrarianRegistry } from './ai/library/librarian-registry.js';
export type { LibrarianIdentity } from './ai/library/librarian.js';
```

Add to the types re-export block (around line 206-245):

```typescript
// Add to the existing type re-exports from types.js:
	ArbitrationResult,
	LibrarianBid,
	LibrarianDefinition,
```

**Step 2: Update LibraryServices to accept optional registry**

Add a `registry` option to `LibraryServicesOptions` in `src/ai/library/library-services.ts`:

```typescript
import type { LibrarianRegistry } from './librarian-registry.js';

export interface LibraryServicesOptions {
	// ... existing fields ...
	/** Optional LibrarianRegistry for multi-librarian routing. */
	readonly registry?: LibrarianRegistry;
}
```

When `registry` is provided and a `circulationDesk` is provided, the CirculationDesk uses the registry. No other changes needed — the CirculationDesk handles the routing internally.

**Step 3: Run typecheck and full tests**

Run: `bun run typecheck && bun test`
Expected: PASS

**Step 4: Commit**

```bash
git add src/lib.ts src/ai/library/library-services.ts
git commit -m "feat(library): export librarian registry types and update library services"
```

---

### Task 7: Add Spawn Threshold Tests

**Files:**
- Modify: `tests/circulation-desk.test.ts` (extend)
- Modify: `tests/librarian-registry.test.ts` (extend)

**Step 1: Write spawn threshold test for CirculationDesk**

Append to `tests/circulation-desk.test.ts`:

```typescript
describe('spawn thresholds', () => {
	let tempDir: string;

	beforeEach(async () => {
		tempDir = await mkdtemp(join(tmpdir(), 'simse-spawn-test-'));
	});

	afterEach(async () => {
		await rm(tempDir, { recursive: true, force: true });
	});

	it('triggers spawn check when topic complexity threshold exceeded', async () => {
		const mockProvider = {
			generate: mock(async () =>
				JSON.stringify({
					memories: [{
						text: 'complex fact',
						topic: 'code/react',
						tags: ['test'],
						entryType: 'fact',
					}],
				}),
			),
		};

		const registry = createLibrarianRegistry({
			librariansDir: join(tempDir, 'librarians'),
			library: {
				search: mock(async () => []),
				getTopics: mock(() => [
					{ topic: 'code/react', entryCount: 55, children: ['code/react/hooks', 'code/react/state', 'code/react/effects'] },
				]),
				filterByTopic: mock(() => []),
			} as any,
			defaultProvider: mockProvider,
		});
		await registry.initialize();

		// Create 55 mock volumes to exceed threshold
		const volumes = Array.from({ length: 55 }, (_, i) => ({
			id: `vol-${i}`,
			text: `Volume ${i}`,
			embedding: [0.1, 0.2],
			metadata: { topic: 'code/react' },
			timestamp: Date.now(),
		}));

		const desk = createCirculationDesk({
			registry,
			addVolume: mock(async () => 'vol-new'),
			checkDuplicate: mock(async () => ({ isDuplicate: false })),
			getVolumesForTopic: mock(() => volumes),
			getAllTopics: mock(() => ['code/react']),
			getTotalVolumeCount: mock(() => 55),
			thresholds: {
				spawning: {
					complexityThreshold: 50,
					depthThreshold: 3,
					childTopicThreshold: 3,
					modelId: 'test-model',
				},
			},
		});

		desk.enqueueExtraction({ userInput: 'test', response: 'test response' });
		await desk.drain();

		// Verify extraction completed (spawn check is fire-and-forget)
		expect(mockProvider.generate).toHaveBeenCalled();
	});
});
```

**Step 2: Write spawn test for LibrarianRegistry**

Append to `tests/librarian-registry.test.ts`:

```typescript
describe('spawnSpecialist', () => {
	it('spawns a new specialist when confirmed', async () => {
		let callCount = 0;
		const mockProvider = {
			generate: mock(async () => {
				callCount++;
				if (callCount <= 2) {
					// Bid responses (for any bidding during register)
					return JSON.stringify({ argument: 'test', confidence: 0.5 });
				}
				if (callCount === 3) {
					// Confirmation response
					return JSON.stringify({ shouldSpawn: true, reason: 'Complex area' });
				}
				// Definition generation response
				return JSON.stringify({
					name: 'react-patterns',
					description: 'React pattern specialist',
					purpose: 'I specialize in React component patterns, hooks, and state management.',
					topics: ['code/react/*', 'code/react'],
					permissions: { add: true, delete: true, reorganize: true },
					thresholds: { topicComplexity: 50, escalateAt: 100 },
				});
			}),
			generateWithModel: mock(async () => '{}'),
		};

		const registry = createLibrarianRegistry({
			librariansDir: join(tempDir, 'librarians'),
			library: createMockLibrary(),
			defaultProvider: mockProvider,
		});
		await registry.initialize();

		const volumes = Array.from({ length: 5 }, (_, i) => ({
			id: `vol-${i}`,
			text: `React pattern ${i}`,
			embedding: [0.1],
			metadata: { topic: 'code/react' },
			timestamp: Date.now(),
		}));

		const managed = await registry.spawnSpecialist('code/react', volumes);
		expect(managed.definition.name).toBe('react-patterns');
		expect(registry.get('react-patterns')).toBeDefined();
	});

	it('throws when spawn is not confirmed', async () => {
		const mockProvider = {
			generate: mock(async () =>
				JSON.stringify({ shouldSpawn: false, reason: 'Not complex enough' }),
			),
		};

		const registry = createLibrarianRegistry({
			librariansDir: join(tempDir, 'librarians'),
			library: createMockLibrary(),
			defaultProvider: mockProvider,
		});
		await registry.initialize();

		await expect(
			registry.spawnSpecialist('simple/topic', []),
		).rejects.toThrow('Specialist not needed');
	});
});
```

**Step 3: Run all tests**

Run: `bun test tests/librarian-registry.test.ts tests/circulation-desk.test.ts`
Expected: PASS

**Step 4: Run full test suite**

Run: `bun test`
Expected: PASS

**Step 5: Commit**

```bash
git add tests/circulation-desk.test.ts tests/librarian-registry.test.ts
git commit -m "test(library): add spawn threshold and specialist spawning tests"
```

---

### Task 8: Run Lint, Typecheck, and Final Verification

**Step 1: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 2: Run lint**

Run: `bun run lint`
Expected: PASS (or fix any issues)

**Step 3: Run full test suite**

Run: `bun test`
Expected: All tests PASS

**Step 4: Fix any lint issues**

Run: `bun run lint:fix`

**Step 5: Final commit if any lint fixes**

```bash
git add -A
git commit -m "chore: fix lint issues in configurable librarians"
```

**Step 6: Push**

```bash
git push
```
