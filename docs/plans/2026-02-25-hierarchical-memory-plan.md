# Hierarchical Memory System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the memory subsystem with hierarchical topics, rich metadata operators, improved adaptive learning, BM25 search with inverted index, query DSL, and decompose vector-store.ts into focused modules.

**Architecture:** Factory functions returning frozen readonly interfaces (no classes). Each new module is a standalone factory composed internally by createVectorStore. TDD throughout — write failing test, implement, verify, commit.

**Tech Stack:** Bun runtime, Bun test runner, Biome linter, TypeScript strict mode with verbatimModuleSyntax.

---

## Task 1: Add New Metadata Operators to Types

**Files:**
- Modify: `src/ai/memory/types.ts:93-110` (MetadataMatchMode + MetadataFilter)

**Step 1: Write the failing test**

Create `tests/metadata-operators.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { matchesMetadataFilter } from '../src/ai/memory/text-search.js';
import type { MetadataFilter } from '../src/ai/memory/types.js';

describe('new metadata operators', () => {
	it('gt: matches when value is greater', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'gt' };
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(false);
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(false);
	});

	it('gte: matches when value is greater or equal', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'gte' };
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(false);
	});

	it('lt: matches when value is less', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'lt' };
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(false);
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(false);
	});

	it('lte: matches when value is less or equal', () => {
		const filter: MetadataFilter = { key: 'score', value: '5', mode: 'lte' };
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(false);
	});

	it('in: matches when value is in array', () => {
		const filter: MetadataFilter = { key: 'status', value: ['active', 'pending'], mode: 'in' };
		expect(matchesMetadataFilter({ status: 'active' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ status: 'closed' }, filter)).toBe(false);
	});

	it('notIn: matches when value is not in array', () => {
		const filter: MetadataFilter = { key: 'status', value: ['blocked', 'closed'], mode: 'notIn' };
		expect(matchesMetadataFilter({ status: 'active' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ status: 'closed' }, filter)).toBe(false);
	});

	it('between: matches when value is in range', () => {
		const filter: MetadataFilter = { key: 'score', value: ['3', '7'], mode: 'between' };
		expect(matchesMetadataFilter({ score: '5' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '3' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '7' }, filter)).toBe(true);
		expect(matchesMetadataFilter({ score: '1' }, filter)).toBe(false);
		expect(matchesMetadataFilter({ score: '10' }, filter)).toBe(false);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/metadata-operators.test.ts`
Expected: TypeScript errors on new mode values

**Step 3: Update types**

In `src/ai/memory/types.ts`, change `MetadataMatchMode` (line ~93) to:

```typescript
export type MetadataMatchMode =
	| 'eq'
	| 'neq'
	| 'contains'
	| 'startsWith'
	| 'endsWith'
	| 'regex'
	| 'exists'
	| 'notExists'
	| 'gt'
	| 'gte'
	| 'lt'
	| 'lte'
	| 'in'
	| 'notIn'
	| 'between';
```

Update `MetadataFilter` (line ~103) — change `value` to support arrays:

```typescript
export interface MetadataFilter {
	readonly key: string;
	readonly value?: string | readonly string[];
	readonly mode: MetadataMatchMode;
}
```

**Step 4: Implement operators in text-search.ts**

In `src/ai/memory/text-search.ts`, add cases to `matchesMetadataFilter()` (line ~286):

```typescript
case 'gt': {
	const numVal = Number(actual);
	const numFilter = Number(filter.value);
	return !Number.isNaN(numVal) && !Number.isNaN(numFilter) && numVal > numFilter;
}
case 'gte': {
	const numVal = Number(actual);
	const numFilter = Number(filter.value);
	return !Number.isNaN(numVal) && !Number.isNaN(numFilter) && numVal >= numFilter;
}
case 'lt': {
	const numVal = Number(actual);
	const numFilter = Number(filter.value);
	return !Number.isNaN(numVal) && !Number.isNaN(numFilter) && numVal < numFilter;
}
case 'lte': {
	const numVal = Number(actual);
	const numFilter = Number(filter.value);
	return !Number.isNaN(numVal) && !Number.isNaN(numFilter) && numVal <= numFilter;
}
case 'in': {
	if (!Array.isArray(filter.value)) return false;
	return filter.value.includes(actual);
}
case 'notIn': {
	if (!Array.isArray(filter.value)) return true;
	return !filter.value.includes(actual);
}
case 'between': {
	if (!Array.isArray(filter.value) || filter.value.length !== 2) return false;
	const numVal = Number(actual);
	const min = Number(filter.value[0]);
	const max = Number(filter.value[1]);
	return !Number.isNaN(numVal) && !Number.isNaN(min) && !Number.isNaN(max) && numVal >= min && numVal <= max;
}
```

**Step 5: Run test to verify it passes**

Run: `bun test tests/metadata-operators.test.ts`
Expected: All 7 tests PASS

**Step 6: Run all existing tests to verify no regressions**

Run: `bun test`
Expected: All existing tests PASS

**Step 7: Commit**

```bash
git add src/ai/memory/types.ts src/ai/memory/text-search.ts tests/metadata-operators.test.ts
git commit -m "feat(memory): add numeric and array metadata operators (gt, gte, lt, lte, in, notIn, between)"
```

---

## Task 2: Hierarchical Topic Index

**Files:**
- Modify: `src/ai/memory/types.ts:202-206` (TopicInfo)
- Modify: `src/ai/memory/indexing.ts:14-212` (TopicIndex + createTopicIndex)

**Step 1: Write the failing test**

Create `tests/hierarchical-topics.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { createTopicIndex } from '../src/ai/memory/indexing.js';
import type { VectorEntry } from '../src/ai/memory/types.js';

function makeEntry(id: string, text: string, topics?: string[]): VectorEntry {
	return {
		id,
		text,
		embedding: [0.1, 0.2, 0.3],
		metadata: topics ? { topics: JSON.stringify(topics) } : {},
		timestamp: Date.now(),
	};
}

describe('hierarchical topic index', () => {
	it('auto-creates parent nodes', () => {
		const index = createTopicIndex();
		const entry = makeEntry('1', 'rust async', ['programming/rust/async']);
		index.addEntry(entry);

		const topics = index.getAllTopics();
		const paths = topics.map((t) => t.topic);
		expect(paths).toContain('programming');
		expect(paths).toContain('programming/rust');
		expect(paths).toContain('programming/rust/async');
	});

	it('ancestor query returns descendant entries', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'rust', ['programming/rust']));
		index.addEntry(makeEntry('2', 'python', ['programming/python']));
		index.addEntry(makeEntry('3', 'pasta', ['cooking/italian']));

		const progEntries = index.getEntries('programming');
		expect(progEntries).toContain('1');
		expect(progEntries).toContain('2');
		expect(progEntries).not.toContain('3');
	});

	it('tracks co-occurrence between topics', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'web dev', ['programming/typescript', 'programming/react']));
		index.addEntry(makeEntry('2', 'more web', ['programming/typescript', 'programming/react']));

		const related = index.getRelatedTopics('programming/typescript');
		expect(related).toContainEqual(
			expect.objectContaining({ topic: 'programming/react' }),
		);
	});

	it('merges topics', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'js stuff', ['js']));
		index.addEntry(makeEntry('2', 'javascript stuff', ['javascript']));

		index.mergeTopic('js', 'javascript');

		expect(index.getEntries('js')).toHaveLength(0);
		expect(index.getEntries('javascript')).toContain('1');
		expect(index.getEntries('javascript')).toContain('2');
	});

	it('supports multi-topic entries via metadata.topics', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'fullstack', ['frontend', 'backend']));

		expect(index.getEntries('frontend')).toContain('1');
		expect(index.getEntries('backend')).toContain('1');
	});

	it('getChildren returns direct children', () => {
		const index = createTopicIndex();
		index.addEntry(makeEntry('1', 'a', ['lang/rust']));
		index.addEntry(makeEntry('2', 'b', ['lang/python']));
		index.addEntry(makeEntry('3', 'c', ['lang/python/django']));

		const children = index.getChildren('lang');
		expect(children).toContain('lang/rust');
		expect(children).toContain('lang/python');
		expect(children).not.toContain('lang/python/django');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/hierarchical-topics.test.ts`
Expected: FAIL — `getRelatedTopics`, `mergeTopic`, `getChildren` don't exist

**Step 3: Update TopicInfo type**

In `src/ai/memory/types.ts`, add new types near line ~202:

```typescript
export interface TopicInfo {
	readonly topic: string;
	readonly entryCount: number;
	readonly entryIds: readonly string[];
	readonly parent?: string;
	readonly children: readonly string[];
}

export interface RelatedTopic {
	readonly topic: string;
	readonly coOccurrenceCount: number;
}
```

**Step 4: Rewrite TopicIndex interface and createTopicIndex**

In `src/ai/memory/indexing.ts`, update the `TopicIndex` interface (line ~25) to add:

```typescript
readonly getRelatedTopics: (topic: string) => readonly RelatedTopic[];
readonly mergeTopic: (from: string, to: string) => void;
readonly getChildren: (topic: string) => readonly string[];
```

Then rewrite `createTopicIndex` to:
- Store topics in a tree structure with parent/children links
- Auto-create parent nodes when a path like `a/b/c` is added
- `getEntries(topic)` walks all descendants and collects entry IDs
- Track co-occurrence: when an entry has multiple topics, increment pairwise counters
- Parse `metadata.topics` (JSON array) in `addEntry` alongside `metadata.topic`
- `mergeTopic(from, to)`: move all entries from `from` to `to`, update co-occurrence
- `getChildren(topic)`: return direct child paths
- Update `getAllTopics()` to include parent/children in TopicInfo
- Update `removeEntry` to decrement co-occurrence counters

**Step 5: Run test to verify it passes**

Run: `bun test tests/hierarchical-topics.test.ts`
Expected: All 6 tests PASS

**Step 6: Run all tests**

Run: `bun test`
Expected: All existing tests PASS (TopicIndex is backward compatible)

**Step 7: Commit**

```bash
git add src/ai/memory/types.ts src/ai/memory/indexing.ts tests/hierarchical-topics.test.ts
git commit -m "feat(memory): add hierarchical topic index with paths, co-occurrence, merging"
```

---

## Task 3: Inverted Text Index + BM25

**Files:**
- Create: `src/ai/memory/inverted-index.ts`
- Modify: `src/ai/memory/types.ts:50-55` (add 'bm25' to TextSearchMode)

**Step 1: Write the failing test**

Create `tests/inverted-index.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { createInvertedIndex } from '../src/ai/memory/inverted-index.js';
import type { VectorEntry } from '../src/ai/memory/types.js';

function makeEntry(id: string, text: string): VectorEntry {
	return {
		id,
		text,
		embedding: [0.1, 0.2, 0.3],
		metadata: {},
		timestamp: Date.now(),
	};
}

describe('InvertedIndex', () => {
	it('indexes terms and retrieves entry IDs', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'the quick brown fox'));
		idx.addEntry(makeEntry('2', 'the lazy brown dog'));

		expect(idx.getEntries('brown')).toContain('1');
		expect(idx.getEntries('brown')).toContain('2');
		expect(idx.getEntries('fox')).toContain('1');
		expect(idx.getEntries('fox')).not.toContain('2');
	});

	it('removes entry from index', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'hello world'));
		idx.removeEntry('1', 'hello world');
		expect(idx.getEntries('hello')).toHaveLength(0);
	});

	it('computes BM25 scores', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'rust programming language systems'));
		idx.addEntry(makeEntry('2', 'python programming language scripting'));
		idx.addEntry(makeEntry('3', 'rust rust rust systems low level'));

		const results = idx.bm25Search('rust programming');
		expect(results.length).toBeGreaterThan(0);
		// Entry 3 has more 'rust' terms, entry 1 has both terms
		const ids = results.map((r) => r.id);
		expect(ids).toContain('1');
		expect(ids).toContain('3');
	});

	it('returns empty for unknown terms', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'hello'));
		expect(idx.bm25Search('nonexistent')).toHaveLength(0);
	});

	it('clear removes all entries', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'hello world'));
		idx.clear();
		expect(idx.getEntries('hello')).toHaveLength(0);
		expect(idx.documentCount).toBe(0);
	});

	it('tracks document count and average length', () => {
		const idx = createInvertedIndex();
		idx.addEntry(makeEntry('1', 'one two three'));
		idx.addEntry(makeEntry('2', 'four five'));
		expect(idx.documentCount).toBe(2);
		expect(idx.averageDocumentLength).toBe(2.5);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/inverted-index.test.ts`
Expected: FAIL — module not found

**Step 3: Add 'bm25' to TextSearchMode**

In `src/ai/memory/types.ts` line ~50, add `'bm25'`:

```typescript
export type TextSearchMode = 'fuzzy' | 'substring' | 'exact' | 'regex' | 'token' | 'bm25';
```

**Step 4: Implement inverted-index.ts**

Create `src/ai/memory/inverted-index.ts`:

```typescript
import type { VectorEntry } from './types.js';

export interface BM25Result {
	readonly id: string;
	readonly score: number;
}

export interface BM25Options {
	readonly k1?: number;
	readonly b?: number;
}

export interface InvertedIndex {
	readonly addEntry: (entry: VectorEntry) => void;
	readonly addEntries: (entries: readonly VectorEntry[]) => void;
	readonly removeEntry: (id: string, text: string) => void;
	readonly getEntries: (term: string) => readonly string[];
	readonly bm25Search: (query: string, options?: BM25Options) => readonly BM25Result[];
	readonly clear: () => void;
	readonly documentCount: number;
	readonly averageDocumentLength: number;
}

export function tokenizeForIndex(text: string): string[] {
	return text
		.toLowerCase()
		.replace(/[^\w\s]/g, ' ')
		.split(/\s+/)
		.filter((t) => t.length > 0);
}

export function createInvertedIndex(): InvertedIndex {
	// term -> Set<entryId>
	const index = new Map<string, Set<string>>();
	// entryId -> document length (in tokens)
	const docLengths = new Map<string, number>();
	// term -> Map<entryId, termFrequency>
	const termFreqs = new Map<string, Map<string, number>>();
	let totalDocLength = 0;

	function addEntry(entry: VectorEntry): void {
		const tokens = tokenizeForIndex(entry.text);
		docLengths.set(entry.id, tokens.length);
		totalDocLength += tokens.length;

		const freqs = new Map<string, number>();
		for (const token of tokens) {
			freqs.set(token, (freqs.get(token) ?? 0) + 1);
			let set = index.get(token);
			if (!set) {
				set = new Set();
				index.set(token, set);
			}
			set.add(entry.id);
		}

		for (const [term, freq] of freqs) {
			let tfMap = termFreqs.get(term);
			if (!tfMap) {
				tfMap = new Map();
				termFreqs.set(term, tfMap);
			}
			tfMap.set(entry.id, freq);
		}
	}

	function removeEntry(id: string, text: string): void {
		const tokens = tokenizeForIndex(text);
		const len = docLengths.get(id);
		if (len !== undefined) {
			totalDocLength -= len;
			docLengths.delete(id);
		}

		for (const token of new Set(tokens)) {
			const set = index.get(token);
			if (set) {
				set.delete(id);
				if (set.size === 0) index.delete(token);
			}
			const tfMap = termFreqs.get(token);
			if (tfMap) {
				tfMap.delete(id);
				if (tfMap.size === 0) termFreqs.delete(token);
			}
		}
	}

	function bm25Search(query: string, options?: BM25Options): readonly BM25Result[] {
		const k1 = options?.k1 ?? 1.2;
		const b = options?.b ?? 0.75;
		const N = docLengths.size;
		if (N === 0) return [];

		const avgdl = totalDocLength / N;
		const queryTokens = tokenizeForIndex(query);
		if (queryTokens.length === 0) return [];

		const scores = new Map<string, number>();

		for (const term of queryTokens) {
			const postings = index.get(term);
			if (!postings) continue;

			const df = postings.size;
			const idf = Math.log((N - df + 0.5) / (df + 0.5) + 1);
			const tfMap = termFreqs.get(term);
			if (!tfMap) continue;

			for (const [docId, tf] of tfMap) {
				const dl = docLengths.get(docId) ?? 0;
				const tfNorm = (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * dl / avgdl));
				const score = idf * tfNorm;
				scores.set(docId, (scores.get(docId) ?? 0) + score);
			}
		}

		return [...scores.entries()]
			.map(([id, score]) => ({ id, score }))
			.sort((a, b) => b.score - a.score);
	}

	return Object.freeze({
		addEntry,
		addEntries: (entries: readonly VectorEntry[]) => {
			for (const e of entries) addEntry(e);
		},
		removeEntry,
		getEntries: (term: string) => [...(index.get(term.toLowerCase()) ?? [])],
		bm25Search,
		clear: () => {
			index.clear();
			docLengths.clear();
			termFreqs.clear();
			totalDocLength = 0;
		},
		get documentCount() {
			return docLengths.size;
		},
		get averageDocumentLength() {
			return docLengths.size === 0 ? 0 : totalDocLength / docLengths.size;
		},
	});
}
```

**Step 5: Run test to verify it passes**

Run: `bun test tests/inverted-index.test.ts`
Expected: All 6 tests PASS

**Step 6: Run all tests**

Run: `bun test`
Expected: All existing tests PASS

**Step 7: Commit**

```bash
git add src/ai/memory/inverted-index.ts src/ai/memory/types.ts tests/inverted-index.test.ts
git commit -m "feat(memory): add inverted text index with BM25 scoring"
```

---

## Task 4: Decompose vector-store.ts — Extract Serialization

**Files:**
- Create: `src/ai/memory/vector-serialize.ts`
- Modify: `src/ai/memory/vector-store.ts` (extract load/save/format detection)

**Step 1: Run existing tests as baseline**

Run: `bun test`
Expected: All PASS — this is our safety net

**Step 2: Extract serialization module**

Create `src/ai/memory/vector-serialize.ts` by extracting from `vector-store.ts`:
- The `loadFromStorage()` logic (lines ~506-615)
- The `saveToStorage()` logic (lines ~616-650)
- Index file parsing, format detection (v1 vs v2)
- Entry deserialization (IndexEntry -> VectorEntry + embedding decode)
- Entry serialization (VectorEntry -> IndexEntry + embedding encode)

The module should export:

```typescript
export interface SerializationContext {
	readonly entries: Map<string, VectorEntry>;
	readonly embeddings: Map<string, readonly number[]>;
	readonly accessStats: Map<string, { accessCount: number; lastAccessed: number }>;
}

export interface SerializeResult {
	readonly data: Map<string, Buffer>;
}

export function serializeEntries(ctx: SerializationContext, learningState?: LearningState): SerializeResult;
export function deserializeEntries(data: Map<string, Buffer>, logger?: Logger): SerializationContext & { learningState?: LearningState };
```

**Step 3: Update vector-store.ts to use the extracted module**

Replace the inline load/save code with imports from `vector-serialize.ts`. The `load()` and `save()` methods in vector-store become thin wrappers.

**Step 4: Run all tests to verify no regressions**

Run: `bun test`
Expected: All existing tests PASS

**Step 5: Commit**

```bash
git add src/ai/memory/vector-serialize.ts src/ai/memory/vector-store.ts
git commit -m "refactor(memory): extract serialization into vector-serialize.ts"
```

---

## Task 5: Decompose vector-store.ts — Extract Search

**Files:**
- Create: `src/ai/memory/vector-search.ts`
- Modify: `src/ai/memory/vector-store.ts` (extract search/advancedSearch/textSearch)

**Step 1: Extract search module**

Create `src/ai/memory/vector-search.ts` by extracting:
- `search()` method (lines ~910-951)
- `textSearch()` method (lines ~957-994)
- `advancedSearch()` method (lines ~1052-1159)
- `filterByMetadata()` method (lines ~1000-1032)
- `filterByDateRange()` method (lines ~1038-1046)

The module should export functions that take the entries map, indexes, and options as parameters:

```typescript
export function vectorSearch(
	entries: ReadonlyMap<string, VectorEntry>,
	embeddings: ReadonlyMap<string, readonly number[]>,
	queryEmbedding: readonly number[],
	maxResults: number,
	threshold: number,
	magnitudeCache: MagnitudeCache,
): SearchResult[];

export function advancedVectorSearch(
	entries: ReadonlyMap<string, VectorEntry>,
	embeddings: ReadonlyMap<string, readonly number[]>,
	options: SearchOptions,
	magnitudeCache: MagnitudeCache,
	metadataIndex: MetadataIndex,
	invertedIndex?: InvertedIndex,
): AdvancedSearchResult[];
```

**Step 2: Integrate inverted index into advancedSearch**

When `options.textSearch?.mode === 'bm25'`, use the inverted index for O(terms) lookup instead of linear scan. Fall back to linear scan for other text modes.

**Step 3: Update vector-store.ts**

Replace inline search methods with calls to the extracted module. Wire up the inverted index: build it on `load()`, update on `add()`/`delete()`.

**Step 4: Run all tests**

Run: `bun test`
Expected: All PASS

**Step 5: Write BM25 integration test**

Add to `tests/inverted-index.test.ts`:

```typescript
describe('BM25 via advancedSearch', () => {
	it('uses bm25 mode in text search', async () => {
		// Create vector store with inverted index, add entries, search with mode: 'bm25'
		// Verify results are ranked by BM25 score
	});
});
```

**Step 6: Run tests, commit**

```bash
git add src/ai/memory/vector-search.ts src/ai/memory/vector-store.ts tests/inverted-index.test.ts
git commit -m "refactor(memory): extract search into vector-search.ts, integrate BM25"
```

---

## Task 6: Decompose vector-store.ts — Extract Recommendations

**Files:**
- Create: `src/ai/memory/vector-recommend.ts`
- Modify: `src/ai/memory/vector-store.ts` (extract recommend method)

**Step 1: Extract recommendation logic**

Create `src/ai/memory/vector-recommend.ts` extracting:
- `recommend()` method (lines ~1225-1336)
- Access stat tracking helpers
- Learning engine integration (adapted weights, boost computation)

Export:

```typescript
export function computeRecommendations(
	entries: ReadonlyMap<string, VectorEntry>,
	embeddings: ReadonlyMap<string, readonly number[]>,
	accessStats: ReadonlyMap<string, { accessCount: number; lastAccessed: number }>,
	options: RecommendOptions,
	magnitudeCache: MagnitudeCache,
	topicIndex: TopicIndex,
	metadataIndex: MetadataIndex,
	learningEngine?: LearningEngine,
	recencyOptions?: RecencyOptions,
): RecommendationResult[];
```

**Step 2: Update vector-store.ts to use extracted module**

**Step 3: Run all tests**

Run: `bun test`
Expected: All PASS

**Step 4: Commit**

```bash
git add src/ai/memory/vector-recommend.ts src/ai/memory/vector-store.ts
git commit -m "refactor(memory): extract recommendations into vector-recommend.ts"
```

---

## Task 7: Explicit Relevance Feedback in Learning Engine

**Files:**
- Modify: `src/ai/memory/learning.ts:37-68` (LearningEngine interface)
- Modify: `src/ai/memory/learning.ts:84-487` (createLearningEngine)
- Modify: `src/ai/memory/vector-persistence.ts:53-65` (LearningState)
- Modify: `src/ai/memory/memory.ts:54-96` (MemoryManager interface)

**Step 1: Write the failing test**

Create `tests/explicit-feedback.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { createLearningEngine } from '../src/ai/memory/learning.js';

describe('explicit relevance feedback', () => {
	it('positive feedback boosts relevance score', () => {
		const engine = createLearningEngine({ enabled: true });
		const queryEmb = [1, 0, 0];
		engine.recordQuery(queryEmb, ['entry-1']);

		const before = engine.getRelevanceFeedback('entry-1');
		engine.recordFeedback('entry-1', true);
		const after = engine.getRelevanceFeedback('entry-1');

		expect(after!.relevanceScore).toBeGreaterThan(before!.relevanceScore);
	});

	it('negative feedback reduces relevance score', () => {
		const engine = createLearningEngine({ enabled: true });
		const queryEmb = [1, 0, 0];
		engine.recordQuery(queryEmb, ['entry-1']);

		engine.recordFeedback('entry-1', true);
		const before = engine.getRelevanceFeedback('entry-1');
		engine.recordFeedback('entry-1', false);
		const after = engine.getRelevanceFeedback('entry-1');

		expect(after!.relevanceScore).toBeLessThan(before!.relevanceScore);
	});

	it('feedback persists through serialize/restore', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1']);
		engine.recordFeedback('entry-1', true);

		const state = engine.serialize();
		const engine2 = createLearningEngine({ enabled: true });
		engine2.restore(state);

		const feedback = engine2.getRelevanceFeedback('entry-1');
		expect(feedback).toBeDefined();
		expect(feedback!.relevanceScore).toBeGreaterThan(0);
	});

	it('explicit feedback weighted stronger than implicit', () => {
		const engine1 = createLearningEngine({ enabled: true });
		// Implicit only: 5 retrievals
		for (let i = 0; i < 5; i++) {
			engine1.recordQuery([1, 0, 0], ['entry-1']);
		}
		const implicitScore = engine1.getRelevanceFeedback('entry-1')!.relevanceScore;

		const engine2 = createLearningEngine({ enabled: true });
		engine2.recordQuery([1, 0, 0], ['entry-1']);
		engine2.recordFeedback('entry-1', true);
		const explicitScore = engine2.getRelevanceFeedback('entry-1')!.relevanceScore;

		expect(explicitScore).toBeGreaterThan(implicitScore);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/explicit-feedback.test.ts`
Expected: FAIL — `recordFeedback` not found on LearningEngine

**Step 3: Add recordFeedback to LearningEngine interface**

In `src/ai/memory/learning.ts` line ~37, add to the interface:

```typescript
readonly recordFeedback: (entryId: string, relevant: boolean) => void;
```

**Step 4: Implement recordFeedback**

In `createLearningEngine()`, add:
- Track explicit feedback per entry (positive count, negative count)
- Weight explicit feedback 5x in relevance score computation
- `relevanceScore` formula: `(queryCount + positiveFeedback * 5 - negativeFeedback * 5) / (maxQueryHistory + maxFeedbackCount)`
- Clamp score to `[0, 1]`
- Include feedback in `serialize()` / `restore()`

**Step 5: Update LearningState in vector-persistence.ts**

Add `explicitFeedback` field (optional, backward compatible):

```typescript
export interface LearningState {
	// ... existing fields
	readonly explicitFeedback?: ReadonlyArray<{
		readonly entryId: string;
		readonly positiveCount: number;
		readonly negativeCount: number;
	}>;
}
```

Bump version to 2. Update `isValidLearningState` guard to accept both v1 and v2.

**Step 6: Add recordFeedback to MemoryManager**

In `src/ai/memory/memory.ts`, expose via the MemoryManager interface:

```typescript
readonly recordFeedback: (entryId: string, relevant: boolean) => void;
```

Delegates to `store.learningEngine.recordFeedback()`.

**Step 7: Run tests**

Run: `bun test`
Expected: All PASS

**Step 8: Commit**

```bash
git add src/ai/memory/learning.ts src/ai/memory/vector-persistence.ts src/ai/memory/memory.ts tests/explicit-feedback.test.ts
git commit -m "feat(memory): add explicit relevance feedback to learning engine"
```

---

## Task 8: Per-Topic Weight Profiles

**Files:**
- Modify: `src/ai/memory/learning.ts` (per-topic weights)
- Modify: `src/ai/memory/vector-persistence.ts` (serialize per-topic state)

**Step 1: Write the failing test**

Create `tests/per-topic-learning.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { createLearningEngine } from '../src/ai/memory/learning.js';

describe('per-topic weight profiles', () => {
	it('adapts weights per topic independently', () => {
		const engine = createLearningEngine({ enabled: true });

		// Simulate many queries in 'news' topic with high-access results
		for (let i = 0; i < 15; i++) {
			engine.recordQuery([1, 0, 0], ['news-entry'], { topic: 'news' });
		}

		// Simulate many queries in 'code' topic with low-access results
		for (let i = 0; i < 15; i++) {
			engine.recordQuery([0, 1, 0], ['code-entry'], { topic: 'code' });
		}

		const newsWeights = engine.getAdaptedWeights('news');
		const codeWeights = engine.getAdaptedWeights('code');
		const globalWeights = engine.getAdaptedWeights();

		// They should not all be identical
		expect(newsWeights).not.toEqual(codeWeights);
	});

	it('falls back to global weights for topics with < 10 queries', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['entry-1'], { topic: 'rare' });

		const rareWeights = engine.getAdaptedWeights('rare');
		const globalWeights = engine.getAdaptedWeights();

		expect(rareWeights).toEqual(globalWeights);
	});

	it('per-topic interest embeddings are separate', () => {
		const engine = createLearningEngine({ enabled: true });

		for (let i = 0; i < 5; i++) {
			engine.recordQuery([1, 0, 0], ['a'], { topic: 'physics' });
			engine.recordQuery([0, 1, 0], ['b'], { topic: 'cooking' });
		}

		const physicsInterest = engine.getInterestEmbedding('physics');
		const cookingInterest = engine.getInterestEmbedding('cooking');

		expect(physicsInterest).toBeDefined();
		expect(cookingInterest).toBeDefined();
		// They should point in different directions
		expect(physicsInterest![0]).not.toBeCloseTo(cookingInterest![0], 1);
	});

	it('serializes and restores per-topic state', () => {
		const engine = createLearningEngine({ enabled: true });
		for (let i = 0; i < 15; i++) {
			engine.recordQuery([1, 0, 0], ['a'], { topic: 'math' });
		}

		const state = engine.serialize();
		const engine2 = createLearningEngine({ enabled: true });
		engine2.restore(state);

		const weights = engine2.getAdaptedWeights('math');
		expect(weights).toEqual(engine.getAdaptedWeights('math'));
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/per-topic-learning.test.ts`
Expected: FAIL — `recordQuery` doesn't accept topic option, `getAdaptedWeights` doesn't accept topic

**Step 3: Implement per-topic tracking**

In `src/ai/memory/learning.ts`:
- Add `topic?: string` parameter to `recordQuery` signature (as optional options object)
- Maintain per-topic state maps: `topicWeights: Map<string, weights>`, `topicInterest: Map<string, number[]>`, `topicQueryCount: Map<string, number>`
- `getAdaptedWeights(topic?)`: if topic provided and >= 10 queries, return topic weights; else global
- `getInterestEmbedding(topic?)`: if topic provided, return topic interest; else global
- Update `serialize()` / `restore()` to include per-topic maps
- Update `computeBoost()` to accept optional topic for per-topic boosting

**Step 4: Run tests**

Run: `bun test`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/ai/memory/learning.ts src/ai/memory/vector-persistence.ts tests/per-topic-learning.test.ts
git commit -m "feat(memory): add per-topic weight profiles and interest embeddings"
```

---

## Task 9: Query DSL

**Files:**
- Create: `src/ai/memory/query-dsl.ts`
- Modify: `src/ai/memory/memory.ts` (add query method)

**Step 1: Write the failing test**

Create `tests/query-dsl.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
import { parseQuery } from '../src/ai/memory/query-dsl.js';

describe('query DSL parser', () => {
	it('parses plain text as text query', () => {
		const opts = parseQuery('hello world');
		expect(opts.textSearch?.query).toBe('hello world');
	});

	it('parses topic: prefix', () => {
		const opts = parseQuery('topic:programming/rust some query');
		expect(opts.topicFilter).toContain('programming/rust');
		expect(opts.textSearch?.query).toBe('some query');
	});

	it('parses metadata: prefix', () => {
		const opts = parseQuery('metadata:lang=rust hello');
		expect(opts.metadataFilters).toContainEqual(
			expect.objectContaining({ key: 'lang', value: 'rust', mode: 'eq' }),
		);
		expect(opts.textSearch?.query).toBe('hello');
	});

	it('parses quoted exact phrases', () => {
		const opts = parseQuery('"exact match"');
		expect(opts.textSearch?.query).toBe('exact match');
		expect(opts.textSearch?.mode).toBe('exact');
	});

	it('parses fuzzy~ prefix', () => {
		const opts = parseQuery('fuzzy~approx');
		expect(opts.textSearch?.query).toBe('approx');
		expect(opts.textSearch?.mode).toBe('fuzzy');
	});

	it('parses score> numeric filter', () => {
		const opts = parseQuery('score>0.5 some text');
		expect(opts.minScore).toBe(0.5);
		expect(opts.textSearch?.query).toBe('some text');
	});

	it('combines multiple DSL elements', () => {
		const opts = parseQuery('topic:lang/rust metadata:type=tutorial "async programming"');
		expect(opts.topicFilter).toContain('lang/rust');
		expect(opts.metadataFilters).toHaveLength(1);
		expect(opts.textSearch?.query).toBe('async programming');
		expect(opts.textSearch?.mode).toBe('exact');
	});

	it('handles empty query', () => {
		const opts = parseQuery('');
		expect(opts.textSearch?.query).toBe('');
	});
});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/query-dsl.test.ts`
Expected: FAIL — module not found

**Step 3: Implement query-dsl.ts**

Create `src/ai/memory/query-dsl.ts`:

```typescript
import type { MetadataFilter, SearchOptions, TextSearchMode } from './types.js';

export interface ParsedQuery {
	readonly textSearch?: {
		readonly query: string;
		readonly mode: TextSearchMode;
	};
	readonly topicFilter?: readonly string[];
	readonly metadataFilters?: readonly MetadataFilter[];
	readonly minScore?: number;
}

export function parseQuery(dsl: string): ParsedQuery {
	const topics: string[] = [];
	const metadataFilters: MetadataFilter[] = [];
	let minScore: number | undefined;
	let textMode: TextSearchMode = 'bm25';
	const textParts: string[] = [];

	const tokens = tokenizeDSL(dsl);

	for (const token of tokens) {
		if (token.startsWith('topic:')) {
			topics.push(token.slice(6));
		} else if (token.startsWith('metadata:')) {
			const kv = token.slice(9);
			const eqIdx = kv.indexOf('=');
			if (eqIdx > 0) {
				metadataFilters.push({
					key: kv.slice(0, eqIdx),
					value: kv.slice(eqIdx + 1),
					mode: 'eq',
				});
			}
		} else if (token.startsWith('score>')) {
			const val = Number(token.slice(6));
			if (!Number.isNaN(val)) minScore = val;
		} else if (token.startsWith('fuzzy~')) {
			textParts.push(token.slice(6));
			textMode = 'fuzzy';
		} else if (token.startsWith('"') && token.endsWith('"')) {
			textParts.push(token.slice(1, -1));
			textMode = 'exact';
		} else {
			textParts.push(token);
		}
	}

	return Object.freeze({
		textSearch: { query: textParts.join(' '), mode: textMode },
		topicFilter: topics.length > 0 ? topics : undefined,
		metadataFilters: metadataFilters.length > 0 ? metadataFilters : undefined,
		minScore,
	});
}

function tokenizeDSL(dsl: string): string[] {
	const tokens: string[] = [];
	let i = 0;
	while (i < dsl.length) {
		// Skip whitespace
		while (i < dsl.length && dsl[i] === ' ') i++;
		if (i >= dsl.length) break;

		if (dsl[i] === '"') {
			// Quoted string
			const end = dsl.indexOf('"', i + 1);
			if (end === -1) {
				tokens.push(dsl.slice(i));
				break;
			}
			tokens.push(dsl.slice(i, end + 1));
			i = end + 1;
		} else {
			// Regular token
			const end = dsl.indexOf(' ', i);
			if (end === -1) {
				tokens.push(dsl.slice(i));
				break;
			}
			tokens.push(dsl.slice(i, end));
			i = end;
		}
	}
	return tokens;
}
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/query-dsl.test.ts`
Expected: All 8 tests PASS

**Step 5: Add query() method to MemoryManager**

In `src/ai/memory/memory.ts`, add:

```typescript
readonly query: (dsl: string) => Promise<AdvancedSearchResult[]>;
```

Implementation: call `parseQuery()`, convert `ParsedQuery` to `SearchOptions`, delegate to `advancedSearch()`.

**Step 6: Run all tests**

Run: `bun test`
Expected: All PASS

**Step 7: Commit**

```bash
git add src/ai/memory/query-dsl.ts src/ai/memory/memory.ts tests/query-dsl.test.ts
git commit -m "feat(memory): add query DSL parser with topic, metadata, exact, fuzzy, and score filters"
```

---

## Task 10: Field Boosting and Weighted Ranking

**Files:**
- Modify: `src/ai/memory/types.ts:127-168` (SearchOptions)
- Modify: `src/ai/memory/vector-search.ts` (or vector-store.ts if not yet extracted)

**Step 1: Write the failing test**

Add to `tests/metadata-operators.test.ts` or create `tests/field-boosting.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
// ... setup with vector store

describe('field boosting', () => {
	it('boosts entries matching topic filter', () => {
		// Two entries with same vector similarity
		// One matches topic filter, one doesn't
		// The topic-matching entry should score higher with fieldBoosts.topic > 1
	});
});

describe('weighted ranking mode', () => {
	it('uses custom weights for score combination', () => {
		// Search with rankBy: 'weighted', rankWeights: { vector: 0.3, text: 0.7 }
		// Verify that text score has more influence than vector score
	});
});
```

**Step 2: Update SearchOptions type**

In `src/ai/memory/types.ts`, add to `SearchOptions`:

```typescript
readonly fieldBoosts?: {
	readonly text?: number;
	readonly metadata?: number;
	readonly topic?: number;
};
readonly rankWeights?: {
	readonly vector?: number;
	readonly text?: number;
	readonly metadata?: number;
	readonly recency?: number;
};
```

Add `'weighted'` to the `rankBy` union type.

**Step 3: Implement in advancedSearch**

In the search module, when `rankBy === 'weighted'`:
- Use `rankWeights` for custom score combination
- Apply `fieldBoosts` as multipliers on component scores

**Step 4: Run tests, commit**

```bash
git add src/ai/memory/types.ts src/ai/memory/vector-search.ts tests/field-boosting.test.ts
git commit -m "feat(memory): add field boosting and weighted ranking mode"
```

---

## Task 11: Query-Result Correlation in Learning Engine

**Files:**
- Modify: `src/ai/memory/learning.ts`

**Step 1: Write the failing test**

Add to `tests/per-topic-learning.test.ts`:

```typescript
describe('query-result correlation', () => {
	it('tracks co-appearing entries across queries', () => {
		const engine = createLearningEngine({ enabled: true });
		engine.recordQuery([1, 0, 0], ['a', 'b']);
		engine.recordQuery([0, 1, 0], ['a', 'b']);
		engine.recordQuery([0, 0, 1], ['a', 'c']);

		const correlated = engine.getCorrelatedEntries('a');
		// 'b' appeared with 'a' in 2 queries, 'c' in 1
		expect(correlated).toContainEqual(expect.objectContaining({ entryId: 'b' }));
		const bCorr = correlated.find((c) => c.entryId === 'b');
		const cCorr = correlated.find((c) => c.entryId === 'c');
		expect(bCorr!.strength).toBeGreaterThan(cCorr!.strength);
	});
});
```

**Step 2: Implement correlation tracking**

In `createLearningEngine()`:
- Add `correlations: Map<string, Map<string, number>>` — pairwise co-occurrence counts
- In `recordQuery()`, for each pair of result IDs, increment correlation count
- Add `getCorrelatedEntries(entryId)` → sorted by strength
- Include in `serialize()` / `restore()`

**Step 3: Run tests, commit**

```bash
git add src/ai/memory/learning.ts tests/per-topic-learning.test.ts
git commit -m "feat(memory): add query-result correlation tracking"
```

---

## Task 12: Export New Types and Update lib.ts

**Files:**
- Modify: `src/lib.ts:150-194`

**Step 1: Add exports**

Export all new types and functions from `src/lib.ts`:

```typescript
export { createInvertedIndex } from './ai/memory/inverted-index.js';
export type { InvertedIndex, BM25Result, BM25Options } from './ai/memory/inverted-index.js';
export { parseQuery } from './ai/memory/query-dsl.js';
export type { ParsedQuery } from './ai/memory/query-dsl.js';
export type { RelatedTopic } from './ai/memory/types.js';
```

**Step 2: Typecheck**

Run: `bun x tsc --noEmit`
Expected: No errors

**Step 3: Lint**

Run: `bun run lint:fix`
Expected: Clean

**Step 4: Run all tests**

Run: `bun test`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/lib.ts
git commit -m "feat: export new memory types and functions from public API"
```

---

## Task 13: Final Integration Test

**Files:**
- Create: `tests/hierarchical-memory-integration.test.ts`

**Step 1: Write integration test**

```typescript
import { describe, expect, it } from 'bun:test';
// Full integration test exercising:
// 1. Create memory manager with learning enabled
// 2. Add entries with hierarchical topics
// 3. Search with BM25 mode
// 4. Use query DSL
// 5. Record explicit feedback
// 6. Verify per-topic weights adapt
// 7. Test metadata with new operators
// 8. Verify recommendations use adapted weights
// 9. Save and reload — verify persistence
```

**Step 2: Run tests**

Run: `bun test`
Expected: All PASS

**Step 3: Final typecheck and lint**

Run: `bun x tsc --noEmit && bun run lint:fix`
Expected: Clean

**Step 4: Commit**

```bash
git add tests/hierarchical-memory-integration.test.ts
git commit -m "test(memory): add integration test for hierarchical memory system"
```

---

## Summary

| Task | Description | Estimated Complexity |
|------|-------------|---------------------|
| 1 | New metadata operators | Small |
| 2 | Hierarchical topic index | Medium |
| 3 | Inverted text index + BM25 | Medium |
| 4 | Extract serialization from vector-store | Medium |
| 5 | Extract search from vector-store | Medium |
| 6 | Extract recommendations from vector-store | Small |
| 7 | Explicit relevance feedback | Medium |
| 8 | Per-topic weight profiles | Medium |
| 9 | Query DSL | Small |
| 10 | Field boosting + weighted ranking | Small |
| 11 | Query-result correlation | Small |
| 12 | Export new types in lib.ts | Small |
| 13 | Integration test | Small |

**Dependencies:** Task 4-6 (file breakup) should happen before Task 5's BM25 integration. Task 2 (topics) should precede Task 8 (per-topic learning). All other tasks are independent.
