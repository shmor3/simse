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
		expect('patronProfile' in library).toBe(true);
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
