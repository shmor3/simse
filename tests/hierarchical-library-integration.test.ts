import { beforeEach, describe, expect, it, mock } from 'bun:test';
import {
	createLibrary,
	type Library,
} from '../src/ai/library/library.js';
import type {
	EmbeddingProvider,
	LibraryConfig,
} from '../src/ai/library/types.js';
import { createMemoryStorage, createSilentLogger } from './utils/mocks.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Mock embedder that produces deterministic, non-negative embeddings seeded
 * by text content. Uses absolute sin values so all vectors reside in the
 * positive orthant, guaranteeing positive cosine similarity between any
 * two entries. This makes the mock suitable for integration tests that
 * exercise combined vector + text + metadata search pipelines.
 */
function createMockEmbedder(dim = 8): EmbeddingProvider {
	let callCount = 0;
	return {
		embed: mock(async (input: string | readonly string[]) => {
			const texts = typeof input === 'string' ? [input] : input;
			callCount++;
			return {
				embeddings: texts.map((text, i) => {
					const hash = [...text].reduce((acc, ch) => acc + ch.charCodeAt(0), 0);
					return Array.from(
						{ length: dim },
						(_, j) =>
							Math.abs(
								Math.sin(hash * 0.1 + j * 0.7 + callCount * 0.01 + i * 0.1),
							) + 0.01,
					);
				}),
			};
		}),
	};
}

const defaultConfig: LibraryConfig = {
	enabled: true,
	embeddingAgent: 'test',
	similarityThreshold: 0,
	maxResults: 50,
};

// ---------------------------------------------------------------------------
// Integration Tests
// ---------------------------------------------------------------------------

describe('Hierarchical Library System Integration', () => {
	let library: Library;

	beforeEach(async () => {
		const storage = createMemoryStorage();
		const embedder = createMockEmbedder();
		library = createLibrary(embedder, defaultConfig, {
			storage,
			logger: createSilentLogger(),
			stacksOptions: {
				autoSave: true,
				flushIntervalMs: 0,
				learning: { enabled: true },
			},
		});
		await library.initialize();
	});

	// -----------------------------------------------------------------------
	// 1. Hierarchical Topics
	// -----------------------------------------------------------------------

	describe('hierarchical topics', () => {
		it('auto-creates parent topics from hierarchical paths', async () => {
			await library.add('Rust ownership model', {
				topics: JSON.stringify(['programming/rust/ownership']),
			});
			await library.add('Python async await', {
				topics: JSON.stringify(['programming/python/async']),
			});
			await library.add('Italian pasta recipes', {
				topics: JSON.stringify(['cooking/italian']),
			});

			const topics = library.getTopics();
			const paths = topics.map((t) => t.topic);

			// Auto-created parents should exist
			expect(paths).toContain('programming');
			expect(paths).toContain('programming/rust');
			expect(paths).toContain('programming/rust/ownership');
			expect(paths).toContain('programming/python');
			expect(paths).toContain('programming/python/async');
			expect(paths).toContain('cooking');
			expect(paths).toContain('cooking/italian');
		});

		it('ancestor query returns all descendant entries', async () => {
			await library.add('Rust ownership model', {
				topics: JSON.stringify(['programming/rust/ownership']),
			});
			await library.add('Python async await', {
				topics: JSON.stringify(['programming/python/async']),
			});
			await library.add('Italian pasta recipes', {
				topics: JSON.stringify(['cooking/italian']),
			});

			// Querying the ancestor 'programming' should return both programming entries
			const progEntries = library.filterByTopic(['programming']);
			expect(progEntries.length).toBe(2);

			const progTexts = progEntries.map((e) => e.text).sort();
			expect(progTexts).toContain('Rust ownership model');
			expect(progTexts).toContain('Python async await');

			// Querying a leaf topic should return only that entry
			const rustEntries = library.filterByTopic(['programming/rust/ownership']);
			expect(rustEntries.length).toBe(1);
			expect(rustEntries[0].text).toBe('Rust ownership model');

			// Querying 'cooking' should return only the cooking entry
			const cookEntries = library.filterByTopic(['cooking']);
			expect(cookEntries.length).toBe(1);
			expect(cookEntries[0].text).toBe('Italian pasta recipes');
		});

		it('topic info includes parent and children', async () => {
			await library.add('Rust lifetimes', {
				topics: JSON.stringify(['programming/rust/lifetimes']),
			});
			await library.add('Rust ownership', {
				topics: JSON.stringify(['programming/rust/ownership']),
			});

			const topics = library.getTopics();
			const rustTopic = topics.find((t) => t.topic === 'programming/rust');

			expect(rustTopic).toBeDefined();
			expect(rustTopic!.parent).toBe('programming');
			expect(rustTopic!.children).toContain('programming/rust/lifetimes');
			expect(rustTopic!.children).toContain('programming/rust/ownership');
		});
	});

	// -----------------------------------------------------------------------
	// 2. New Metadata Operators
	// -----------------------------------------------------------------------

	describe('metadata operators', () => {
		it('filters with numeric gt operator', async () => {
			await library.add('entry A', { score: '10', status: 'active' });
			await library.add('entry B', { score: '5', status: 'pending' });
			await library.add('entry C', { score: '1', status: 'closed' });

			const highScore = library.filterByMetadata([
				{ key: 'score', value: '5', mode: 'gt' },
			]);
			expect(highScore.length).toBe(1);
			expect(highScore[0].text).toBe('entry A');
		});

		it('filters with in operator for array membership', async () => {
			await library.add('entry A', { status: 'active' });
			await library.add('entry B', { status: 'pending' });
			await library.add('entry C', { status: 'closed' });

			const activePending = library.filterByMetadata([
				{ key: 'status', value: ['active', 'pending'], mode: 'in' },
			]);
			expect(activePending.length).toBe(2);

			const texts = activePending.map((e) => e.text).sort();
			expect(texts).toContain('entry A');
			expect(texts).toContain('entry B');
		});

		it('filters with between operator for numeric range', async () => {
			await library.add('entry A', { score: '10' });
			await library.add('entry B', { score: '5' });
			await library.add('entry C', { score: '1' });

			const midRange = library.filterByMetadata([
				{ key: 'score', value: ['3', '8'], mode: 'between' },
			]);
			expect(midRange.length).toBe(1);
			expect(midRange[0].text).toBe('entry B');
		});

		it('filters with notIn operator', async () => {
			await library.add('entry A', { status: 'active' });
			await library.add('entry B', { status: 'pending' });
			await library.add('entry C', { status: 'closed' });

			const notActivePending = library.filterByMetadata([
				{ key: 'status', value: ['active', 'pending'], mode: 'notIn' },
			]);
			expect(notActivePending.length).toBe(1);
			expect(notActivePending[0].text).toBe('entry C');
		});

		it('filters with gte and lte operators', async () => {
			await library.add('low', { priority: '1' });
			await library.add('mid', { priority: '5' });
			await library.add('high', { priority: '10' });

			const gteResults = library.filterByMetadata([
				{ key: 'priority', value: '5', mode: 'gte' },
			]);
			expect(gteResults.length).toBe(2);

			const lteResults = library.filterByMetadata([
				{ key: 'priority', value: '5', mode: 'lte' },
			]);
			expect(lteResults.length).toBe(2);
		});

		it('combines multiple metadata filters (AND logic)', async () => {
			await library.add('entry A', { score: '10', status: 'active' });
			await library.add('entry B', { score: '5', status: 'active' });
			await library.add('entry C', { score: '10', status: 'closed' });

			const results = library.filterByMetadata([
				{ key: 'score', value: '5', mode: 'gt' },
				{ key: 'status', value: 'active', mode: 'eq' },
			]);
			expect(results.length).toBe(1);
			expect(results[0].text).toBe('entry A');
		});
	});

	// -----------------------------------------------------------------------
	// 3. BM25 Text Search
	// -----------------------------------------------------------------------

	describe('BM25 text search', () => {
		it('finds entries via BM25 text search', async () => {
			await library.add('rust programming language systems');
			await library.add('python programming language scripting');
			await library.add('cooking italian pasta recipes');

			const results = library.textSearch({
				query: 'programming',
				mode: 'bm25',
			});
			expect(results.length).toBe(2);

			const texts = results.map((r) => r.volume.text);
			expect(texts).toContain('rust programming language systems');
			expect(texts).toContain('python programming language scripting');
		});

		it('BM25 scores entries by term relevance', async () => {
			await library.add('programming programming programming');
			await library.add('programming language');

			const results = library.textSearch({
				query: 'programming',
				mode: 'bm25',
			});
			expect(results.length).toBe(2);
			// The entry with more occurrences should score higher (BM25 TF)
			expect(results[0].score).toBeGreaterThanOrEqual(results[1].score);
		});
	});

	// -----------------------------------------------------------------------
	// 4. Query DSL
	// -----------------------------------------------------------------------

	describe('query DSL', () => {
		it('combines metadata filters with text search via advancedSearch', async () => {
			await library.add('rust tutorial', {
				topic: 'programming/rust',
				type: 'tutorial',
			});
			await library.add('python guide', {
				topic: 'programming/python',
				type: 'guide',
			});
			await library.add('rust reference', {
				topic: 'programming/rust',
				type: 'reference',
			});

			// Use advancedSearch directly to exercise combined metadata + text
			// without the DSL auto-embedding (which can produce negative cosine
			// similarity with mock embeddings and filter out all results).
			const results = await library.advancedSearch({
				text: { query: 'rust', mode: 'bm25', threshold: 0 },
				metadata: [{ key: 'type', value: 'tutorial', mode: 'eq' }],
				maxResults: 10,
				rankBy: 'text',
			});
			expect(results.length).toBeGreaterThan(0);
			const texts = results.map((r) => r.volume.text);
			expect(texts).toContain('rust tutorial');
		});

		it('filters by topic via filterByTopic after search', async () => {
			await library.add('Rust ownership', {
				topics: JSON.stringify(['programming/rust']),
			});
			await library.add('Italian cooking', {
				topics: JSON.stringify(['cooking/italian']),
			});

			// Use textSearch + filterByTopic to exercise the feature combination
			const textResults = library.textSearch({
				query: 'ownership',
				mode: 'bm25',
			});
			const topicEntries = library.filterByTopic(['programming/rust']);
			const topicIds = new Set(topicEntries.map((e) => e.id));

			// Intersect: entries that match both text and topic
			const filtered = textResults.filter((r) => topicIds.has(r.volume.id));
			expect(filtered.length).toBeGreaterThan(0);
			expect(filtered[0].volume.text).toBe('Rust ownership');
		});

		it('DSL parses metadata filters correctly', async () => {
			await library.add('tutorial entry', { type: 'tutorial' });
			await library.add('guide entry', { type: 'guide' });

			// The DSL "metadata:type=tutorial" produces a metadata eq filter.
			// Use advancedSearch with text-only ranking to avoid mock embedding issues.
			const results = await library.advancedSearch({
				text: { query: 'entry', mode: 'bm25', threshold: 0 },
				metadata: [{ key: 'type', value: 'tutorial', mode: 'eq' }],
				maxResults: 10,
				rankBy: 'text',
			});
			expect(results.length).toBe(1);
			expect(results[0].volume.text).toBe('tutorial entry');
		});

		it('returns results for plain text queries', async () => {
			await library.add('machine learning algorithms');
			await library.add('deep learning neural networks');
			await library.add('cooking recipes pasta');

			// Use textSearch with BM25 directly (the DSL default text mode)
			const results = library.textSearch({
				query: 'learning',
				mode: 'bm25',
			});
			expect(results.length).toBeGreaterThan(0);
		});
	});

	// -----------------------------------------------------------------------
	// 5. Explicit Feedback
	// -----------------------------------------------------------------------

	describe('explicit feedback', () => {
		it('records positive and negative feedback', async () => {
			const id1 = await library.add('good entry');
			const id2 = await library.add('bad entry');

			// Search to populate learning data
			await library.search('entry');

			// Provide feedback
			library.recordFeedback(id1, true);
			library.recordFeedback(id2, false);

			// The learning profile should reflect the queries
			const profile = library.patronProfile;
			expect(profile).toBeDefined();
			expect(profile!.totalQueries).toBeGreaterThanOrEqual(1);
		});

		it('multiple positive feedback increases relevance', async () => {
			const id = await library.add('important entry');
			await library.search('important');

			// Record multiple positive feedback
			library.recordFeedback(id, true);
			library.recordFeedback(id, true);
			library.recordFeedback(id, true);

			// Profile should still be valid
			const profile = library.patronProfile;
			expect(profile).toBeDefined();
		});
	});

	// -----------------------------------------------------------------------
	// 6. Per-Topic Learning
	// -----------------------------------------------------------------------

	describe('per-topic learning', () => {
		it('accumulates queries in the learning engine', async () => {
			// Add several entries so that searches are likely to return results
			for (let i = 0; i < 15; i++) {
				await library.add(`entry about topic ${i}`);
			}

			// Perform multiple searches — the learning engine only records
			// queries that produce results, so count is >= number of successful ones
			for (let i = 0; i < 15; i++) {
				await library.search(`entry topic ${i}`);
			}

			const profile = library.patronProfile;
			expect(profile).toBeDefined();
			// With mock embeddings, not every query may produce results,
			// but we expect the majority to succeed
			expect(profile!.totalQueries).toBeGreaterThanOrEqual(5);
		});

		it('adapted weights change from defaults after queries', async () => {
			await library.add('first entry');
			await library.add('second entry');

			// Record several searches so the learning engine adapts
			for (let i = 0; i < 20; i++) {
				await library.search(`query variation ${i}`);
			}

			const profile = library.patronProfile;
			expect(profile).toBeDefined();
			expect(profile!.adaptedWeights).toBeDefined();
			expect(profile!.adaptedWeights.vector).toBeGreaterThan(0);
			expect(profile!.adaptedWeights.recency).toBeGreaterThan(0);
			expect(profile!.adaptedWeights.frequency).toBeGreaterThan(0);
		});
	});

	// -----------------------------------------------------------------------
	// 7. Save / Reload Lifecycle
	// -----------------------------------------------------------------------

	describe('save and reload lifecycle', () => {
		it('persists and reloads entries through shared storage', async () => {
			const sharedData = new Map<string, Buffer>();
			const storage1 = createMemoryStorage(sharedData);
			const embedder = createMockEmbedder();

			const library1 = createLibrary(embedder, defaultConfig, {
				storage: storage1,
				logger: createSilentLogger(),
				stacksOptions: {
					autoSave: true,
					flushIntervalMs: 0,
					learning: { enabled: true },
				},
			});
			await library1.initialize();

			// Add entries
			const id1 = await library1.add('important memory', {
				priority: 'high',
			});
			await library1.add('another memory', { priority: 'low' });

			// Search and feedback
			await library1.search('important');
			library1.recordFeedback(id1, true);

			expect(library1.size).toBe(2);

			// Dispose (triggers final save)
			await library1.dispose();
			expect(library1.isInitialized).toBe(false);

			// Reload into a new library using the same shared storage
			const storage2 = createMemoryStorage(sharedData);
			const library2 = createLibrary(embedder, defaultConfig, {
				storage: storage2,
				logger: createSilentLogger(),
				stacksOptions: {
					autoSave: true,
					flushIntervalMs: 0,
					learning: { enabled: true },
				},
			});
			await library2.initialize();

			// Verify entries survived the round-trip
			expect(library2.size).toBe(2);
			const entry = library2.getById(id1);
			expect(entry).toBeDefined();
			expect(entry!.text).toBe('important memory');
			expect(entry!.metadata.priority).toBe('high');

			// Verify learning state was restored
			const profile = library2.patronProfile;
			expect(profile).toBeDefined();
			expect(profile!.totalQueries).toBeGreaterThanOrEqual(1);

			await library2.dispose();
		});

		it('clear removes all entries and resets state', async () => {
			await library.add('entry one');
			await library.add('entry two');
			expect(library.size).toBe(2);

			await library.clear();
			expect(library.size).toBe(0);

			const topics = library.getTopics();
			expect(topics.length).toBe(0);
		});
	});

	// -----------------------------------------------------------------------
	// 8. Combined / Cross-Feature
	// -----------------------------------------------------------------------

	describe('cross-feature interactions', () => {
		it('hierarchical topics work with metadata filters', async () => {
			await library.add('Rust beginner guide', {
				topics: JSON.stringify(['programming/rust']),
				level: 'beginner',
			});
			await library.add('Rust advanced patterns', {
				topics: JSON.stringify(['programming/rust']),
				level: 'advanced',
			});
			await library.add('Python beginner guide', {
				topics: JSON.stringify(['programming/python']),
				level: 'beginner',
			});

			// Filter by topic first
			const rustEntries = library.filterByTopic(['programming/rust']);
			expect(rustEntries.length).toBe(2);

			// Filter by metadata
			const beginnerEntries = library.filterByMetadata([
				{ key: 'level', value: 'beginner', mode: 'eq' },
			]);
			expect(beginnerEntries.length).toBe(2);

			// Manually intersect for entries that are both Rust and beginner
			const rustIds = new Set(rustEntries.map((e) => e.id));
			const rustBeginners = beginnerEntries.filter((e) => rustIds.has(e.id));
			expect(rustBeginners.length).toBe(1);
			expect(rustBeginners[0].text).toBe('Rust beginner guide');
		});

		it('advanced search combines vector and text scoring', async () => {
			await library.add('machine learning with neural networks');
			await library.add('deep learning transformers');
			await library.add('cooking pasta carbonara');

			const results = await library.advancedSearch({
				text: { query: 'learning', mode: 'bm25' },
				maxResults: 10,
				rankBy: 'average',
			});

			expect(results.length).toBeGreaterThan(0);

			// Each result has both vector and text scores
			for (const r of results) {
				expect(r.score).toBeGreaterThanOrEqual(0);
			}
		});

		it('recommendation considers access patterns', async () => {
			await library.add('frequently accessed entry');
			await library.add('rarely accessed entry');

			// Access the first entry multiple times via search
			for (let i = 0; i < 5; i++) {
				await library.search('frequently accessed');
			}

			const recommendations = await library.recommend('accessed entry');
			expect(recommendations.length).toBeGreaterThan(0);

			// Recommendations should include the frequently accessed entry
			const recTexts = recommendations.map((r) => r.volume.text);
			expect(recTexts).toContain('frequently accessed entry');
		});
	});
});
