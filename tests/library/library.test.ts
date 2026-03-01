import { afterEach, beforeEach, describe, expect, it, mock } from 'bun:test';
import { fileURLToPath } from 'node:url';
import { createLibrary, type Library } from '../../src/ai/library/library.js';
import type { EmbeddingProvider, LibraryConfig } from '../../src/ai/library/types.js';
import { createSilentLogger } from './utils.js';

const ENGINE_PATH = fileURLToPath(
	new URL(
		'../../simse-vector/target/debug/simse-vector-engine.exe',
		import.meta.url,
	),
);

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

const defaultConfig: LibraryConfig = {
	enabled: true,
	embeddingAgent: 'test-embedder',
	similarityThreshold: 0,
	maxResults: 10,
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Library', () => {
	let library: Library;
	let embedder: EmbeddingProvider;

	beforeEach(async () => {
		embedder = createMockEmbedder();
		library = createLibrary(embedder, defaultConfig, {
			enginePath: ENGINE_PATH,
			logger: createSilentLogger(),
			stacksOptions: {
				duplicateThreshold: 1,
				learning: { enabled: false },
			},
		});
		await library.initialize();
	});

	afterEach(async () => {
		await library?.dispose();
	});

	// -----------------------------------------------------------------------
	// Interface shape
	// -----------------------------------------------------------------------

	it('has the Library interface shape', () => {
		expect(typeof library.add).toBe('function');
		expect(typeof library.search).toBe('function');
		expect(typeof library.compendium).toBe('function');
		expect('patronProfile' in library).toBe(true);
	});

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	describe('initialize', () => {
		it('sets isInitialized to true', () => {
			expect(library.isInitialized).toBe(true);
		});

		it('deduplicates concurrent initialize calls', async () => {
			const fresh = createLibrary(embedder, defaultConfig, {
				enginePath: ENGINE_PATH,
				logger: createSilentLogger(),
				stacksOptions: {
					duplicateThreshold: 1,
					learning: { enabled: false },
				},
			});
			const [, ,] = await Promise.all([
				fresh.initialize(),
				fresh.initialize(),
				fresh.initialize(),
			]);
			expect(fresh.isInitialized).toBe(true);
			await fresh.dispose();
		});
	});

	describe('dispose', () => {
		it('sets isInitialized to false after dispose', async () => {
			await library.dispose();
			expect(library.isInitialized).toBe(false);
		});
	});

	// -----------------------------------------------------------------------
	// add / addBatch
	// -----------------------------------------------------------------------

	describe('add', () => {
		it('adds a volume and returns an ID', async () => {
			const id = await library.add('hello world');
			expect(id).toBeTruthy();
			expect(await library.size).toBe(1);
		});

		it('adds a volume with metadata', async () => {
			const id = await library.add('hello', { topic: 'greeting' });
			const volume = await library.getById(id);
			expect(volume).toBeDefined();
			expect(volume!.metadata).toEqual({ topic: 'greeting' });
		});

		it('throws on empty text', async () => {
			await expect(library.add('')).rejects.toThrow();
		});

		it('throws on whitespace-only text', async () => {
			await expect(library.add('   ')).rejects.toThrow();
		});

		it('throws when not initialized', async () => {
			const fresh = createLibrary(embedder, defaultConfig, {
				enginePath: ENGINE_PATH,
				logger: createSilentLogger(),
				stacksOptions: {
					duplicateThreshold: 1,
					learning: { enabled: false },
				},
			});
			await expect(fresh.add('test')).rejects.toThrow();
		});
	});

	describe('addBatch', () => {
		it('adds multiple volumes and returns their IDs', async () => {
			const ids = await library.addBatch([
				{ text: 'one' },
				{ text: 'two' },
				{ text: 'three' },
			]);
			expect(ids).toHaveLength(3);
			expect(await library.size).toBe(3);
		});

		it('returns empty array for empty batch', async () => {
			const ids = await library.addBatch([]);
			expect(ids).toEqual([]);
		});

		it('throws on empty text in batch', async () => {
			await expect(
				library.addBatch([{ text: 'ok' }, { text: '' }]),
			).rejects.toThrow();
		});

		it('preserves metadata on batch volumes', async () => {
			const ids = await library.addBatch([
				{ text: 'one', metadata: { k: 'v1' } },
				{ text: 'two', metadata: { k: 'v2' } },
			]);
			const e1 = await library.getById(ids[0]);
			const e2 = await library.getById(ids[1]);
			expect(e1!.metadata).toEqual({ k: 'v1' });
			expect(e2!.metadata).toEqual({ k: 'v2' });
		});
	});

	// -----------------------------------------------------------------------
	// delete / deleteBatch
	// -----------------------------------------------------------------------

	describe('delete', () => {
		it('removes a volume by ID', async () => {
			const id = await library.add('to delete');
			expect(await library.size).toBe(1);
			const deleted = await library.delete(id);
			expect(deleted).toBe(true);
			expect(await library.size).toBe(0);
		});

		it('returns false for non-existent ID', async () => {
			const deleted = await library.delete('nonexistent');
			expect(deleted).toBe(false);
		});
	});

	describe('deleteBatch', () => {
		it('removes multiple volumes', async () => {
			const ids = await library.addBatch([
				{ text: 'a' },
				{ text: 'b' },
				{ text: 'c' },
			]);
			const count = await library.deleteBatch([ids[0], ids[2]]);
			expect(count).toBe(2);
			expect(await library.size).toBe(1);
		});

		it('returns 0 for empty array', async () => {
			const count = await library.deleteBatch([]);
			expect(count).toBe(0);
		});
	});

	// -----------------------------------------------------------------------
	// search
	// -----------------------------------------------------------------------

	describe('search', () => {
		it('returns results ranked by similarity', async () => {
			await library.add('cats are cute');
			await library.add('dogs are loyal');
			const results = await library.search('cats');
			expect(results.length).toBeGreaterThan(0);
		});

		it('returns Lookup[] with volume field', async () => {
			await library.add('important fact about databases', { topic: 'db' });
			const results = await library.search('databases');
			expect(results.length).toBeGreaterThan(0);
			expect(results[0].volume).toBeDefined();
			expect(results[0].volume.text).toContain('databases');
			expect(typeof results[0].score).toBe('number');
		});

		it('returns empty array for whitespace query', async () => {
			await library.add('test');
			const results = await library.search('   ');
			expect(results).toEqual([]);
		});
	});

	// -----------------------------------------------------------------------
	// findDuplicates
	// -----------------------------------------------------------------------

	describe('findDuplicates', () => {
		it('finds duplicates above threshold', async () => {
			const dm = createLibrary(
				{
					embed: mock(async () => ({
						embeddings: [[1, 0, 0]],
					})),
				},
				defaultConfig,
				{
					enginePath: ENGINE_PATH,
					logger: createSilentLogger(),
					stacksOptions: {
						// Disable dedup on add so both entries are stored
						duplicateThreshold: 1,
						learning: { enabled: false },
					},
				},
			);
			await dm.initialize();
			await dm.add('volume 1');
			await dm.add('volume 2');

			// Both stored; now check for duplicates at 0.99 threshold
			const groups = await dm.findDuplicates(0.99);
			expect(groups.length).toBeGreaterThan(0);
			await dm.dispose();
		});
	});

	// -----------------------------------------------------------------------
	// compendium (was summarize)
	// -----------------------------------------------------------------------

	describe('compendium', () => {
		it('throws when no text generator is set', async () => {
			const id1 = await library.add('first');
			const id2 = await library.add('second');

			await expect(library.compendium({ ids: [id1, id2] })).rejects.toThrow();
		});

		it('creates compendium with text generator', async () => {
			library.setTextGenerator({
				generate: mock(async () => 'Summary of volumes'),
			});

			const id1 = await library.add('first thing');
			const id2 = await library.add('second thing');

			const result = await library.compendium({ ids: [id1, id2] });
			expect(result.text).toBe('Summary of volumes');
			expect(result.sourceIds).toEqual([id1, id2]);
			expect(result.deletedOriginals).toBe(false);
		});

		it('deletes originals when requested', async () => {
			library.setTextGenerator({
				generate: mock(async () => 'Summary'),
			});

			const id1 = await library.add('first');
			const id2 = await library.add('second');
			const sizeBefore = await library.size;

			const result = await library.compendium({
				ids: [id1, id2],
				deleteOriginals: true,
			});
			expect(result.deletedOriginals).toBe(true);
			// Should have removed 2 originals and added 1 summary
			expect(await library.size).toBe(sizeBefore - 2 + 1);
		});

		it('throws when fewer than 2 IDs provided', async () => {
			library.setTextGenerator({
				generate: mock(async () => 'Summary'),
			});
			const id = await library.add('only one');

			await expect(library.compendium({ ids: [id] })).rejects.toThrow();
		});
	});

	// -----------------------------------------------------------------------
	// clear
	// -----------------------------------------------------------------------

	describe('clear', () => {
		it('removes all volumes', async () => {
			await library.add('a');
			await library.add('b');
			expect(await library.size).toBe(2);

			await library.clear();
			expect(await library.size).toBe(0);
		});
	});
});
