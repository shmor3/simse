import { afterEach, describe, expect, it, mock } from 'bun:test';
import { isEmbeddingError } from '../src/errors/index.js';

// ---------------------------------------------------------------------------
// Mock pipeline — we don't want to download a real model in tests
// ---------------------------------------------------------------------------

const MOCK_EMBEDDINGS = [
	[0.1, 0.2, 0.3, 0.4],
	[0.5, 0.6, 0.7, 0.8],
];

function createMockExtractor(embeddings: number[][] = MOCK_EMBEDDINGS) {
	return mock((_input: string | string[], _opts: unknown) =>
		Promise.resolve({ tolist: () => embeddings }),
	);
}

function createMockPipeline(extractor: ReturnType<typeof createMockExtractor>) {
	return mock((_task: string, _model: string, _opts: unknown) =>
		Promise.resolve(extractor),
	);
}

// ---------------------------------------------------------------------------
// Helpers to swap the module mock
// ---------------------------------------------------------------------------

let mockPipelineFn: ReturnType<typeof createMockPipeline>;
let mockExtractorFn: ReturnType<typeof createMockExtractor>;

// We mock the dynamic import by replacing the module in Bun's registry.
// Since local-embedder uses `await import(...)`, we use Bun's mock.module.
function setupMocks(
	extractor?: ReturnType<typeof createMockExtractor>,
	pipelineFn?: ReturnType<typeof createMockPipeline>,
) {
	mockExtractorFn = extractor ?? createMockExtractor();
	mockPipelineFn = pipelineFn ?? createMockPipeline(mockExtractorFn);

	mock.module('@huggingface/transformers', () => ({
		pipeline: mockPipelineFn,
	}));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('createLocalEmbedder', () => {
	afterEach(() => {
		mock.restore();
	});

	it('returns a frozen EmbeddingProvider', async () => {
		setupMocks();
		// Fresh import after mock
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);
		const embedder = createLocalEmbedder();
		expect(Object.isFrozen(embedder)).toBe(true);
		expect(typeof embedder.embed).toBe('function');
	});

	it('embeds a single string', async () => {
		const extractor = createMockExtractor([[0.1, 0.2, 0.3]]);
		setupMocks(extractor);
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();
		const result = await embedder.embed('hello');

		expect(result.embeddings).toEqual([[0.1, 0.2, 0.3]]);
		expect(extractor).toHaveBeenCalledWith(['hello'], {
			pooling: 'mean',
			normalize: true,
		});
	});

	it('embeds an array of strings', async () => {
		setupMocks();
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();
		const result = await embedder.embed(['hello', 'world']);

		expect(result.embeddings).toEqual(MOCK_EMBEDDINGS);
		expect(mockExtractorFn).toHaveBeenCalledWith(['hello', 'world'], {
			pooling: 'mean',
			normalize: true,
		});
	});

	it('loads model lazily on first call', async () => {
		setupMocks();
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();

		// Pipeline not loaded yet
		expect(mockPipelineFn).not.toHaveBeenCalled();

		await embedder.embed('test');

		// Now it's loaded
		expect(mockPipelineFn).toHaveBeenCalledTimes(1);
	});

	it('reuses pipeline on subsequent calls', async () => {
		setupMocks();
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();
		await embedder.embed('first');
		await embedder.embed('second');

		// Pipeline loaded only once
		expect(mockPipelineFn).toHaveBeenCalledTimes(1);
		// Extractor called twice
		expect(mockExtractorFn).toHaveBeenCalledTimes(2);
	});

	it('passes custom options to pipeline', async () => {
		setupMocks();
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder({
			model: 'custom/model',
			dtype: 'fp32',
			pooling: 'cls',
			normalize: false,
		});
		await embedder.embed('test');

		expect(mockPipelineFn).toHaveBeenCalledWith(
			'feature-extraction',
			'custom/model',
			{ dtype: 'fp32' },
		);
		expect(mockExtractorFn).toHaveBeenCalledWith(['test'], {
			pooling: 'cls',
			normalize: false,
		});
	});

	it('uses default model nomic-ai/nomic-embed-text-v1.5', async () => {
		setupMocks();
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();
		await embedder.embed('test');

		expect(mockPipelineFn).toHaveBeenCalledWith(
			'feature-extraction',
			'nomic-ai/nomic-embed-text-v1.5',
			{ dtype: 'q8' },
		);
	});

	it('throws EmbeddingError when model fails to load', async () => {
		const failingPipeline = mock(() =>
			Promise.reject(new Error('Model not found')),
		);
		setupMocks(undefined, failingPipeline);
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();

		try {
			await embedder.embed('test');
			expect.unreachable('should have thrown');
		} catch (err) {
			expect(isEmbeddingError(err)).toBe(true);
			expect((err as Error).message).toContain(
				'Failed to load embedding model',
			);
			expect((err as Error).message).toContain('Model not found');
		}
	});

	it('throws EmbeddingError when extractor returns empty', async () => {
		const emptyExtractor = createMockExtractor([]);
		setupMocks(emptyExtractor);
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();

		try {
			await embedder.embed('test');
			expect.unreachable('should have thrown');
		} catch (err) {
			expect(isEmbeddingError(err)).toBe(true);
			expect((err as Error).message).toContain('returned no embeddings');
		}
	});

	it('throws EmbeddingError on extractor failure', async () => {
		const failingExtractor = mock(() =>
			Promise.reject(new Error('ONNX runtime error')),
		);
		setupMocks(failingExtractor);
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();

		try {
			await embedder.embed('test');
			expect.unreachable('should have thrown');
		} catch (err) {
			expect(isEmbeddingError(err)).toBe(true);
			expect((err as Error).message).toContain('ONNX runtime error');
		}
	});

	it('re-throws existing embedding errors without wrapping', async () => {
		const emptyExtractor = createMockExtractor([]);
		setupMocks(emptyExtractor);
		const { createLocalEmbedder } = await import(
			'../src/ai/acp/local-embedder.js'
		);

		const embedder = createLocalEmbedder();

		try {
			await embedder.embed('test');
			expect.unreachable('should have thrown');
		} catch (err) {
			expect(isEmbeddingError(err)).toBe(true);
			// Should be directly thrown, not double-wrapped
			expect((err as Error).message).not.toContain('Local embedding failed');
		}
	});
});
