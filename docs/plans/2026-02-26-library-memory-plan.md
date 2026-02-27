# Library Memory System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the memory system into a library model with volumes, shelves, catalog, librarians, circulation desk, and compendia.

**Architecture:** Rename all memory modules to library terminology, add TopicCatalog for LLM-driven classification, Librarian for extraction/summarization, CirculationDesk for background queue processing, and Shelf for agent-scoped memory. Clean break — no backwards compat.

**Tech Stack:** TypeScript, Bun test runner, Biome linter

---

## Phase 1: File and Type Renames (Mechanical, No New Features)

### Task 1 — Rename `src/ai/memory/` directory to `src/ai/library/`

**Files:**
- Create: `src/ai/library/` (directory)
- Move: all 20 files from `src/ai/memory/` into `src/ai/library/`
- Rename within `src/ai/library/`:
  - `memory.ts` -> `library.ts`
  - `vector-store.ts` -> `stacks.ts`
  - `vector-persistence.ts` -> `stacks-persistence.ts`
  - `compression.ts` -> `preservation.ts`
  - `indexing.ts` -> `cataloging.ts`
  - `learning.ts` -> `patron-learning.ts`
  - `middleware.ts` -> `library-services.ts`
  - `vector-search.ts` -> `stacks-search.ts`
  - `vector-recommend.ts` -> `stacks-recommend.ts`
  - `vector-serialize.ts` -> `stacks-serialize.ts`
- Unchanged names within `src/ai/library/`:
  - `types.ts`, `cosine.ts`, `storage.ts`, `deduplication.ts`, `recommendation.ts`, `text-search.ts`, `inverted-index.ts`, `prompt-injection.ts`, `query-dsl.ts`, `text-cache.ts`
- No test file yet — mechanical move only

**Steps:**

1. Create the target directory:
   ```bash
   mkdir -p src/ai/library
   ```

2. Copy all files from `src/ai/memory/` to `src/ai/library/`, renaming the ones that change:
   ```bash
   # Files that get renamed
   cp src/ai/memory/memory.ts src/ai/library/library.ts
   cp src/ai/memory/vector-store.ts src/ai/library/stacks.ts
   cp src/ai/memory/vector-persistence.ts src/ai/library/stacks-persistence.ts
   cp src/ai/memory/compression.ts src/ai/library/preservation.ts
   cp src/ai/memory/indexing.ts src/ai/library/cataloging.ts
   cp src/ai/memory/learning.ts src/ai/library/patron-learning.ts
   cp src/ai/memory/middleware.ts src/ai/library/library-services.ts
   cp src/ai/memory/vector-search.ts src/ai/library/stacks-search.ts
   cp src/ai/memory/vector-recommend.ts src/ai/library/stacks-recommend.ts
   cp src/ai/memory/vector-serialize.ts src/ai/library/stacks-serialize.ts
   # Files that keep their name
   cp src/ai/memory/types.ts src/ai/library/types.ts
   cp src/ai/memory/cosine.ts src/ai/library/cosine.ts
   cp src/ai/memory/storage.ts src/ai/library/storage.ts
   cp src/ai/memory/deduplication.ts src/ai/library/deduplication.ts
   cp src/ai/memory/recommendation.ts src/ai/library/recommendation.ts
   cp src/ai/memory/text-search.ts src/ai/library/text-search.ts
   cp src/ai/memory/inverted-index.ts src/ai/library/inverted-index.ts
   cp src/ai/memory/prompt-injection.ts src/ai/library/prompt-injection.ts
   cp src/ai/memory/query-dsl.ts src/ai/library/query-dsl.ts
   cp src/ai/memory/text-cache.ts src/ai/library/text-cache.ts
   ```

3. Update all internal imports within `src/ai/library/` files. Every file that imports from a renamed sibling must update its import path. Key changes:
   - `'./compression.js'` -> `'./preservation.js'`
   - `'./indexing.js'` -> `'./cataloging.js'`
   - `'./learning.js'` -> `'./patron-learning.js'`
   - `'./vector-store.js'` -> `'./stacks.js'`
   - `'./vector-persistence.js'` -> `'./stacks-persistence.js'`
   - `'./vector-search.js'` -> `'./stacks-search.js'`
   - `'./vector-recommend.js'` -> `'./stacks-recommend.js'`
   - `'./vector-serialize.js'` -> `'./stacks-serialize.js'`
   - `'./memory.js'` -> `'./library.js'`
   - `'./middleware.js'` -> `'./library-services.js'`

   Files requiring import path updates (non-exhaustive — check every file):
   - `src/ai/library/library.ts`: imports from `./stacks.js`, `./query-dsl.js`, `./storage.js`, `./types.js`
   - `src/ai/library/stacks.ts`: imports from `./deduplication.js`, `./cataloging.js`, `./inverted-index.js`, `./patron-learning.js`, `./recommendation.js`, `./storage.js`, `./types.js`, `./stacks-recommend.js`, `./stacks-search.js`, `./stacks-serialize.js`
   - `src/ai/library/patron-learning.ts`: imports from `./preservation.js`, `./cosine.js`, `./types.js`, `./stacks-persistence.js`
   - `src/ai/library/stacks-search.ts`: imports from `./cataloging.js`, `./inverted-index.js`, `./recommendation.js`, `./text-search.js`, `./types.js`
   - `src/ai/library/stacks-recommend.ts`: imports from `./cataloging.js`, `./patron-learning.js`, `./recommendation.js`, `./types.js`
   - `src/ai/library/stacks-serialize.ts`: imports from `./preservation.js`, `./types.js`, `./stacks-persistence.js`
   - `src/ai/library/library-services.ts`: imports from `./library.js`, `./prompt-injection.js`
   - `src/ai/library/deduplication.ts`: imports from `./cosine.js`, `./types.js`
   - `src/ai/library/cataloging.ts`: imports from `./types.js`
   - `src/ai/library/prompt-injection.ts`: imports from `./types.js`
   - `src/ai/library/query-dsl.ts`: imports from `./types.js`
   - `src/ai/library/text-search.ts`: imports from `./types.js`

4. Delete the old `src/ai/memory/` directory:
   ```bash
   rm -rf src/ai/memory
   ```

5. Verify typecheck passes:
   ```bash
   bun run typecheck
   ```
   (This will fail because external consumers still import from `../memory/` — that's expected and fixed in tasks 7-8.)

6. Commit:
   ```
   refactor: rename src/ai/memory/ to src/ai/library/ with file renames
   ```

---

### Task 2 — Rename types in `src/ai/library/types.ts`

**Files:**
- Modify: `src/ai/library/types.ts`
- Test: `bun test tests/library-types.test.ts`

**Steps:**

1. Write a failing test that imports the new type names:
   ```typescript
   // tests/library-types.test.ts
   import { describe, expect, it } from 'bun:test';
   import type {
   	Volume,
   	Lookup,
   	TextLookup,
   	AdvancedLookup,
   	DuplicateVolumes,
   	CompendiumOptions,
   	CompendiumResult,
   	PatronProfile,
   	LibraryConfig,
   } from '../src/ai/library/types.js';

   describe('Library types', () => {
   	it('Volume has the correct shape', () => {
   		const vol: Volume = {
   			id: 'v1',
   			text: 'hello',
   			embedding: [0.1, 0.2],
   			metadata: { topic: 'test' },
   			timestamp: Date.now(),
   		};
   		expect(vol.id).toBe('v1');
   	});

   	it('Lookup has volume + score', () => {
   		const lookup: Lookup = {
   			volume: {
   				id: 'v1',
   				text: 'hello',
   				embedding: [0.1],
   				metadata: {},
   				timestamp: Date.now(),
   			},
   			score: 0.95,
   		};
   		expect(lookup.score).toBe(0.95);
   	});

   	it('CompendiumOptions has required fields', () => {
   		const opts: CompendiumOptions = {
   			ids: ['a', 'b'],
   		};
   		expect(opts.ids.length).toBe(2);
   	});

   	it('CompendiumResult has compendiumId', () => {
   		const result: CompendiumResult = {
   			compendiumId: 'c1',
   			text: 'summary',
   			sourceIds: ['a', 'b'],
   			deletedOriginals: false,
   		};
   		expect(result.compendiumId).toBe('c1');
   	});

   	it('PatronProfile has adaptedWeights', () => {
   		const profile: PatronProfile = {
   			queryHistory: [],
   			adaptedWeights: { vector: 0.6, recency: 0.2, frequency: 0.2 },
   			interestEmbedding: undefined,
   			totalQueries: 0,
   			lastUpdated: 0,
   		};
   		expect(profile.totalQueries).toBe(0);
   	});

   	it('LibraryConfig replaces MemoryConfig', () => {
   		const config: LibraryConfig = {
   			enabled: true,
   			similarityThreshold: 0.7,
   			maxResults: 10,
   		};
   		expect(config.enabled).toBe(true);
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/library-types.test.ts`

3. Rename types in `src/ai/library/types.ts`. The following renames apply throughout the file:
   - `VectorEntry` -> `Volume`
   - `SearchResult` -> `Lookup` (with `entry` field renamed to `volume`)
   - `TextSearchResult` -> `TextLookup` (with `entry` field renamed to `volume`)
   - `AdvancedSearchResult` -> `AdvancedLookup` (with `entry` field renamed to `volume`)
   - `DuplicateCheckResult` -> `DuplicateCheckResult` (keep — generic enough; but `existingEntry` -> `existingVolume`)
   - `DuplicateGroup` -> `DuplicateVolumes` (with `representative` and `duplicates` using `Volume`)
   - `SummarizeOptions` -> `CompendiumOptions`
   - `SummarizeResult` -> `CompendiumResult` (with `summaryId` -> `compendiumId`, `summaryText` -> `text`)
   - `LearningProfile` -> `PatronProfile`
   - `MemoryConfig` -> `LibraryConfig`
   - `RecommendationResult` -> `Recommendation` (with `entry` field renamed to `volume`)
   - `TopicInfo` -> `TopicInfo` (keep name — works with library model)

   Also add backward-compat type aliases at the bottom of the file (temporarily, to unblock migration; will remove later):
   ```typescript
   /** @deprecated Use Volume */
   export type VectorEntry = Volume;
   /** @deprecated Use Lookup */
   export type SearchResult = Lookup;
   // ... etc for all renamed types
   ```
   These aliases allow incremental migration of internal consumers without breaking the build after each task.

4. Run test to verify pass: `bun test tests/library-types.test.ts`

5. Commit:
   ```
   refactor: rename memory types to library terminology (Volume, Lookup, etc.)
   ```

---

### Task 3 — Rename factory and interface: `createMemoryManager` -> `createLibrary`

**Files:**
- Modify: `src/ai/library/library.ts`
- Test: `bun test tests/library.test.ts`

**Steps:**

1. Write a failing test importing the new factory name:
   ```typescript
   // tests/library.test.ts
   import { beforeEach, describe, expect, it, mock } from 'bun:test';
   import { createLibrary, type Library } from '../src/ai/library/library.js';
   import type {
   	EmbeddingProvider,
   	LibraryConfig,
   } from '../src/ai/library/types.js';
   import { createMemoryStorage, createSilentLogger } from './utils/mocks.js';

   function createMockEmbedder(dim = 3): EmbeddingProvider {
   	let callCount = 0;
   	return {
   		embed: mock(async (input: string | readonly string[]) => {
   			const texts = typeof input === 'string' ? [input] : input;
   			callCount++;
   			return {
   				embeddings: texts.map((_, i) => {
   					const base = (callCount * 10 + i) * 0.1;
   					return Array.from({ length: dim }, (__, j) =>
   						Math.sin(base + j * 0.7),
   					);
   				}),
   			};
   		}),
   	};
   }

   const defaultConfig: LibraryConfig = {
   	enabled: true,
   	embeddingAgent: 'test-embedder',
   	similarityThreshold: 0,
   	maxResults: 10,
   };

   describe('Library (was MemoryManager)', () => {
   	let library: Library;

   	beforeEach(async () => {
   		library = createLibrary(createMockEmbedder(), defaultConfig, {
   			storage: createMemoryStorage(),
   			logger: createSilentLogger(),
   			stacksOptions: {
   				autoSave: true,
   				flushIntervalMs: 0,
   				learning: { enabled: false },
   			},
   		});
   		await library.initialize();
   	});

   	it('has the Library interface shape', () => {
   		expect(typeof library.add).toBe('function');
   		expect(typeof library.search).toBe('function');
   		expect(typeof library.compendium).toBe('function');
   		expect(typeof library.patronProfile).not.toBe('undefined');
   	});

   	it('add returns a volume id', async () => {
   		const id = await library.add('test text', { topic: 'testing' });
   		expect(typeof id).toBe('string');
   		expect(id.length).toBeGreaterThan(0);
   	});

   	it('search returns Lookup[] with volume field', async () => {
   		await library.add('important fact about databases', { topic: 'db' });
   		const results = await library.search('databases');
   		expect(results.length).toBeGreaterThan(0);
   		expect(results[0].volume).toBeDefined();
   		expect(results[0].volume.text).toContain('databases');
   		expect(typeof results[0].score).toBe('number');
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/library.test.ts`

3. Rename in `src/ai/library/library.ts`:
   - `MemoryManagerOptions` -> `LibraryOptions`
   - `vectorStoreOptions` field -> `stacksOptions`
   - `MemoryManager` interface -> `Library`
   - `createMemoryManager` function -> `createLibrary`
   - `learningProfile` property -> `patronProfile`
   - `summarize` method -> `compendium` (signature: `CompendiumOptions` -> `CompendiumResult`)
   - `SummarizeOptions`/`SummarizeResult` imports -> `CompendiumOptions`/`CompendiumResult`
   - `SearchResult` -> `Lookup`
   - `TextSearchResult` -> `TextLookup`
   - `AdvancedSearchResult` -> `AdvancedLookup`
   - `DuplicateGroup` -> `DuplicateVolumes`
   - `RecommendationResult` -> `Recommendation`
   - `LearningProfile` -> `PatronProfile`
   - `MemoryConfig` -> `LibraryConfig`
   - `VectorEntry` -> `Volume`
   - All `createMemoryError` calls -> `createLibraryError` (from `../../errors/index.js` — updated in Task 5)
   - Update logger child name: `'memory'` -> `'library'`
   - Update log messages: `'memory manager'` -> `'library'`
   - Update event names: `'memory.add'` -> `'library.shelve'`, `'memory.search'` -> `'library.search'`, `'memory.delete'` -> `'library.withdraw'`
   - `createVectorStore` import -> `createStacks` (renamed in Task 1)
   - Return type field renames: `entry` -> `volume` in Lookup results
   - `setTextGenerator` stays (still needed for compendium)
   - `recordFeedback` stays (PatronProfile concept)

   Also rename `VectorStoreOptions` in `stacks.ts`:
   - `VectorStoreOptions` -> `StacksOptions`
   - `VectorStore` interface -> `Stacks`
   - `createVectorStore` -> `createStacks`
   - All internal references to `SearchResult` -> `Lookup`, etc.

   And in `stacks.ts`, rename field `entry` -> `volume` in returned result objects from search functions. This cascades into `stacks-search.ts`, `stacks-recommend.ts`.

4. Run test to verify pass: `bun test tests/library.test.ts`

5. Commit:
   ```
   refactor: rename createMemoryManager to createLibrary with Library interface
   ```

---

### Task 4 — Rename VectorStore to Stacks and update all stacks-* files

**Files:**
- Modify: `src/ai/library/stacks.ts`, `src/ai/library/stacks-persistence.ts`, `src/ai/library/stacks-search.ts`, `src/ai/library/stacks-recommend.ts`, `src/ai/library/stacks-serialize.ts`
- Test: `bun test tests/stacks.test.ts`

**Steps:**

1. Write a failing test:
   ```typescript
   // tests/stacks.test.ts
   import { beforeEach, describe, expect, it } from 'bun:test';
   import { createStacks, type Stacks } from '../src/ai/library/stacks.js';
   import { createMemoryStorage, createSilentLogger } from './utils/mocks.js';

   describe('Stacks (was VectorStore)', () => {
   	let stacks: Stacks;

   	beforeEach(async () => {
   		stacks = createStacks({
   			storage: createMemoryStorage(),
   			logger: createSilentLogger(),
   			autoSave: true,
   			flushIntervalMs: 0,
   			learning: { enabled: false },
   		});
   		await stacks.load();
   	});

   	it('add and search return Lookup with volume field', async () => {
   		const id = await stacks.add('hello world', [0.1, 0.2, 0.3]);
   		expect(typeof id).toBe('string');
   		const results = stacks.search([0.1, 0.2, 0.3], 10, 0);
   		expect(results.length).toBe(1);
   		expect(results[0].volume).toBeDefined();
   		expect(results[0].volume.id).toBe(id);
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/stacks.test.ts`

3. In `src/ai/library/stacks.ts`:
   - Rename interface `VectorStoreOptions` -> `StacksOptions`
   - Rename interface `VectorStore` -> `Stacks`
   - Rename function `createVectorStore` -> `createStacks`
   - All `SearchResult` -> `Lookup`, `TextSearchResult` -> `TextLookup`, `AdvancedSearchResult` -> `AdvancedLookup`, `DuplicateGroup` -> `DuplicateVolumes`, `RecommendationResult` -> `Recommendation`, `VectorEntry` -> `Volume`, `LearningProfile` -> `PatronProfile`
   - `createMemoryError` -> `createLibraryError` (deferred to Task 5 — use alias for now)
   - `createVectorStoreCorruptionError` -> `createStacksCorruptionError` (deferred to Task 5)
   - Logger child name: `'vector-store'` -> `'stacks'`
   - Update error codes: `VECTOR_STORE_NOT_LOADED` -> `STACKS_NOT_LOADED`, `VECTOR_STORE_EMPTY_TEXT` -> `STACKS_EMPTY_TEXT`, `VECTOR_STORE_EMPTY_EMBEDDING` -> `STACKS_EMPTY_EMBEDDING`, `VECTOR_STORE_DUPLICATE` -> `STACKS_DUPLICATE`
   - `LearningEngine` import from `./patron-learning.js`

   In `src/ai/library/stacks-persistence.ts`:
   - `IndexEntry` -> `StacksIndexEntry`
   - `IndexFile` -> `StacksIndexFile`
   - `isValidIndexEntry` -> `isValidStacksIndexEntry`
   - `isValidIndexFile` -> `isValidStacksIndexFile`
   - `LearningState` -> `PatronLearningState`
   - `isValidLearningState` -> `isValidPatronLearningState`
   - Other types stay (FeedbackEntry, SerializedQueryRecord, etc. — internal persistence only)

   In `src/ai/library/stacks-search.ts`:
   - `VectorSearchConfig` -> `StacksSearchConfig`
   - `vectorSearch` -> `stacksSearch`
   - `advancedVectorSearch` -> `advancedStacksSearch`
   - `textSearchEntries` -> `textSearchVolumes`
   - `filterEntriesByMetadata` -> `filterVolumesByMetadata`
   - `filterEntriesByDateRange` -> `filterVolumesByDateRange`
   - All `VectorEntry` -> `Volume` in parameters/returns

   In `src/ai/library/stacks-recommend.ts`:
   - `computeRecommendations` -> `computeRecommendations` (keep — clear enough)
   - `VectorEntry` -> `Volume`

   In `src/ai/library/stacks-serialize.ts`:
   - `serializeToStorage` / `deserializeFromStorage` — keep names (still descriptive)
   - `VectorEntry` -> `Volume`
   - Imports from `stacks-persistence.js` instead of `vector-persistence.js`

4. Run test to verify pass: `bun test tests/stacks.test.ts`

5. Commit:
   ```
   refactor: rename VectorStore to Stacks across all stacks-* files
   ```

---

### Task 5 — Rename errors: `src/errors/memory.ts` -> `src/errors/library.ts`

**Files:**
- Create: `src/errors/library.ts`
- Delete: `src/errors/memory.ts`
- Modify: `src/errors/index.ts`
- Modify: `src/ai/library/library.ts`, `src/ai/library/stacks.ts` (update error imports)
- Test: `bun test tests/library-errors.test.ts`

**Steps:**

1. Write a failing test:
   ```typescript
   // tests/library-errors.test.ts
   import { describe, expect, it } from 'bun:test';
   import {
   	createLibraryError,
   	createStacksError,
   	createStacksCorruptionError,
   	createStacksIOError,
   	createEmbeddingError,
   	isLibraryError,
   	isStacksError,
   	isEmbeddingError,
   	isStacksCorruptionError,
   	isStacksIOError,
   } from '../src/errors/index.js';

   describe('Library errors', () => {
   	it('createLibraryError creates a LIBRARY_ERROR', () => {
   		const err = createLibraryError('test');
   		expect(err.name).toBe('LibraryError');
   		expect(err.code).toBe('LIBRARY_ERROR');
   		expect(isLibraryError(err)).toBe(true);
   	});

   	it('createStacksCorruptionError creates a STACKS_CORRUPT', () => {
   		const err = createStacksCorruptionError('path/to/store');
   		expect(err.code).toBe('STACKS_CORRUPT');
   		expect(isStacksCorruptionError(err)).toBe(true);
   		expect(isLibraryError(err)).toBe(true);
   	});

   	it('createEmbeddingError still works', () => {
   		const err = createEmbeddingError('embed failed');
   		expect(err.code).toBe('EMBEDDING_ERROR');
   		expect(isEmbeddingError(err)).toBe(true);
   		expect(isLibraryError(err)).toBe(true);
   	});

   	it('isStacksError matches STACKS_ codes', () => {
   		const err = createLibraryError('test', { code: 'STACKS_NOT_LOADED' });
   		expect(isStacksError(err)).toBe(true);
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/library-errors.test.ts`

3. Create `src/errors/library.ts`:
   ```typescript
   // ---------------------------------------------------------------------------
   // Library / Stacks / Embedding Errors
   // ---------------------------------------------------------------------------

   import type { SimseError } from './base.js';
   import { createSimseError, isSimseError } from './base.js';

   export const createLibraryError = (
   	message: string,
   	options: {
   		name?: string;
   		code?: string;
   		cause?: unknown;
   		metadata?: Record<string, unknown>;
   	} = {},
   ): SimseError =>
   	createSimseError(message, {
   		name: options.name ?? 'LibraryError',
   		code: options.code ?? 'LIBRARY_ERROR',
   		statusCode: 500,
   		cause: options.cause,
   		metadata: options.metadata,
   	});

   export const createEmbeddingError = (
   	message: string,
   	options: { cause?: unknown; model?: string } = {},
   ): SimseError =>
   	createLibraryError(message, {
   		name: 'EmbeddingError',
   		code: 'EMBEDDING_ERROR',
   		cause: options.cause,
   		metadata: options.model ? { model: options.model } : {},
   	});

   export const createStacksCorruptionError = (
   	storePath: string,
   	options: { cause?: unknown } = {},
   ): SimseError & { readonly storePath: string } => {
   	const err = createLibraryError(
   		`Stacks file is corrupted: ${storePath}`,
   		{
   			name: 'StacksCorruptionError',
   			code: 'STACKS_CORRUPT',
   			cause: options.cause,
   			metadata: { storePath },
   		},
   	) as SimseError & { readonly storePath: string };

   	Object.defineProperty(err, 'storePath', {
   		value: storePath,
   		writable: false,
   		enumerable: true,
   	});

   	return err;
   };

   export const createStacksIOError = (
   	storePath: string,
   	operation: 'read' | 'write',
   	options: { cause?: unknown } = {},
   ): SimseError & { readonly storePath: string } => {
   	const err = createLibraryError(
   		`Failed to ${operation} stacks: ${storePath}`,
   		{
   			name: 'StacksIOError',
   			code: 'STACKS_IO',
   			cause: options.cause,
   			metadata: { storePath, operation },
   		},
   	) as SimseError & { readonly storePath: string };

   	Object.defineProperty(err, 'storePath', {
   		value: storePath,
   		writable: false,
   		enumerable: true,
   	});

   	return err;
   };

   // ---------------------------------------------------------------------------
   // Convenience alias (some internal code uses createStacksError)
   // ---------------------------------------------------------------------------

   export const createStacksError = (
   	message: string,
   	options: {
   		code?: string;
   		cause?: unknown;
   		metadata?: Record<string, unknown>;
   	} = {},
   ): SimseError =>
   	createLibraryError(message, {
   		name: 'StacksError',
   		code: options.code ?? 'STACKS_ERROR',
   		cause: options.cause,
   		metadata: options.metadata,
   	});

   // ---------------------------------------------------------------------------
   // Type Guards
   // ---------------------------------------------------------------------------

   export const isLibraryError = (value: unknown): value is SimseError =>
   	isSimseError(value) &&
   	(value.code.startsWith('LIBRARY_') ||
   		value.code.startsWith('EMBEDDING_') ||
   		value.code.startsWith('STACKS_'));

   export const isStacksError = (value: unknown): value is SimseError =>
   	isSimseError(value) && value.code.startsWith('STACKS_');

   export const isEmbeddingError = (value: unknown): value is SimseError =>
   	isSimseError(value) && value.code === 'EMBEDDING_ERROR';

   export const isStacksCorruptionError = (
   	value: unknown,
   ): value is SimseError & { readonly storePath: string } =>
   	isSimseError(value) && value.code === 'STACKS_CORRUPT';

   export const isStacksIOError = (
   	value: unknown,
   ): value is SimseError & { readonly storePath: string } =>
   	isSimseError(value) && value.code === 'STACKS_IO';

   // ---------------------------------------------------------------------------
   // Backward-compat aliases (to be removed after full migration)
   // ---------------------------------------------------------------------------

   /** @deprecated Use createLibraryError */
   export const createMemoryError = createLibraryError;
   /** @deprecated Use isLibraryError */
   export const isMemoryError = isLibraryError;
   /** @deprecated Use createStacksCorruptionError */
   export const createVectorStoreCorruptionError = createStacksCorruptionError;
   /** @deprecated Use isStacksCorruptionError */
   export const isVectorStoreCorruptionError = isStacksCorruptionError;
   /** @deprecated Use createStacksIOError */
   export const createVectorStoreIOError = createStacksIOError;
   /** @deprecated Use isStacksIOError */
   export const isVectorStoreIOError = isStacksIOError;
   ```

4. Update `src/errors/index.ts`: replace the `from './memory.js'` block with `from './library.js'` exporting both new names and backward-compat aliases.

5. Delete `src/errors/memory.ts`.

6. Update all library files to import from `../../errors/index.js` using the new names (where they currently use `createMemoryError`, switch to `createLibraryError`; where they use `createVectorStoreCorruptionError`, switch to `createStacksCorruptionError`).

7. Run test to verify pass: `bun test tests/library-errors.test.ts`

8. Commit:
   ```
   refactor: rename memory errors to library/stacks terminology
   ```

---

### Task 6 — Update `src/lib.ts` barrel exports

**Files:**
- Modify: `src/lib.ts`
- Test: `bun run typecheck`

**Steps:**

1. Replace all `./ai/memory/` import paths with `./ai/library/` equivalents:
   - `'./ai/memory/compression.js'` -> `'./ai/library/preservation.js'`
   - `'./ai/memory/cosine.js'` -> `'./ai/library/cosine.js'`
   - `'./ai/memory/indexing.js'` -> `'./ai/library/cataloging.js'`
   - `'./ai/memory/inverted-index.js'` -> `'./ai/library/inverted-index.js'`
   - `'./ai/memory/learning.js'` -> `'./ai/library/patron-learning.js'`
   - `'./ai/memory/memory.js'` -> `'./ai/library/library.js'`
   - `'./ai/memory/middleware.js'` -> `'./ai/library/library-services.js'`
   - `'./ai/memory/prompt-injection.js'` -> `'./ai/library/prompt-injection.js'`
   - `'./ai/memory/query-dsl.js'` -> `'./ai/library/query-dsl.js'`
   - `'./ai/memory/recommendation.js'` -> `'./ai/library/recommendation.js'`
   - `'./ai/memory/storage.js'` -> `'./ai/library/storage.js'`
   - `'./ai/memory/text-cache.js'` -> `'./ai/library/text-cache.js'`
   - `'./ai/memory/types.js'` -> `'./ai/library/types.js'`
   - `'./ai/memory/vector-recommend.js'` -> `'./ai/library/stacks-recommend.js'`
   - `'./ai/memory/vector-search.js'` -> `'./ai/library/stacks-search.js'`
   - `'./ai/memory/vector-serialize.js'` -> `'./ai/library/stacks-serialize.js'`
   - `'./ai/memory/vector-store.js'` -> `'./ai/library/stacks.js'`
   - `'./errors/memory.js'` -> `'./errors/library.js'` (already handled in errors/index.ts)

2. Rename exported symbols:
   - `MemoryManager` -> `Library` (also keep `MemoryManager` as deprecated alias)
   - `MemoryManagerOptions` -> `LibraryOptions`
   - `createMemoryManager` -> `createLibrary` (also keep `createMemoryManager` as deprecated alias)
   - `MemoryMiddleware` -> `LibraryServices`
   - `MemoryMiddlewareOptions` -> `LibraryServicesOptions`
   - `createMemoryMiddleware` -> `createLibraryServices`
   - `MiddlewareContext` -> `LibraryContext`
   - `CompressionOptions` -> keep (still valid for preservation)
   - `TopicIndexOptions` -> keep (still valid for cataloging)
   - `LearningEngine` -> `PatronLearningEngine`
   - `createLearningEngine` -> `createPatronLearningEngine`
   - `VectorStore` -> `Stacks`
   - `VectorStoreOptions` -> `StacksOptions`
   - `createVectorStore` -> `createStacks`
   - `VectorSearchConfig` -> `StacksSearchConfig`
   - `vectorSearch` -> `stacksSearch`
   - `advancedVectorSearch` -> `advancedStacksSearch`
   - `filterEntriesByDateRange` -> `filterVolumesByDateRange`
   - `filterEntriesByMetadata` -> `filterVolumesByMetadata`
   - `textSearchEntries` -> `textSearchVolumes`
   - All type renames from Task 2 (`VectorEntry`->`Volume`, `SearchResult`->`Lookup`, etc.)
   - Error renames from Task 5

   Keep deprecated aliases for the most common exports (`createMemoryManager`, `MemoryManager`, `VectorEntry`, `SearchResult`, etc.) so downstream code compiles while migrating.

3. Rename `registerMemoryTools` export -> `registerLibraryTools` (also keep old name as alias).

4. Run typecheck: `bun run typecheck`

5. Commit:
   ```
   refactor: update lib.ts barrel exports to library terminology
   ```

---

### Task 7 — Update internal consumers (agentic-loop, builtin-tools, mcp-server, subagent-tools)

**Files:**
- Modify: `src/ai/loop/types.ts`
- Modify: `src/ai/loop/agentic-loop.ts`
- Modify: `src/ai/tools/builtin-tools.ts`
- Modify: `src/ai/tools/subagent-tools.ts`
- Modify: `src/ai/mcp/mcp-server.ts`
- Modify: `src/ai/tools/index.ts` (barrel)
- Test: `bun run typecheck`

**Steps:**

1. In `src/ai/loop/types.ts`:
   - Change `import type { MemoryMiddleware } from '../memory/middleware.js'` -> `import type { LibraryServices } from '../library/library-services.js'`
   - Change `import type { TextGenerationProvider } from '../memory/types.js'` -> `import type { TextGenerationProvider } from '../library/types.js'`
   - Rename `memoryMiddleware` option -> `libraryServices` in `AgenticLoopOptions`

2. In `src/ai/loop/agentic-loop.ts`:
   - Update destructured option: `memoryMiddleware` -> `libraryServices`
   - Update all references: `memoryMiddleware.enrichSystemPrompt(...)` -> `libraryServices.enrichSystemPrompt(...)`
   - Update all references: `memoryMiddleware.afterResponse(...)` -> `libraryServices.afterResponse(...)`

3. In `src/ai/tools/builtin-tools.ts`:
   - Change `import type { MemoryManager } from '../memory/memory.js'` -> `import type { Library } from '../library/library.js'`
   - Rename `registerMemoryTools` -> `registerLibraryTools`
   - Rename parameter `memoryManager: MemoryManager` -> `library: Library`
   - Rename tool names: `memory_search` -> `library_search`, `memory_add` -> `library_shelve`, `memory_delete` -> `library_withdraw`
   - Update tool descriptions to use library terminology
   - Update result formatting: `r.entry.metadata.topic` -> `r.volume.metadata.topic`, `r.entry.text` -> `r.volume.text`
   - Update category: `'memory'` -> `'library'`
   - Keep `registerMemoryTools` as a deprecated re-export alias

4. In `src/ai/tools/subagent-tools.ts`:
   - No direct memory imports — but if memory tools are copied from parent, the tool names change. No code changes needed here yet (tool copying is by name from parent registry).

5. In `src/ai/mcp/mcp-server.ts`:
   - Change `import type { MemoryManager } from '../memory/memory.js'` -> `import type { Library } from '../library/library.js'`
   - Rename `memoryManager` option -> `library` in `MCPServerOptions`
   - Rename tool names: `'memory-search'` -> `'library-search'`, `'memory-add'` -> `'library-shelve'`, `'memory-delete'` -> `'library-withdraw'`
   - Update tool titles and descriptions
   - Update all `memoryManager.search(...)` -> `library.search(...)`, etc.
   - Update log messages: `'memory-search'` -> `'library-search'`, etc.

6. Update `src/ai/tools/index.ts` barrel to export `registerLibraryTools` (and keep `registerMemoryTools` as alias).

7. Run typecheck: `bun run typecheck`

8. Commit:
   ```
   refactor: update agentic-loop, builtin-tools, mcp-server to library terminology
   ```

---

### Task 8 — Update simse-code consumers (tool-registry.ts, cli.ts)

**Files:**
- Modify: `simse-code/tool-registry.ts`
- Modify: `simse-code/cli.ts`
- Test: `bun run typecheck`

**Steps:**

1. In `simse-code/tool-registry.ts`:
   - Change `import type { ... MemoryManager ... } from 'simse'` -> `import type { ... Library ... } from 'simse'`
   - Rename option: `memoryManager?: MemoryManager` -> `library?: Library`
   - Rename tool names: `memory_search` -> `library_search`, `memory_add` -> `library_shelve`
   - Update handler code: `memoryManager.search(...)` -> `library.search(...)`, `memoryManager.add(...)` -> `library.add(...)`
   - Update result formatting: `r.entry.metadata.topic` -> `r.volume.metadata.topic`, `r.entry.text` -> `r.volume.text`

2. In `simse-code/cli.ts` — search for all memory-related sections and update:
   - Import renames: `MemoryManager` -> `Library`, `createMemoryManager` -> `createLibrary`, `MemoryMiddleware` -> `LibraryServices`, `createMemoryMiddleware` -> `createLibraryServices`
   - Variable renames: `memoryManager` -> `library`
   - Option field renames: `memoryManager: app.memory` -> `library: app.library`
   - String references: update `/memory` command to `/library`, update display labels `'Memory'` -> `'Library'`
   - Tool names in any string references
   - Update `session.memoryEnabled` -> `session.libraryEnabled`
   - Update UI strings: `'Loading memory...'` -> `'Loading library...'`, etc.

3. Run typecheck: `bun run typecheck`

4. Commit:
   ```
   refactor: update simse-code consumers to library terminology
   ```

---

### Task 9 — Rename tools: memory_search -> library_search, memory_add -> library_shelve, memory_delete -> library_withdraw

This is already handled in Tasks 7 and 8. This task exists as a verification checkpoint.

**Files:**
- Verify: `src/ai/tools/builtin-tools.ts`
- Verify: `src/ai/mcp/mcp-server.ts`
- Verify: `simse-code/tool-registry.ts`
- Test: `bun test tests/builtin-tools.test.ts`

**Steps:**

1. Update `tests/builtin-tools.test.ts`:
   - Change `import type { MemoryManager } from '../src/ai/memory/memory.js'` -> `import type { Library } from '../src/ai/library/library.js'`
   - Change `import { registerMemoryTools, ... } from '../src/ai/tools/builtin-tools.js'` -> `import { registerLibraryTools, ... } from '../src/ai/tools/builtin-tools.js'`
   - Rename `createMockMemoryManager` -> `createMockLibrary`
   - Update mock to return `{ volume: { ... }, score: 0.9 }` instead of `{ entry: { ... }, score: 0.9 }`
   - Update tool name assertions: `'memory_search'` -> `'library_search'`, `'memory_add'` -> `'library_shelve'`, `'memory_delete'` -> `'library_withdraw'`
   - Update mock interface to match `Library` (add `compendium`, `patronProfile` etc.; rename `learningProfile` -> `patronProfile`, `summarize` -> `compendium`)

2. Run test to verify pass: `bun test tests/builtin-tools.test.ts`

3. Grep for any remaining `memory_search`, `memory_add`, `memory_delete` tool name strings in the entire codebase:
   ```bash
   grep -r 'memory_search\|memory_add\|memory_delete' src/ simse-code/ tests/ --include='*.ts'
   ```
   Fix any stragglers.

4. Commit:
   ```
   refactor: verify tool renames library_search, library_shelve, library_withdraw
   ```

---

### Task 10 — Update all test files

**Files:**
- Modify: `tests/memory-manager.test.ts` -> rename to `tests/library.test.ts` (if not already created in Task 3, merge)
- Modify: `tests/vector-store.test.ts` -> rename to `tests/stacks.test.ts` (if not already created in Task 4, merge)
- Modify: `tests/memory-middleware.test.ts` -> rename to `tests/library-services.test.ts`
- Modify: `tests/e2e-memory-pipeline.test.ts` -> rename to `tests/e2e-library-pipeline.test.ts`
- Modify: `tests/hierarchical-memory-integration.test.ts` -> rename to `tests/hierarchical-library-integration.test.ts`
- Modify: `tests/learning.test.ts` -> rename to `tests/patron-learning.test.ts`
- Modify: `tests/per-topic-learning.test.ts` -> rename to `tests/per-topic-patron-learning.test.ts`
- Modify: `tests/recommendation.test.ts` (update imports)
- Modify: `tests/builtin-tools.test.ts` (done in Task 9)
- Modify: `tests/agentic-loop.test.ts` (update memoryMiddleware -> libraryServices)
- Modify: `tests/query-dsl.test.ts` (update import paths)
- Modify: `tests/inverted-index.test.ts` (update import paths)
- Modify: `tests/builtin-subagents.test.ts` (update if references memory tools)
- Modify: `tests/subagent-tools.test.ts` (update if references memory tools)
- Modify: `tests/loop-events.test.ts` (update event names if referenced)
- Modify: `tests/doom-loop.test.ts` (update if references memoryMiddleware)
- Test: `bun test`

**Steps:**

1. For each test file, update:
   - Import paths: `'../src/ai/memory/...'` -> `'../src/ai/library/...'`
   - Type names: `MemoryManager` -> `Library`, `VectorEntry` -> `Volume`, `SearchResult` -> `Lookup`, etc.
   - Factory names: `createMemoryManager` -> `createLibrary`, `createVectorStore` -> `createStacks`
   - Field access: `.entry.` -> `.volume.`
   - Function names: `registerMemoryTools` -> `registerLibraryTools`
   - Tool names in assertions: `'memory_search'` -> `'library_search'`, etc.
   - Option names: `memoryMiddleware` -> `libraryServices`, `vectorStoreOptions` -> `stacksOptions`
   - Error names: `createMemoryError` -> `createLibraryError`

2. Rename test files (copy + delete old):
   ```bash
   cp tests/memory-manager.test.ts tests/library.test.ts
   cp tests/vector-store.test.ts tests/stacks.test.ts
   cp tests/memory-middleware.test.ts tests/library-services.test.ts
   cp tests/e2e-memory-pipeline.test.ts tests/e2e-library-pipeline.test.ts
   cp tests/hierarchical-memory-integration.test.ts tests/hierarchical-library-integration.test.ts
   cp tests/learning.test.ts tests/patron-learning.test.ts
   cp tests/per-topic-learning.test.ts tests/per-topic-patron-learning.test.ts
   rm tests/memory-manager.test.ts tests/vector-store.test.ts tests/memory-middleware.test.ts tests/e2e-memory-pipeline.test.ts tests/hierarchical-memory-integration.test.ts tests/learning.test.ts tests/per-topic-learning.test.ts
   ```
   (If Task 3/4 already created `tests/library.test.ts` and `tests/stacks.test.ts`, merge the content from the old renamed files into those.)

3. Run all tests: `bun test`

4. Fix any remaining failures.

5. Commit:
   ```
   refactor: rename and update all test files for library terminology
   ```

---

### Task 11 — Update CLAUDE.md documentation

**Files:**
- Modify: `CLAUDE.md`
- Test: visual review

**Steps:**

1. Update the Module Layout section:
   - Replace all `src/ai/memory/` paths with `src/ai/library/` paths
   - Rename files in the layout:
     - `memory.ts` -> `library.ts` with description: `Library: add/search/recommend/compendium/findDuplicates`
     - `vector-store.ts` -> `stacks.ts` with description: `Stacks: file-backed storage with indexes + preservation`
     - `vector-persistence.ts` -> `stacks-persistence.ts`
     - `compression.ts` -> `preservation.ts` with description: `Float32 base64 embedding encode/decode, gzip wrappers`
     - `indexing.ts` -> `cataloging.ts` with description: `TopicIndex, MetadataIndex, MagnitudeCache factories`
     - `learning.ts` -> `patron-learning.ts` with description: `Adaptive patron learning engine`
   - Add new files to layout (placeholders for Phase 2):
     - `topic-catalog.ts` — `TopicCatalog with resolve/relocate/merge/alias`
     - `librarian.ts` — `Librarian: extract/summarize/classifyTopic/reorganize`
     - `circulation-desk.ts` — `CirculationDesk: async background processing queue`
     - `shelf.ts` — `Shelf: agent-scoped library view`

2. Update Key Patterns section:
   - `MemoryManager` -> `Library`
   - `createMemoryManager()` -> `createLibrary()`
   - `createMemoryError` -> `createLibraryError`
   - `vector-store.ts` -> `stacks.ts`
   - `VectorStore` -> `Stacks`
   - `VectorEntry` -> `Volume`
   - `SearchResult` -> `Lookup`
   - `writeLock` note: `stacks.ts` uses a promise-chain

3. Update Memory System section -> rename to "Library System":
   - "Compression" -> "Preservation" (`preservation.ts`)
   - "Indexing" -> "Cataloging" (`cataloging.ts`)
   - Tool names: `memory-search` -> `library-search`, `memory-add` -> `library-shelve`

4. Remove deprecated backward-compat type aliases from types.ts and lib.ts now that all consumers are migrated. (Or leave them with `@deprecated` tags if external consumers may exist.)

5. Commit:
   ```
   docs: update CLAUDE.md for library terminology
   ```

---

## Phase 2: New Features (Build on Renamed Foundation)

### Task 12 — Add `shelf` metadata field and `createShelf()` / `library.shelf()` method

**Files:**
- Create: `src/ai/library/shelf.ts`
- Modify: `src/ai/library/types.ts` (add `Shelf` interface)
- Modify: `src/ai/library/library.ts` (add `shelf()` and `shelves()` methods)
- Test: `tests/shelf.test.ts`

**Steps:**

1. Write a failing test:
   ```typescript
   // tests/shelf.test.ts
   import { beforeEach, describe, expect, it, mock } from 'bun:test';
   import { createLibrary, type Library } from '../src/ai/library/library.js';
   import type {
   	EmbeddingProvider,
   	LibraryConfig,
   	Shelf,
   } from '../src/ai/library/types.js';
   import { createMemoryStorage, createSilentLogger } from './utils/mocks.js';

   function createMockEmbedder(dim = 3): EmbeddingProvider {
   	let callCount = 0;
   	return {
   		embed: mock(async (input: string | readonly string[]) => {
   			const texts = typeof input === 'string' ? [input] : input;
   			callCount++;
   			return {
   				embeddings: texts.map((_, i) => {
   					const base = (callCount * 10 + i) * 0.1;
   					return Array.from({ length: dim }, (__, j) =>
   						Math.sin(base + j * 0.7),
   					);
   				}),
   			};
   		}),
   	};
   }

   const defaultConfig: LibraryConfig = {
   	enabled: true,
   	embeddingAgent: 'test-embedder',
   	similarityThreshold: 0,
   	maxResults: 10,
   };

   describe('Shelf', () => {
   	let library: Library;

   	beforeEach(async () => {
   		library = createLibrary(createMockEmbedder(), defaultConfig, {
   			storage: createMemoryStorage(),
   			logger: createSilentLogger(),
   			stacksOptions: {
   				autoSave: true,
   				flushIntervalMs: 0,
   				learning: { enabled: false },
   			},
   		});
   		await library.initialize();
   	});

   	it('library.shelf() returns a Shelf with the given name', () => {
   		const shelf = library.shelf('researcher');
   		expect(shelf.name).toBe('researcher');
   	});

   	it('shelf.add() stores with shelf metadata', async () => {
   		const shelf = library.shelf('researcher');
   		const id = await shelf.add('finding about APIs', { topic: 'api' });
   		const volume = library.getById(id);
   		expect(volume).toBeDefined();
   		expect(volume!.metadata.shelf).toBe('researcher');
   	});

   	it('shelf.search() returns only volumes from that shelf', async () => {
   		const s1 = library.shelf('researcher');
   		const s2 = library.shelf('writer');
   		await s1.add('API endpoint design');
   		await s2.add('prose style guide');
   		const results = await s1.search('design');
   		for (const r of results) {
   			expect(r.volume.metadata.shelf).toBe('researcher');
   		}
   	});

   	it('shelf.searchGlobal() returns volumes from all shelves', async () => {
   		const s1 = library.shelf('researcher');
   		await library.add('global knowledge');
   		await s1.add('shelf-scoped note');
   		const results = await s1.searchGlobal('knowledge');
   		// Should include both global and shelf-scoped
   		expect(results.length).toBeGreaterThan(0);
   	});

   	it('shelf.volumes() returns only that shelf volumes', async () => {
   		const shelf = library.shelf('test');
   		await shelf.add('note 1');
   		await shelf.add('note 2');
   		await library.add('unscoped note');
   		const vols = shelf.volumes();
   		expect(vols.length).toBe(2);
   		for (const v of vols) {
   			expect(v.metadata.shelf).toBe('test');
   		}
   	});

   	it('library.shelves() lists all shelf names', async () => {
   		library.shelf('alpha');
   		library.shelf('beta');
   		await library.shelf('alpha').add('note');
   		const names = library.shelves();
   		expect(names).toContain('alpha');
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/shelf.test.ts`

3. Add `Shelf` interface to `src/ai/library/types.ts`:
   ```typescript
   export interface Shelf {
   	readonly name: string;
   	readonly add: (text: string, metadata?: Record<string, string>) => Promise<string>;
   	readonly search: (query: string, maxResults?: number, threshold?: number) => Promise<Lookup[]>;
   	readonly searchGlobal: (query: string, maxResults?: number, threshold?: number) => Promise<Lookup[]>;
   	readonly volumes: () => Volume[];
   }
   ```

4. Create `src/ai/library/shelf.ts`:
   ```typescript
   import type { Library } from './library.js';
   import type { Lookup, Shelf, Volume } from './types.js';

   export function createShelf(name: string, library: Library): Shelf {
   	const add = async (
   		text: string,
   		metadata: Record<string, string> = {},
   	): Promise<string> => {
   		return library.add(text, { ...metadata, shelf: name });
   	};

   	const search = async (
   		query: string,
   		maxResults?: number,
   		threshold?: number,
   	): Promise<Lookup[]> => {
   		const results = await library.search(query, maxResults, threshold);
   		return results.filter((r) => r.volume.metadata.shelf === name);
   	};

   	const searchGlobal = async (
   		query: string,
   		maxResults?: number,
   		threshold?: number,
   	): Promise<Lookup[]> => {
   		return library.search(query, maxResults, threshold);
   	};

   	const volumes = (): Volume[] => {
   		return library.getAll().filter((v) => v.metadata.shelf === name);
   	};

   	return Object.freeze({ name, add, search, searchGlobal, volumes });
   }
   ```

5. In `src/ai/library/library.ts`, add `shelf()` and `shelves()` to the `Library` interface and implementation:
   ```typescript
   import { createShelf } from './shelf.js';
   // ...
   const shelfCache = new Map<string, Shelf>();

   const shelf = (name: string): Shelf => {
   	ensureInitialized();
   	let s = shelfCache.get(name);
   	if (!s) {
   		// Build the shelf with a reference to the outer Library
   		// (we need to pass `manager` here — the frozen object)
   		s = createShelf(name, manager);
   		shelfCache.set(name, s);
   	}
   	return s;
   };

   const shelves = (): string[] => {
   	ensureInitialized();
   	const names = new Set<string>();
   	for (const vol of store.getAll()) {
   		if (vol.metadata.shelf) {
   			names.add(vol.metadata.shelf);
   		}
   	}
   	return [...names];
   };
   ```
   Note: The `manager` variable is the frozen return object — assign it before freezing so `shelf()` can reference it. Pattern:
   ```typescript
   const manager: Library = Object.freeze({
   	// ... all methods including shelf, shelves
   });
   // Need to handle circular ref: createShelf needs Library ref.
   // Solution: create shelf lazily and pass `manager` which is already assigned.
   ```
   Since `Object.freeze` returns the same reference, assign before calling freeze:
   ```typescript
   const result = {
   	// all methods...
   	shelf,
   	shelves,
   };
   const manager = Object.freeze(result);
   return manager;
   ```
   And make `shelf()` close over a `let managerRef: Library` that gets assigned right after `Object.freeze`.

6. Run test to verify pass: `bun test tests/shelf.test.ts`

7. Commit:
   ```
   feat: add Shelf for agent-scoped library access
   ```

---

### Task 13 — Add TopicCatalog with resolve/relocate/merge/alias

**Files:**
- Create: `src/ai/library/topic-catalog.ts`
- Modify: `src/ai/library/types.ts` (add `TopicCatalog` interface)
- Test: `tests/topic-catalog.test.ts`

**Steps:**

1. Write a failing test:
   ```typescript
   // tests/topic-catalog.test.ts
   import { describe, expect, it } from 'bun:test';
   import {
   	createTopicCatalog,
   	type TopicCatalog,
   } from '../src/ai/library/topic-catalog.js';

   describe('TopicCatalog', () => {
   	it('resolve() registers a new topic', () => {
   		const catalog = createTopicCatalog();
   		const resolved = catalog.resolve('architecture/database');
   		expect(resolved).toBe('architecture/database');
   		expect(catalog.sections()).toContainEqual(
   			expect.objectContaining({ topic: 'architecture/database' }),
   		);
   	});

   	it('resolve() normalizes similar topics via Levenshtein', () => {
   		const catalog = createTopicCatalog();
   		catalog.resolve('architecture/database');
   		// 'architecure/database' is a typo — should match existing
   		const resolved = catalog.resolve('architecure/database');
   		expect(resolved).toBe('architecture/database');
   	});

   	it('resolve() maps aliases to canonical names', () => {
   		const catalog = createTopicCatalog();
   		catalog.resolve('architecture/database');
   		catalog.addAlias('db', 'architecture/database');
   		const resolved = catalog.resolve('db');
   		expect(resolved).toBe('architecture/database');
   	});

   	it('relocate() moves a volume to a new topic', () => {
   		const catalog = createTopicCatalog();
   		catalog.resolve('bugs/open');
   		catalog.registerVolume('v1', 'bugs/open');
   		catalog.relocate('v1', 'bugs/resolved');
   		const volumes = catalog.volumes('bugs/resolved');
   		expect(volumes).toContain('v1');
   		expect(catalog.volumes('bugs/open')).not.toContain('v1');
   	});

   	it('merge() combines two sections', () => {
   		const catalog = createTopicCatalog();
   		catalog.resolve('arch/db');
   		catalog.resolve('architecture/database');
   		catalog.registerVolume('v1', 'arch/db');
   		catalog.registerVolume('v2', 'architecture/database');
   		catalog.merge('arch/db', 'architecture/database');
   		const volumes = catalog.volumes('architecture/database');
   		expect(volumes).toContain('v1');
   		expect(volumes).toContain('v2');
   	});

   	it('sections() returns the full tree', () => {
   		const catalog = createTopicCatalog();
   		catalog.resolve('architecture/database/schema');
   		catalog.resolve('architecture/api');
   		const sections = catalog.sections();
   		const topics = sections.map((s) => s.topic);
   		expect(topics).toContain('architecture');
   		expect(topics).toContain('architecture/database');
   		expect(topics).toContain('architecture/database/schema');
   		expect(topics).toContain('architecture/api');
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/topic-catalog.test.ts`

3. Add `TopicCatalog` interface to `src/ai/library/types.ts`:
   ```typescript
   export interface TopicCatalogSection {
   	readonly topic: string;
   	readonly parent?: string;
   	readonly children: readonly string[];
   	readonly volumeCount: number;
   }

   export interface TopicCatalog {
   	readonly resolve: (proposedTopic: string) => string;
   	readonly relocate: (volumeId: string, newTopic: string) => void;
   	readonly merge: (sourceTopic: string, targetTopic: string) => void;
   	readonly sections: () => TopicCatalogSection[];
   	readonly volumes: (topic: string) => readonly string[];
   	readonly addAlias: (alias: string, canonical: string) => void;
   	readonly registerVolume: (volumeId: string, topic: string) => void;
   	readonly removeVolume: (volumeId: string) => void;
   	readonly getTopicForVolume: (volumeId: string) => string | undefined;
   }
   ```

4. Create `src/ai/library/topic-catalog.ts`:
   ```typescript
   import { levenshteinSimilarity } from './text-search.js';
   import type { TopicCatalog, TopicCatalogSection } from './types.js';

   export interface TopicCatalogOptions {
   	/** Minimum Levenshtein similarity to match an existing topic. Defaults to 0.85. */
   	readonly similarityThreshold?: number;
   }

   export function createTopicCatalog(options?: TopicCatalogOptions): TopicCatalog {
   	const similarityThreshold = options?.similarityThreshold ?? 0.85;

   	// topic -> Set<volumeId>
   	const topicToVolumes = new Map<string, Set<string>>();
   	// volumeId -> topic
   	const volumeToTopic = new Map<string, string>();
   	// alias -> canonical topic
   	const aliases = new Map<string, string>();
   	// topic -> Set<child topic>
   	const topicToChildren = new Map<string, Set<string>>();

   	const ensureTopicExists = (topic: string): void => {
   		const normalized = topic.toLowerCase().trim();
   		if (!topicToVolumes.has(normalized)) {
   			topicToVolumes.set(normalized, new Set());
   		}
   		// Ensure all ancestors exist
   		const parts = normalized.split('/');
   		for (let i = 1; i < parts.length; i++) {
   			const parent = parts.slice(0, i).join('/');
   			const child = parts.slice(0, i + 1).join('/');
   			if (!topicToVolumes.has(parent)) {
   				topicToVolumes.set(parent, new Set());
   			}
   			let children = topicToChildren.get(parent);
   			if (!children) {
   				children = new Set();
   				topicToChildren.set(parent, children);
   			}
   			children.add(child);
   		}
   	};

   	const resolve = (proposedTopic: string): string => {
   		const normalized = proposedTopic.toLowerCase().trim();

   		// 1. Check aliases
   		const aliased = aliases.get(normalized);
   		if (aliased) return aliased;

   		// 2. Check exact match
   		if (topicToVolumes.has(normalized)) return normalized;

   		// 3. Check similarity against existing topics
   		let bestMatch: string | undefined;
   		let bestScore = 0;
   		for (const existing of topicToVolumes.keys()) {
   			const score = levenshteinSimilarity(normalized, existing);
   			if (score >= similarityThreshold && score > bestScore) {
   				bestScore = score;
   				bestMatch = existing;
   			}
   		}

   		if (bestMatch) return bestMatch;

   		// 4. Register as new topic
   		ensureTopicExists(normalized);
   		return normalized;
   	};

   	const registerVolume = (volumeId: string, topic: string): void => {
   		const canonical = resolve(topic);
   		// Remove from old topic if exists
   		const oldTopic = volumeToTopic.get(volumeId);
   		if (oldTopic) {
   			topicToVolumes.get(oldTopic)?.delete(volumeId);
   		}
   		topicToVolumes.get(canonical)?.add(volumeId);
   		volumeToTopic.set(volumeId, canonical);
   	};

   	const removeVolume = (volumeId: string): void => {
   		const topic = volumeToTopic.get(volumeId);
   		if (topic) {
   			topicToVolumes.get(topic)?.delete(volumeId);
   			volumeToTopic.delete(volumeId);
   		}
   	};

   	const relocate = (volumeId: string, newTopic: string): void => {
   		removeVolume(volumeId);
   		registerVolume(volumeId, newTopic);
   	};

   	const merge = (sourceTopic: string, targetTopic: string): void => {
   		const srcNorm = sourceTopic.toLowerCase().trim();
   		const tgtNorm = resolve(targetTopic);
   		const srcVolumes = topicToVolumes.get(srcNorm);
   		if (!srcVolumes) return;

   		const tgtVolumes = topicToVolumes.get(tgtNorm);
   		if (!tgtVolumes) {
   			ensureTopicExists(tgtNorm);
   		}

   		for (const volumeId of srcVolumes) {
   			topicToVolumes.get(tgtNorm)?.add(volumeId);
   			volumeToTopic.set(volumeId, tgtNorm);
   		}

   		srcVolumes.clear();
   		// Add alias so future references to source go to target
   		aliases.set(srcNorm, tgtNorm);
   	};

   	const sections = (): TopicCatalogSection[] => {
   		const result: TopicCatalogSection[] = [];
   		for (const [topic, vols] of topicToVolumes) {
   			const parts = topic.split('/');
   			const parent = parts.length > 1
   				? parts.slice(0, -1).join('/')
   				: undefined;
   			const children = topicToChildren.get(topic);
   			result.push({
   				topic,
   				parent,
   				children: children ? [...children] : [],
   				volumeCount: vols.size,
   			});
   		}
   		return result;
   	};

   	const volumes = (topic: string): readonly string[] => {
   		const normalized = topic.toLowerCase().trim();
   		const vols = topicToVolumes.get(normalized);
   		return vols ? [...vols] : [];
   	};

   	const addAlias = (alias: string, canonical: string): void => {
   		aliases.set(alias.toLowerCase().trim(), canonical.toLowerCase().trim());
   	};

   	const getTopicForVolume = (volumeId: string): string | undefined => {
   		return volumeToTopic.get(volumeId);
   	};

   	return Object.freeze({
   		resolve,
   		relocate,
   		merge,
   		sections,
   		volumes,
   		addAlias,
   		registerVolume,
   		removeVolume,
   		getTopicForVolume,
   	});
   }
   ```

5. Run test to verify pass: `bun test tests/topic-catalog.test.ts`

6. Commit:
   ```
   feat: add TopicCatalog with resolve, relocate, merge, and alias support
   ```

---

### Task 14 — Add Librarian with extract/summarize/classifyTopic/reorganize

**Files:**
- Create: `src/ai/library/librarian.ts`
- Modify: `src/ai/library/types.ts` (add interfaces)
- Test: `tests/librarian.test.ts`

**Steps:**

1. Write a failing test:
   ```typescript
   // tests/librarian.test.ts
   import { describe, expect, it, mock } from 'bun:test';
   import { createLibrarian } from '../src/ai/library/librarian.js';
   import type { TextGenerationProvider, Volume } from '../src/ai/library/types.js';

   function createMockGenerator(response: string): TextGenerationProvider {
   	return {
   		generate: mock(async () => response),
   	};
   }

   describe('Librarian', () => {
   	it('extract() parses LLM JSON into ExtractionResult', async () => {
   		const generator = createMockGenerator(JSON.stringify({
   			memories: [
   				{
   					text: 'Users table uses UUID primary keys',
   					topic: 'architecture/database/schema',
   					tags: ['postgresql', 'uuid', 'schema'],
   					entryType: 'fact',
   				},
   			],
   		}));
   		const librarian = createLibrarian(generator);
   		const result = await librarian.extract({
   			userInput: 'What PK type should we use?',
   			response: 'We decided to use UUID primary keys for the users table.',
   		});
   		expect(result.memories.length).toBe(1);
   		expect(result.memories[0].topic).toBe('architecture/database/schema');
   		expect(result.memories[0].entryType).toBe('fact');
   	});

   	it('extract() returns empty memories on LLM garbage', async () => {
   		const generator = createMockGenerator('not valid json');
   		const librarian = createLibrarian(generator);
   		const result = await librarian.extract({
   			userInput: 'hello',
   			response: 'hi',
   		});
   		expect(result.memories).toEqual([]);
   	});

   	it('summarize() returns a CompendiumResult', async () => {
   		const generator = createMockGenerator(
   			'PostgreSQL uses UUID PKs across all tables for consistency.',
   		);
   		const librarian = createLibrarian(generator);
   		const volumes: Volume[] = [
   			{ id: 'v1', text: 'Users table has UUID PK', embedding: [0.1], metadata: {}, timestamp: 1 },
   			{ id: 'v2', text: 'Orders table has UUID PK', embedding: [0.2], metadata: {}, timestamp: 2 },
   		];
   		const result = await librarian.summarize(volumes, 'architecture/database');
   		expect(result.text.length).toBeGreaterThan(0);
   		expect(result.sourceIds).toEqual(['v1', 'v2']);
   	});

   	it('classifyTopic() returns classification result', async () => {
   		const generator = createMockGenerator(JSON.stringify({
   			topic: 'architecture/database/schema',
   			confidence: 0.9,
   		}));
   		const librarian = createLibrarian(generator);
   		const result = await librarian.classifyTopic(
   			'Users table uses UUID PKs',
   			['architecture/database', 'bugs/open'],
   		);
   		expect(result.topic).toBe('architecture/database/schema');
   	});

   	it('reorganize() returns a plan', async () => {
   		const generator = createMockGenerator(JSON.stringify({
   			moves: [{ volumeId: 'v1', newTopic: 'architecture/database/optimization' }],
   			newSubtopics: ['architecture/database/optimization'],
   			merges: [],
   		}));
   		const librarian = createLibrarian(generator);
   		const volumes: Volume[] = [
   			{ id: 'v1', text: 'Index optimization', embedding: [0.1], metadata: {}, timestamp: 1 },
   		];
   		const result = await librarian.reorganize('architecture/database', volumes);
   		expect(result.moves.length).toBe(1);
   		expect(result.moves[0].newTopic).toBe('architecture/database/optimization');
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/librarian.test.ts`

3. Add types to `src/ai/library/types.ts`:
   ```typescript
   export interface TurnContext {
   	readonly userInput: string;
   	readonly response: string;
   }

   export interface ExtractionMemory {
   	readonly text: string;
   	readonly topic: string;
   	readonly tags: string[];
   	readonly entryType: 'fact' | 'decision' | 'observation';
   }

   export interface ExtractionResult {
   	readonly memories: readonly ExtractionMemory[];
   }

   export interface ClassificationResult {
   	readonly topic: string;
   	readonly confidence: number;
   }

   export interface ReorganizationPlan {
   	readonly moves: ReadonlyArray<{ readonly volumeId: string; readonly newTopic: string }>;
   	readonly newSubtopics: readonly string[];
   	readonly merges: ReadonlyArray<{ readonly source: string; readonly target: string }>;
   }

   export interface Librarian {
   	readonly extract: (turn: TurnContext) => Promise<ExtractionResult>;
   	readonly summarize: (volumes: readonly Volume[], topic: string) => Promise<{ text: string; sourceIds: readonly string[] }>;
   	readonly classifyTopic: (text: string, existingTopics: readonly string[]) => Promise<ClassificationResult>;
   	readonly reorganize: (topic: string, volumes: readonly Volume[]) => Promise<ReorganizationPlan>;
   }
   ```

4. Create `src/ai/library/librarian.ts` with `createLibrarian(textGenerator, options?)`:
   - `extract()`: Prompts the LLM with a structured prompt asking it to identify facts/decisions/observations from the turn, return JSON with `{ memories: [...] }`. Parses the response, validates shape, returns `ExtractionResult`. On parse failure returns `{ memories: [] }`.
   - `summarize()`: Concatenates volume texts, prompts LLM to condense, returns `{ text, sourceIds }`.
   - `classifyTopic()`: Sends text + existing topics to LLM, asks for best topic + confidence. Parses JSON response.
   - `reorganize()`: Sends all volumes for a topic to LLM, asks for moves/new subtopics/merges. Parses JSON response.

   Each method wraps LLM output parsing in try/catch and returns safe defaults on failure.

5. Run test to verify pass: `bun test tests/librarian.test.ts`

6. Commit:
   ```
   feat: add Librarian with extract, summarize, classifyTopic, reorganize
   ```

---

### Task 15 — Add CirculationDesk queue with enqueue/drain/flush

**Files:**
- Create: `src/ai/library/circulation-desk.ts`
- Modify: `src/ai/library/types.ts` (add `CirculationDesk` interface, `CirculationDeskOptions`)
- Test: `tests/circulation-desk.test.ts`

**Steps:**

1. Write a failing test:
   ```typescript
   // tests/circulation-desk.test.ts
   import { describe, expect, it, mock } from 'bun:test';
   import {
   	createCirculationDesk,
   } from '../src/ai/library/circulation-desk.js';
   import type {
   	Librarian,
   	TextGenerationProvider,
   	TopicCatalog,
   } from '../src/ai/library/types.js';

   // Mock librarian that returns a single extraction
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
   		reorganize: mock(async () => ({ moves: [], newSubtopics: [], merges: [] })),
   	};
   }

   describe('CirculationDesk', () => {
   	it('enqueueExtraction adds a job and drain processes it', async () => {
   		const librarian = createMockLibrarian();
   		const addFn = mock(async () => 'new-id');
   		const desk = createCirculationDesk({
   			librarian,
   			addVolume: addFn,
   			checkDuplicate: async () => ({ isDuplicate: false }),
   			getVolumesForTopic: () => [],
   		});

   		desk.enqueueExtraction({
   			userInput: 'What is X?',
   			response: 'X is a fact about databases.',
   		});

   		expect(desk.pending).toBe(1);
   		await desk.drain();
   		expect(desk.pending).toBe(0);
   		expect(librarian.extract).toHaveBeenCalled();
   		expect(addFn).toHaveBeenCalled();
   	});

   	it('flush() cancels all pending jobs', async () => {
   		const librarian = createMockLibrarian();
   		const desk = createCirculationDesk({
   			librarian,
   			addVolume: async () => 'id',
   			checkDuplicate: async () => ({ isDuplicate: false }),
   			getVolumesForTopic: () => [],
   		});

   		desk.enqueueExtraction({ userInput: 'a', response: 'b' });
   		desk.enqueueExtraction({ userInput: 'c', response: 'd' });
   		expect(desk.pending).toBe(2);
   		await desk.flush();
   		expect(desk.pending).toBe(0);
   	});

   	it('processing is true during drain', async () => {
   		const librarian = createMockLibrarian();
   		const desk = createCirculationDesk({
   			librarian,
   			addVolume: async () => 'id',
   			checkDuplicate: async () => ({ isDuplicate: false }),
   			getVolumesForTopic: () => [],
   		});

   		desk.enqueueExtraction({ userInput: 'a', response: 'b' });
   		const drainPromise = desk.drain();
   		// processing may be true briefly — hard to assert timing
   		await drainPromise;
   		expect(desk.processing).toBe(false);
   	});

   	it('dispose() prevents further processing', () => {
   		const librarian = createMockLibrarian();
   		const desk = createCirculationDesk({
   			librarian,
   			addVolume: async () => 'id',
   			checkDuplicate: async () => ({ isDuplicate: false }),
   			getVolumesForTopic: () => [],
   		});

   		desk.dispose();
   		desk.enqueueExtraction({ userInput: 'a', response: 'b' });
   		expect(desk.pending).toBe(0); // disposed — job not queued
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/circulation-desk.test.ts`

3. Add `CirculationDesk` interface to `src/ai/library/types.ts`:
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
   }

   export interface CirculationDesk {
   	readonly enqueueExtraction: (turn: TurnContext) => void;
   	readonly enqueueCompendium: (topic: string) => void;
   	readonly enqueueReorganization: (topic: string) => void;
   	readonly drain: () => Promise<void>;
   	readonly flush: () => Promise<void>;
   	readonly dispose: () => void;
   	readonly pending: number;
   	readonly processing: boolean;
   }
   ```

4. Create `src/ai/library/circulation-desk.ts`:
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
   	readonly addVolume: (text: string, metadata?: Record<string, string>) => Promise<string>;
   	readonly checkDuplicate: (text: string) => Promise<DuplicateCheckResult>;
   	readonly getVolumesForTopic: (topic: string) => Volume[];
   	readonly thresholds?: CirculationDeskThresholds;
   	readonly catalog?: import('./types.js').TopicCatalog;
   }

   type Job =
   	| { type: 'extraction'; turn: TurnContext }
   	| { type: 'compendium'; topic: string }
   	| { type: 'reorganization'; topic: string };

   export function createCirculationDesk(
   	options: CirculationDeskOptions,
   ): CirculationDesk {
   	const { librarian, addVolume, checkDuplicate, getVolumesForTopic, catalog } = options;
   	const minEntries = options.thresholds?.compendium?.minEntries ?? 10;
   	const minAgeMs = options.thresholds?.compendium?.minAgeMs ?? 900_000;
   	const maxVolumesPerTopic = options.thresholds?.reorganization?.maxVolumesPerTopic ?? 30;

   	const queue: Job[] = [];
   	let isProcessing = false;
   	let disposed = false;

   	const processJob = async (job: Job): Promise<void> => {
   		try {
   			switch (job.type) {
   				case 'extraction': {
   					const result = await librarian.extract(job.turn);
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
   						const plan = await librarian.reorganize(job.topic, volumes);
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

5. Run test to verify pass: `bun test tests/circulation-desk.test.ts`

6. Commit:
   ```
   feat: add CirculationDesk for async background queue processing
   ```

---

### Task 16 — Add new tools: library_catalog, library_compact

**Files:**
- Modify: `src/ai/tools/builtin-tools.ts`
- Modify: `src/ai/mcp/mcp-server.ts`
- Test: `bun test tests/builtin-tools.test.ts`

**Steps:**

1. Add failing test cases in `tests/builtin-tools.test.ts`:
   ```typescript
   it('registers library_catalog tool', () => {
   	const defs = registry.getToolDefinitions();
   	expect(defs.find((d) => d.name === 'library_catalog')).toBeDefined();
   });

   it('library_catalog returns topic tree', async () => {
   	const result = await registry.execute({
   		id: 'call_1',
   		name: 'library_catalog',
   		arguments: {},
   	});
   	expect(result.isError).toBe(false);
   });

   it('registers library_compact tool', () => {
   	const defs = registry.getToolDefinitions();
   	expect(defs.find((d) => d.name === 'library_compact')).toBeDefined();
   });
   ```

2. Run test to verify failure: `bun test tests/builtin-tools.test.ts`

3. In `src/ai/tools/builtin-tools.ts`, add two new tools within `registerLibraryTools()`:

   ```typescript
   // library_catalog — browse the topic catalog
   registerTool(
   	registry,
   	{
   		name: 'library_catalog',
   		description: 'Browse the topic catalog. Returns the hierarchical topic tree with volume counts.',
   		parameters: {
   			topic: {
   				type: 'string',
   				description: 'Optional topic to filter by (shows subtopics)',
   			},
   		},
   		category: 'library',
   		annotations: { readOnly: true },
   	},
   	async (args) => {
   		try {
   			const topics = library.getTopics();
   			const filterTopic = typeof args.topic === 'string' ? args.topic : undefined;
   			const filtered = filterTopic
   				? topics.filter((t) =>
   					t.topic === filterTopic || t.topic.startsWith(`${filterTopic}/`),
   				)
   				: topics;

   			if (filtered.length === 0) return 'No topics found.';

   			return filtered
   				.map((t) => {
   					const indent = '  '.repeat((t.topic.match(/\//g) || []).length);
   					return `${indent}${t.topic} (${t.entryCount} volumes)`;
   				})
   				.join('\n');
   		} catch (err) {
   			throw toError(err);
   		}
   	},
   );

   // library_compact — trigger compendium for a topic
   registerTool(
   	registry,
   	{
   		name: 'library_compact',
   		description: 'Trigger a compendium (summarization) for a specific topic. Condenses multiple volumes into a single summary.',
   		parameters: {
   			topic: {
   				type: 'string',
   				description: 'The topic to compact',
   				required: true,
   			},
   		},
   		category: 'library',
   	},
   	async (args) => {
   		try {
   			const topic = String(args.topic ?? '');
   			const volumes = library.filterByTopic([topic]);
   			if (volumes.length < 2) {
   				return `Topic "${topic}" has fewer than 2 volumes — nothing to compact.`;
   			}
   			const ids = volumes.map((v) => v.id);
   			const result = await library.compendium({ ids });
   			return `Created compendium ${result.compendiumId} from ${result.sourceIds.length} volumes.`;
   		} catch (err) {
   			throw toError(err);
   		}
   	},
   );
   ```

4. Add corresponding MCP tools in `src/ai/mcp/mcp-server.ts`:
   - `'library-catalog'` tool
   - `'library-compact'` tool

5. Update mock library in tests to include `getTopics`, `filterByTopic`, and `compendium` methods.

6. Run test to verify pass: `bun test tests/builtin-tools.test.ts`

7. Commit:
   ```
   feat: add library_catalog and library_compact tools
   ```

---

### Task 17 — Wire LibraryServices middleware into agentic loop

**Files:**
- Modify: `src/ai/library/library-services.ts`
- Modify: `src/ai/loop/agentic-loop.ts` (already done in Task 7 for the rename)
- Test: `tests/library-services.test.ts`

**Steps:**

1. Write a failing test:
   ```typescript
   // tests/library-services.test.ts
   import { describe, expect, it, mock } from 'bun:test';
   import {
   	createLibraryServices,
   } from '../src/ai/library/library-services.js';
   import type { Library } from '../src/ai/library/library.js';

   function createMockLibrary(): Library {
   	// ... mock with search returning results, add working, etc.
   	return {
   		// ... same shape as mock from builtin-tools test
   		isInitialized: true,
   		size: 5,
   		search: mock(async () => [
   			{
   				volume: {
   					id: '1',
   					text: 'relevant context',
   					embedding: [0.1],
   					metadata: { topic: 'test' },
   					timestamp: Date.now(),
   				},
   				score: 0.9,
   			},
   		]),
   		add: mock(async () => 'new-id'),
   		// ... rest of Library interface
   	} as unknown as Library;
   }

   describe('LibraryServices', () => {
   	it('enrichSystemPrompt injects library context', async () => {
   		const library = createMockLibrary();
   		const services = createLibraryServices(library);
   		const result = await services.enrichSystemPrompt({
   			userInput: 'tell me about databases',
   			currentSystemPrompt: 'You are a helper.',
   			conversationHistory: '',
   			turn: 1,
   		});
   		expect(result).toContain('relevant context');
   	});

   	it('afterResponse stores Q&A in library', async () => {
   		const library = createMockLibrary();
   		const services = createLibraryServices(library);
   		await services.afterResponse('What is X?', 'X is a thing.');
   		expect(library.add).toHaveBeenCalled();
   	});

   	it('afterResponse with circulationDesk enqueues extraction', async () => {
   		const library = createMockLibrary();
   		const enqueueMock = mock(() => {});
   		const services = createLibraryServices(library, {
   			circulationDesk: {
   				enqueueExtraction: enqueueMock,
   				enqueueCompendium: mock(() => {}),
   				enqueueReorganization: mock(() => {}),
   				drain: mock(async () => {}),
   				flush: mock(async () => {}),
   				dispose: mock(() => {}),
   				pending: 0,
   				processing: false,
   			},
   		});
   		await services.afterResponse('What is X?', 'X is a thing.');
   		expect(enqueueMock).toHaveBeenCalled();
   	});
   });
   ```

2. Run test to verify failure: `bun test tests/library-services.test.ts`

3. Update `src/ai/library/library-services.ts`:
   - Rename `MemoryMiddleware` -> `LibraryServices` (interface)
   - Rename `MemoryMiddlewareOptions` -> `LibraryServicesOptions`
   - Rename `MiddlewareContext` -> `LibraryContext`
   - Rename `createMemoryMiddleware` -> `createLibraryServices`
   - Add optional `circulationDesk` parameter to options
   - In `afterResponse()`: if `circulationDesk` is provided, call `circulationDesk.enqueueExtraction({ userInput, response })` instead of (or in addition to) direct `library.add()`
   - Update `import type { MemoryManager } from './memory.js'` -> `import type { Library } from './library.js'`
   - Rename internal variable: `memoryManager` -> `library`
   - Update `SearchResult` -> `Lookup`, `r.entry.text` -> `r.volume.text` in `prompt-injection.ts`

4. Run test to verify pass: `bun test tests/library-services.test.ts`

5. Commit:
   ```
   feat: wire LibraryServices with CirculationDesk for per-turn extraction
   ```

---

### Task 18 — Add subagent shelf integration

**Files:**
- Modify: `src/ai/tools/subagent-tools.ts`
- Test: `tests/subagent-tools.test.ts`

**Steps:**

1. Write a failing test:
   ```typescript
   // Add to tests/subagent-tools.test.ts
   it('subagent spawns with a shelf-scoped library', async () => {
   	// Test that when options include a library, the child registry
   	// gets library tools scoped to a shelf named after the subagent description
   	// This is a behavioral test — verify the child loop gets tools that
   	// auto-scope to a shelf.
   });
   ```

2. Run test to verify failure: `bun test tests/subagent-tools.test.ts`

3. In `src/ai/tools/subagent-tools.ts`:
   - Add optional `library?: Library` to `SubagentToolsOptions`
   - When spawning a subagent (in `subagent_spawn` handler):
     - If `options.library` is provided:
       - Create a shelf: `const shelf = options.library.shelf(desc.replace(/\s+/g, '-').toLowerCase())`
       - Register shelf-scoped library tools on the child registry (replacing the parent's library tools):
         - `library_search` -> delegates to `shelf.search()`
         - `library_shelve` -> delegates to `shelf.add()`
         - `library_search_global` -> delegates to `shelf.searchGlobal()`

4. Run test to verify pass: `bun test tests/subagent-tools.test.ts`

5. Commit:
   ```
   feat: integrate shelf-scoped library into subagent spawning
   ```

---

### Task 19 — Add configurable compendium thresholds

**Files:**
- Modify: `src/ai/library/types.ts` (already has `CirculationDeskThresholds` from Task 15)
- Modify: `src/ai/library/library.ts` (accept thresholds in `LibraryOptions`)
- Test: `tests/circulation-desk.test.ts`

**Steps:**

1. Add failing test:
   ```typescript
   // Add to tests/circulation-desk.test.ts
   it('respects custom compendium thresholds', async () => {
   	const librarian = createMockLibrarian();
   	const addFn = mock(async () => 'id');
   	const desk = createCirculationDesk({
   		librarian,
   		addVolume: addFn,
   		checkDuplicate: async () => ({ isDuplicate: false }),
   		getVolumesForTopic: () => [],
   		thresholds: {
   			compendium: {
   				minEntries: 3,
   				minAgeMs: 60_000,
   				deleteOriginals: true,
   			},
   			reorganization: {
   				maxVolumesPerTopic: 5,
   			},
   		},
   	});

   	// With only 2 volumes and threshold of 3, compendium should not trigger
   	desk.enqueueCompendium('test/topic');
   	await desk.drain();
   	expect(librarian.summarize).not.toHaveBeenCalled();
   });
   ```

2. Run test to verify failure: `bun test tests/circulation-desk.test.ts`

3. Wire thresholds into `LibraryOptions`:
   ```typescript
   // In src/ai/library/library.ts LibraryOptions:
   readonly circulationDeskThresholds?: CirculationDeskThresholds;
   ```

4. Pass thresholds through when creating a CirculationDesk in the library or in LibraryServices.

5. Run test to verify pass: `bun test tests/circulation-desk.test.ts`

6. Run full test suite: `bun test`

7. Run typecheck: `bun run typecheck`

8. Run lint: `bun run lint`

9. Commit:
   ```
   feat: add configurable compendium and reorganization thresholds
   ```

---

## Post-Implementation Checklist

After all tasks are complete:

1. Remove all `@deprecated` backward-compat type aliases from `src/ai/library/types.ts`
2. Remove all `@deprecated` backward-compat factory aliases from `src/errors/library.ts`
3. Remove all `@deprecated` re-exports from `src/lib.ts`
4. Run full suite:
   ```bash
   bun run typecheck
   bun run lint
   bun test
   ```
5. Delete old `src/errors/memory.ts` if not already deleted
6. Verify no remaining references to `src/ai/memory/` anywhere:
   ```bash
   grep -r 'ai/memory' src/ simse-code/ tests/ --include='*.ts'
   ```
7. Final commit:
   ```
   chore: remove deprecated backward-compat aliases after library migration
   ```
