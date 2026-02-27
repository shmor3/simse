/**
 * E2E test: Full library pipeline — embed → store → search → dedup.
 *
 * Tests the complete flow from createLocalEmbedder through
 * createLibrary with real ONNX embeddings.
 */
import { describe, expect, it } from 'bun:test';
import type { Buffer } from 'node:buffer';
import { createLocalEmbedder } from '../src/ai/acp/local-embedder.js';
import { createLibrary } from '../src/ai/library/library.js';
import type { StorageBackend } from '../src/ai/library/storage.js';

// ---------------------------------------------------------------------------
// In-memory storage backend for tests
// ---------------------------------------------------------------------------

function createMemoryStorage(): StorageBackend {
	const data = new Map<string, Buffer>();
	return Object.freeze({
		load: async () => new Map(data),
		save: async (snapshot: Map<string, Buffer>) => {
			data.clear();
			for (const [k, v] of snapshot) data.set(k, v);
		},
		close: async () => {},
	});
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Library pipeline E2E', () => {
	const embedder = createLocalEmbedder({
		model: 'Xenova/all-MiniLM-L6-v2',
		dtype: 'q8',
	});

	it('adds volumes and searches semantically', async () => {
		const library = createLibrary(
			embedder,
			{},
			{
				storage: createMemoryStorage(),
			},
		);
		await library.initialize();

		await library.add('TypeScript is a typed superset of JavaScript', {
			topic: 'programming',
		});
		await library.add('Python is great for data science and machine learning', {
			topic: 'programming',
		});
		await library.add('The weather in London is often rainy and cold', {
			topic: 'weather',
		});

		// Search with positional args: query, maxResults, threshold
		const results = await library.search(
			'What programming languages are useful?',
			3,
			0.0,
		);

		expect(results.length).toBeGreaterThanOrEqual(2);

		// Programming results should rank above weather
		const topics = results.map((r) => r.volume.metadata?.topic);
		expect(topics[0]).toBe('programming');
		expect(topics[1]).toBe('programming');
	}, 120_000);

	it('detects near-duplicate text', async () => {
		const library = createLibrary(
			embedder,
			{ duplicateThreshold: 0.9 },
			{ storage: createMemoryStorage() },
		);
		await library.initialize();

		await library.add('TypeScript is a typed superset of JavaScript');

		const dupeResult = await library.checkDuplicate(
			'TypeScript is a typed superset of JavaScript language',
		);
		expect(dupeResult.isDuplicate).toBe(true);
		expect(dupeResult.similarity).toBeGreaterThan(0.8);
	}, 120_000);

	it('stores and retrieves by topic', async () => {
		const library = createLibrary(
			embedder,
			{},
			{
				storage: createMemoryStorage(),
			},
		);
		await library.initialize();

		await library.add('React is a UI library', { topic: 'frontend' });
		await library.add('Express is a Node framework', { topic: 'backend' });
		await library.add('Vue is another UI framework', { topic: 'frontend' });

		const frontend = library.filterByTopic(['frontend']);
		expect(frontend).toHaveLength(2);

		const backend = library.filterByTopic(['backend']);
		expect(backend).toHaveLength(1);

		const topics = library.getTopics();
		expect(topics.length).toBe(2);
	}, 120_000);
});
