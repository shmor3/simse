/**
 * E2E test: Full library pipeline — embed → store → search → dedup.
 *
 * Uses a deterministic mock embedder that produces consistent vectors
 * from text content for testing library semantics.
 */
import { describe, expect, it } from 'bun:test';
import type { Buffer } from 'node:buffer';
import { createLibrary } from '../src/library.js';
import type { StorageBackend } from '../src/storage.js';
import type { EmbeddingProvider } from '../src/types.js';

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
// Deterministic mock embedder
//
// Produces 64-dimensional vectors seeded from text content.
// Similar texts produce similar vectors (word overlap → dimension overlap).
// ---------------------------------------------------------------------------

function createMockEmbedder(): EmbeddingProvider {
	const DIM = 128;

	function wordHash(word: string): number {
		let h = 0;
		for (let i = 0; i < word.length; i++) {
			h = (h * 31 + word.charCodeAt(i)) | 0;
		}
		return ((h % DIM) + DIM) % DIM;
	}

	function hashText(text: string): number[] {
		const vec = new Array(DIM).fill(0);
		const words = text
			.toLowerCase()
			.replace(/[^a-z0-9\s]/g, '')
			.split(/\s+/);
		for (const word of words) {
			if (word.length === 0) continue;
			// Each word activates a primary and secondary dimension
			const primary = wordHash(word);
			const secondary = wordHash(`${word}_`);
			vec[primary] += 1.0;
			vec[secondary] += 0.5;
		}
		const mag =
			Math.sqrt(vec.reduce((s: number, v: number) => s + v * v, 0)) || 1;
		return vec.map((v: number) => v / mag);
	}

	return Object.freeze({
		embed: async (input: string | readonly string[]) => {
			const texts = typeof input === 'string' ? [input] : [...input];
			return { embeddings: texts.map(hashText) };
		},
	});
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Library pipeline E2E', () => {
	const embedder = createMockEmbedder();

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
	});

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
	});

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
	});
});
