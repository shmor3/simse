import { afterEach, beforeEach, describe, expect, it, mock } from 'bun:test';
import { fileURLToPath } from 'node:url';
import type { EmbeddingProvider, LibraryConfig } from '../src/ai/library/types.js';
import { createLibrary, type Library } from '../src/ai/library/library.js';
import { createSilentLogger } from './utils/mocks.js';

const ENGINE_PATH = fileURLToPath(
	new URL(
		'../simse-vector/target/debug/simse-vector-engine.exe',
		import.meta.url,
	),
);

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
			enginePath: ENGINE_PATH,
			logger: createSilentLogger(),
			stacksOptions: {
				learning: { enabled: false },
			},
		});
		await library.initialize();
	});

	afterEach(async () => {
		await library?.dispose();
	});

	it('library.shelf() returns a Shelf with the given name', () => {
		const shelf = library.shelf('researcher');
		expect(shelf.name).toBe('researcher');
	});

	it('shelf.add() stores with shelf metadata', async () => {
		const shelf = library.shelf('researcher');
		const id = await shelf.add('finding about APIs', { topic: 'api' });
		const volume = await library.getById(id);
		expect(volume).toBeDefined();
		expect(volume?.metadata.shelf).toBe('researcher');
	});

	it('shelf.search() returns only volumes from that shelf', async () => {
		const s1 = library.shelf('researcher');
		const s2 = library.shelf('writer');
		await s1.add('API endpoint design');
		await s2.add('prose style guide');
		const results = await s1.search('design', undefined, -1);
		expect(results.length).toBeGreaterThan(0);
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
		const vols = await shelf.volumes();
		expect(vols.length).toBe(2);
		for (const v of vols) {
			expect(v.metadata.shelf).toBe('test');
		}
	});

	it('library.shelves() lists all shelf names', async () => {
		library.shelf('alpha');
		library.shelf('beta');
		await library.shelf('alpha').add('note');
		const names = await library.shelves();
		expect(names).toContain('alpha');
	});
});
