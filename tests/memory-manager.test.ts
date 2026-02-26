import { beforeEach, describe, expect, it, mock } from 'bun:test';
import {
	createMemoryManager,
	type MemoryManager,
} from '../src/ai/memory/memory.js';
import type {
	EmbeddingProvider,
	MemoryConfig,
} from '../src/ai/memory/types.js';
import { createMemoryStorage, createSilentLogger } from './utils/mocks.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createMockEmbedder(dim = 3): EmbeddingProvider {
	let callCount = 0;
	return {
		embed: mock(async (input: string | readonly string[]) => {
			const texts = typeof input === 'string' ? [input] : input;
			callCount++;
			return {
				embeddings: texts.map((_, i) => {
					// Generate slightly different embeddings each time
					const base = (callCount * 10 + i) * 0.1;
					return Array.from({ length: dim }, (__, j) =>
						Math.sin(base + j * 0.7),
					);
				}),
			};
		}),
	};
}

const defaultConfig: MemoryConfig = {
	enabled: true,
	embeddingAgent: 'test-embedder',
	similarityThreshold: 0,
	maxResults: 10,
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('MemoryManager', () => {
	let manager: MemoryManager;
	let embedder: EmbeddingProvider;

	beforeEach(async () => {
		embedder = createMockEmbedder();
		manager = createMemoryManager(embedder, defaultConfig, {
			storage: createMemoryStorage(),
			logger: createSilentLogger(),
			vectorStoreOptions: {
				autoSave: true,
				flushIntervalMs: 0,
				learning: { enabled: false },
			},
		});
		await manager.initialize();
	});

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	describe('initialize', () => {
		it('sets isInitialized to true', () => {
			expect(manager.isInitialized).toBe(true);
		});

		it('deduplicates concurrent initialize calls', async () => {
			const fresh = createMemoryManager(embedder, defaultConfig, {
				storage: createMemoryStorage(),
				logger: createSilentLogger(),
				vectorStoreOptions: {
					autoSave: true,
					flushIntervalMs: 0,
					learning: { enabled: false },
				},
			});
			const [, ,] = await Promise.all([
				fresh.initialize(),
				fresh.initialize(),
				fresh.initialize(),
			]);
			expect(fresh.isInitialized).toBe(true);
		});
	});

	describe('dispose', () => {
		it('sets isInitialized to false after dispose', async () => {
			await manager.dispose();
			expect(manager.isInitialized).toBe(false);
		});
	});

	// -----------------------------------------------------------------------
	// add / addBatch
	// -----------------------------------------------------------------------

	describe('add', () => {
		it('adds an entry and returns an ID', async () => {
			const id = await manager.add('hello world');
			expect(id).toBeTruthy();
			expect(manager.size).toBe(1);
		});

		it('adds an entry with metadata', async () => {
			const id = await manager.add('hello', { topic: 'greeting' });
			const entry = manager.getById(id);
			expect(entry).toBeDefined();
			expect(entry!.metadata).toEqual({ topic: 'greeting' });
		});

		it('throws on empty text', async () => {
			await expect(manager.add('')).rejects.toThrow();
		});

		it('throws on whitespace-only text', async () => {
			await expect(manager.add('   ')).rejects.toThrow();
		});

		it('throws when not initialized', async () => {
			const fresh = createMemoryManager(embedder, defaultConfig, {
				storage: createMemoryStorage(),
				logger: createSilentLogger(),
				vectorStoreOptions: {
					autoSave: true,
					flushIntervalMs: 0,
					learning: { enabled: false },
				},
			});
			await expect(fresh.add('test')).rejects.toThrow();
		});
	});

	describe('addBatch', () => {
		it('adds multiple entries and returns their IDs', async () => {
			const ids = await manager.addBatch([
				{ text: 'one' },
				{ text: 'two' },
				{ text: 'three' },
			]);
			expect(ids).toHaveLength(3);
			expect(manager.size).toBe(3);
		});

		it('returns empty array for empty batch', async () => {
			const ids = await manager.addBatch([]);
			expect(ids).toEqual([]);
		});

		it('throws on empty text in batch', async () => {
			await expect(
				manager.addBatch([{ text: 'ok' }, { text: '' }]),
			).rejects.toThrow();
		});

		it('preserves metadata on batch entries', async () => {
			const ids = await manager.addBatch([
				{ text: 'one', metadata: { k: 'v1' } },
				{ text: 'two', metadata: { k: 'v2' } },
			]);
			const e1 = manager.getById(ids[0]);
			const e2 = manager.getById(ids[1]);
			expect(e1!.metadata).toEqual({ k: 'v1' });
			expect(e2!.metadata).toEqual({ k: 'v2' });
		});
	});

	// -----------------------------------------------------------------------
	// delete / deleteBatch
	// -----------------------------------------------------------------------

	describe('delete', () => {
		it('removes an entry by ID', async () => {
			const id = await manager.add('to delete');
			expect(manager.size).toBe(1);
			const deleted = await manager.delete(id);
			expect(deleted).toBe(true);
			expect(manager.size).toBe(0);
		});

		it('returns false for non-existent ID', async () => {
			const deleted = await manager.delete('nonexistent');
			expect(deleted).toBe(false);
		});
	});

	describe('deleteBatch', () => {
		it('removes multiple entries', async () => {
			const ids = await manager.addBatch([
				{ text: 'a' },
				{ text: 'b' },
				{ text: 'c' },
			]);
			const count = await manager.deleteBatch([ids[0], ids[2]]);
			expect(count).toBe(2);
			expect(manager.size).toBe(1);
		});

		it('returns 0 for empty array', async () => {
			const count = await manager.deleteBatch([]);
			expect(count).toBe(0);
		});
	});

	// -----------------------------------------------------------------------
	// search
	// -----------------------------------------------------------------------

	describe('search', () => {
		it('returns results ranked by similarity', async () => {
			await manager.add('cats are cute');
			await manager.add('dogs are loyal');
			const results = await manager.search('cats');
			expect(results.length).toBeGreaterThan(0);
		});

		it('returns empty array for whitespace query', async () => {
			await manager.add('test');
			const results = await manager.search('   ');
			expect(results).toEqual([]);
		});
	});

	// -----------------------------------------------------------------------
	// findDuplicates / findDuplicateGroups
	// -----------------------------------------------------------------------

	describe('findDuplicates', () => {
		it('finds duplicates above threshold', async () => {
			// Use a manager with duplicate threshold
			const dm = createMemoryManager(
				{
					embed: mock(async () => ({
						embeddings: [[1, 0, 0]],
					})),
				},
				defaultConfig,
				{
					storage: createMemoryStorage(),
					logger: createSilentLogger(),
					vectorStoreOptions: {
						autoSave: true,
						flushIntervalMs: 0,
						duplicateThreshold: 0.99,
						duplicateBehavior: 'warn',
						learning: { enabled: false },
					},
				},
			);
			await dm.initialize();
			await dm.add('entry 1');
			await dm.add('entry 2');

			const groups = dm.findDuplicates(0.99);
			expect(groups.length).toBeGreaterThan(0);
		});
	});

	// -----------------------------------------------------------------------
	// summarize
	// -----------------------------------------------------------------------

	describe('summarize', () => {
		it('throws when no text generator is set', async () => {
			const id1 = await manager.add('first');
			const id2 = await manager.add('second');

			await expect(manager.summarize({ ids: [id1, id2] })).rejects.toThrow();
		});

		it('summarizes entries with text generator', async () => {
			manager.setTextGenerator({
				generate: mock(async () => 'Summary of entries'),
			});

			const id1 = await manager.add('first thing');
			const id2 = await manager.add('second thing');

			const result = await manager.summarize({ ids: [id1, id2] });
			expect(result.summaryText).toBe('Summary of entries');
			expect(result.sourceIds).toEqual([id1, id2]);
			expect(result.deletedOriginals).toBe(false);
		});

		it('deletes originals when requested', async () => {
			manager.setTextGenerator({
				generate: mock(async () => 'Summary'),
			});

			const id1 = await manager.add('first');
			const id2 = await manager.add('second');
			const sizeBefore = manager.size;

			const result = await manager.summarize({
				ids: [id1, id2],
				deleteOriginals: true,
			});
			expect(result.deletedOriginals).toBe(true);
			// Should have removed 2 originals and added 1 summary
			expect(manager.size).toBe(sizeBefore - 2 + 1);
		});

		it('throws when fewer than 2 IDs provided', async () => {
			manager.setTextGenerator({
				generate: mock(async () => 'Summary'),
			});
			const id = await manager.add('only one');

			await expect(manager.summarize({ ids: [id] })).rejects.toThrow();
		});
	});

	// -----------------------------------------------------------------------
	// clear
	// -----------------------------------------------------------------------

	describe('clear', () => {
		it('removes all entries', async () => {
			await manager.add('a');
			await manager.add('b');
			expect(manager.size).toBe(2);

			await manager.clear();
			expect(manager.size).toBe(0);
		});
	});
});
