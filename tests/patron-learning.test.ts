import { beforeEach, describe, expect, it } from 'bun:test';
import type { Buffer } from 'node:buffer';
import type { LearningEngine } from '../src/ai/library/patron-learning.js';
import { createLearningEngine } from '../src/ai/library/patron-learning.js';
import type { StorageBackend } from '../src/ai/library/storage.js';
import { createStacks } from '../src/ai/library/stacks.js';
import { createLogger, type Logger } from '../src/logger.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createSilentLogger(): Logger {
	return createLogger({ context: 'test', level: 'none', transports: [] });
}

function createMemoryStorage(sharedData?: Map<string, Buffer>): StorageBackend {
	const data: Map<string, Buffer> = sharedData ?? new Map();
	return Object.freeze({
		load: async () => new Map(data),
		save: async (newData: Map<string, Buffer>) => {
			data.clear();
			for (const [k, v] of newData) {
				data.set(k, v);
			}
		},
		close: async () => {},
	});
}

/** Generate a simple deterministic embedding vector. */
function makeEmbedding(seed: number, dims = 8): number[] {
	const result: number[] = [];
	for (let i = 0; i < dims; i++) {
		result.push(Math.sin(seed * (i + 1) * 0.7));
	}
	// Normalize
	const mag = Math.sqrt(result.reduce((s, v) => s + v * v, 0));
	return mag > 0 ? result.map((v) => v / mag) : result;
}

// ---------------------------------------------------------------------------
// Learning Engine — Unit Tests
// ---------------------------------------------------------------------------

describe('createLearningEngine', () => {
	let engine: LearningEngine;

	beforeEach(() => {
		engine = createLearningEngine();
	});

	it('starts with zero queries and no data', () => {
		expect(engine.totalQueries).toBe(0);
		expect(engine.hasData).toBe(false);
		expect(engine.getInterestEmbedding()).toBeUndefined();
	});

	it('records a query and updates state', () => {
		const embedding = makeEmbedding(1);
		engine.recordQuery(embedding, ['a', 'b']);

		expect(engine.totalQueries).toBe(1);
		expect(engine.hasData).toBe(true);
	});

	it('tracks relevance feedback per entry', () => {
		const emb1 = makeEmbedding(1);
		const emb2 = makeEmbedding(2);

		engine.recordQuery(emb1, ['a', 'b']);
		engine.recordQuery(emb2, ['a', 'c']);

		const fbA = engine.getRelevanceFeedback('a');
		expect(fbA).toBeDefined();
		expect(fbA!.totalRetrievals).toBe(2);
		expect(fbA!.queryCount).toBe(2); // two diverse queries

		const fbB = engine.getRelevanceFeedback('b');
		expect(fbB).toBeDefined();
		expect(fbB!.totalRetrievals).toBe(1);

		const fbD = engine.getRelevanceFeedback('nonexistent');
		expect(fbD).toBeUndefined();
	});

	it('caps query history at maxQueryHistory', () => {
		const engine5 = createLearningEngine({ maxQueryHistory: 5 });
		for (let i = 0; i < 10; i++) {
			engine5.recordQuery(makeEmbedding(i), ['x']);
		}
		const profile = engine5.getProfile();
		expect(profile.queryHistory.length).toBe(5);
		expect(profile.totalQueries).toBe(10);
	});

	it('computes an interest embedding from query history', () => {
		engine.recordQuery(makeEmbedding(1), ['a']);
		engine.recordQuery(makeEmbedding(2), ['b']);

		const interest = engine.getInterestEmbedding();
		expect(interest).toBeDefined();
		expect(interest!.length).toBe(8);

		// Should be a unit vector
		const mag = Math.sqrt(interest!.reduce((s, v) => s + v * v, 0));
		expect(mag).toBeCloseTo(1.0, 4);
	});

	it('adapts weight profile over queries', () => {
		const initial = engine.getAdaptedWeights();
		expect(initial.vector).toBeCloseTo(0.6, 2);

		// Record many queries where entries get retrieved repeatedly
		// to trigger frequency weight adaptation
		for (let i = 0; i < 20; i++) {
			engine.recordQuery(makeEmbedding(i), ['a', 'b']);
		}

		const adapted = engine.getAdaptedWeights();
		// Weights should have shifted from defaults
		const sum = adapted.vector + adapted.recency + adapted.frequency;
		expect(sum).toBeCloseTo(1.0, 4);
	});

	it('computes boost within bounds', () => {
		engine.recordQuery(makeEmbedding(1), ['a']);

		const boost = engine.computeBoost('a', makeEmbedding(1));
		expect(boost).toBeGreaterThanOrEqual(0.8);
		expect(boost).toBeLessThanOrEqual(1.2);

		// Unknown entry gets neutral boost
		const boostUnknown = engine.computeBoost('unknown', makeEmbedding(1));
		expect(boostUnknown).toBeGreaterThanOrEqual(0.8);
		expect(boostUnknown).toBeLessThanOrEqual(1.2);
	});

	it('entries retrieved by diverse queries get higher boost', () => {
		// Entry 'a' retrieved by many diverse queries
		for (let i = 0; i < 10; i++) {
			engine.recordQuery(makeEmbedding(i * 10), ['a']);
		}
		// Entry 'b' retrieved by only one query
		engine.recordQuery(makeEmbedding(100), ['b']);

		const boostA = engine.computeBoost('a', makeEmbedding(5));
		const boostB = engine.computeBoost('b', makeEmbedding(5));
		expect(boostA).toBeGreaterThan(boostB);
	});

	it('serializes and restores state', () => {
		engine.recordQuery(makeEmbedding(1), ['a', 'b']);
		engine.recordQuery(makeEmbedding(2), ['a', 'c']);

		const serialized = engine.serialize();
		expect(serialized.version).toBe(1);
		expect(serialized.totalQueries).toBe(2);
		expect(serialized.feedback.length).toBe(3); // a, b, c

		const engine2 = createLearningEngine();
		engine2.restore(serialized);

		expect(engine2.totalQueries).toBe(2);
		expect(engine2.getRelevanceFeedback('a')?.totalRetrievals).toBe(2);
		expect(engine2.getAdaptedWeights().vector).toBeCloseTo(
			engine.getAdaptedWeights().vector,
			4,
		);
	});

	it('clears all state', () => {
		engine.recordQuery(makeEmbedding(1), ['a']);
		engine.clear();

		expect(engine.totalQueries).toBe(0);
		expect(engine.hasData).toBe(false);
		expect(engine.getRelevanceFeedback('a')).toBeUndefined();
		expect(engine.getInterestEmbedding()).toBeUndefined();
	});

	it('prunes entries that no longer exist', () => {
		engine.recordQuery(makeEmbedding(1), ['a', 'b', 'c']);

		engine.pruneEntries(new Set(['a', 'c']));

		expect(engine.getRelevanceFeedback('a')).toBeDefined();
		expect(engine.getRelevanceFeedback('b')).toBeUndefined();
		expect(engine.getRelevanceFeedback('c')).toBeDefined();
	});

	it('does nothing when disabled', () => {
		const disabled = createLearningEngine({ enabled: false });
		disabled.recordQuery(makeEmbedding(1), ['a']);

		expect(disabled.totalQueries).toBe(0);
		expect(disabled.computeBoost('a', makeEmbedding(1))).toBe(1.0);
	});

	it('ignores empty embeddings or empty result sets', () => {
		engine.recordQuery([], ['a']);
		engine.recordQuery(makeEmbedding(1), []);

		expect(engine.totalQueries).toBe(0);
	});

	it('returns a complete profile', () => {
		engine.recordQuery(makeEmbedding(1), ['a']);

		const profile = engine.getProfile();
		expect(profile.totalQueries).toBe(1);
		expect(profile.queryHistory.length).toBe(1);
		expect(profile.adaptedWeights.vector).toBeGreaterThan(0);
		expect(profile.lastUpdated).toBeGreaterThan(0);
	});
});

// ---------------------------------------------------------------------------
// VectorStore Learning Integration Tests
// ---------------------------------------------------------------------------

describe('Stacks learning integration', () => {
	it('creates a learning engine by default', async () => {
		const store = createStacks({
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
		});
		await store.load();

		expect(store.learningEngine).toBeDefined();
		expect(store.learningProfile).toBeDefined();
		expect(store.learningProfile!.totalQueries).toBe(0);

		await store.dispose();
	});

	it('disables learning when configured', async () => {
		const store = createStacks({
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			learning: { enabled: false },
		});
		await store.load();

		expect(store.learningEngine).toBeUndefined();
		expect(store.learningProfile).toBeUndefined();

		await store.dispose();
	});

	it('records queries during search()', async () => {
		const store = createStacks({
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			autoSave: true,
		});
		await store.load();

		await store.add('hello world', makeEmbedding(1));
		await store.add('goodbye world', makeEmbedding(2));

		store.search(makeEmbedding(1), 5, 0);

		expect(store.learningProfile!.totalQueries).toBe(1);

		await store.dispose();
	});

	it('records queries during advancedSearch()', async () => {
		const store = createStacks({
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			autoSave: true,
		});
		await store.load();

		await store.add('hello world', makeEmbedding(1));

		store.advancedSearch({
			queryEmbedding: makeEmbedding(1),
			maxResults: 5,
		});

		expect(store.learningProfile!.totalQueries).toBe(1);

		await store.dispose();
	});

	it('applies learning boost to recommend()', async () => {
		const store = createStacks({
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			autoSave: true,
		});
		await store.load();

		const id1 = await store.add('frequently queried', makeEmbedding(1));
		await store.add('rarely queried', makeEmbedding(2));

		// Search for entry 1 many times to build up feedback
		for (let i = 0; i < 5; i++) {
			store.search(makeEmbedding(1 + i * 0.01), 1, 0);
		}

		const recs = store.recommend({
			queryEmbedding: makeEmbedding(1),
			maxResults: 5,
		});

		expect(recs.length).toBeGreaterThan(0);
		// The first result should be the frequently-queried entry
		expect(recs[0].volume.id).toBe(id1);

		await store.dispose();
	});

	it('persists and restores learning state', async () => {
		const sharedData = new Map<string, Buffer>();

		// First session: create store, add entries, search, save
		const store1 = createStacks({
			storage: createMemoryStorage(sharedData),
			logger: createSilentLogger(),
			autoSave: true,
		});
		await store1.load();

		await store1.add('test entry', makeEmbedding(1));
		store1.search(makeEmbedding(1), 5, 0);

		expect(store1.learningProfile!.totalQueries).toBe(1);
		await store1.dispose();

		// Second session: load and verify learning state persisted
		const store2 = createStacks({
			storage: createMemoryStorage(sharedData),
			logger: createSilentLogger(),
		});
		await store2.load();

		expect(store2.learningProfile!.totalQueries).toBe(1);

		await store2.dispose();
	});

	it('clears learning state on clear()', async () => {
		const store = createStacks({
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			autoSave: true,
		});
		await store.load();

		await store.add('test', makeEmbedding(1));
		store.search(makeEmbedding(1), 5, 0);

		expect(store.learningProfile!.totalQueries).toBe(1);

		await store.clear();
		expect(store.learningProfile!.totalQueries).toBe(0);

		await store.dispose();
	});
});
