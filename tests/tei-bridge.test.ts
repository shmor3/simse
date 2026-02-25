import { afterEach, describe, expect, it, mock } from 'bun:test';
import { createTEIEmbedder } from '../src/ai/acp/tei-bridge.js';
import { isEmbeddingError } from '../src/errors/index.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const MOCK_EMBEDDINGS = [
	[0.1, 0.2, 0.3],
	[0.4, 0.5, 0.6],
];

function jsonResponse(data: unknown, status = 200): Response {
	return new Response(JSON.stringify(data), {
		status,
		headers: { 'Content-Type': 'application/json' },
	});
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('createTEIEmbedder', () => {
	const originalFetch = globalThis.fetch;

	afterEach(() => {
		globalThis.fetch = originalFetch;
	});

	it('returns a frozen EmbeddingProvider', () => {
		const embedder = createTEIEmbedder();
		expect(Object.isFrozen(embedder)).toBe(true);
		expect(typeof embedder.embed).toBe('function');
	});

	it('embeds a single string', async () => {
		const fetchMock = mock(() =>
			Promise.resolve(jsonResponse([MOCK_EMBEDDINGS[0]])),
		);
		globalThis.fetch = fetchMock;

		const embedder = createTEIEmbedder();
		const result = await embedder.embed('hello');

		expect(result.embeddings).toEqual([MOCK_EMBEDDINGS[0]]);
		expect(fetchMock).toHaveBeenCalledTimes(1);

		const [url, opts] = fetchMock.mock.calls[0];
		expect(url).toBe('http://localhost:8080/embed');

		const body = JSON.parse((opts as RequestInit).body as string);
		expect(body.inputs).toBe('hello');
		expect(body.normalize).toBe(true);
		expect(body.truncate).toBe(false);
	});

	it('embeds an array of strings', async () => {
		const fetchMock = mock(() =>
			Promise.resolve(jsonResponse(MOCK_EMBEDDINGS)),
		);
		globalThis.fetch = fetchMock;

		const embedder = createTEIEmbedder();
		const result = await embedder.embed(['hello', 'world']);

		expect(result.embeddings).toEqual(MOCK_EMBEDDINGS);

		const body = JSON.parse(
			(fetchMock.mock.calls[0][1] as RequestInit).body as string,
		);
		expect(body.inputs).toEqual(['hello', 'world']);
	});

	it('uses custom baseUrl', async () => {
		const fetchMock = mock(() => Promise.resolve(jsonResponse([[0.1]])));
		globalThis.fetch = fetchMock;

		const embedder = createTEIEmbedder({ baseUrl: 'http://myhost:9090/' });
		await embedder.embed('x');

		expect(fetchMock.mock.calls[0][0]).toBe('http://myhost:9090/embed');
	});

	it('strips trailing slashes from baseUrl', async () => {
		const fetchMock = mock(() => Promise.resolve(jsonResponse([[0.1]])));
		globalThis.fetch = fetchMock;

		const embedder = createTEIEmbedder({ baseUrl: 'http://host:1234///' });
		await embedder.embed('x');

		expect(fetchMock.mock.calls[0][0]).toBe('http://host:1234/embed');
	});

	it('passes normalize and truncate options', async () => {
		const fetchMock = mock(() => Promise.resolve(jsonResponse([[0.1]])));
		globalThis.fetch = fetchMock;

		const embedder = createTEIEmbedder({
			normalize: false,
			truncate: true,
		});
		await embedder.embed('x');

		const body = JSON.parse(
			(fetchMock.mock.calls[0][1] as RequestInit).body as string,
		);
		expect(body.normalize).toBe(false);
		expect(body.truncate).toBe(true);
	});

	it('throws EmbeddingError on non-OK response', async () => {
		globalThis.fetch = mock(() =>
			Promise.resolve(
				new Response('model not found', {
					status: 404,
					statusText: 'Not Found',
				}),
			),
		);

		const embedder = createTEIEmbedder();

		try {
			await embedder.embed('test');
			expect.unreachable('should have thrown');
		} catch (err) {
			expect(isEmbeddingError(err)).toBe(true);
			expect((err as Error).message).toContain('404');
			expect((err as Error).message).toContain('model not found');
		}
	});

	it('throws EmbeddingError when response is not an array', async () => {
		globalThis.fetch = mock(() =>
			Promise.resolve(jsonResponse({ something: 'else' })),
		);

		const embedder = createTEIEmbedder();

		try {
			await embedder.embed('test');
			expect.unreachable('should have thrown');
		} catch (err) {
			expect(isEmbeddingError(err)).toBe(true);
			expect((err as Error).message).toContain('not an array');
		}
	});

	it('throws EmbeddingError on network failure', async () => {
		globalThis.fetch = mock(() =>
			Promise.reject(new Error('Connection refused')),
		);

		const embedder = createTEIEmbedder();

		try {
			await embedder.embed('test');
			expect.unreachable('should have thrown');
		} catch (err) {
			expect(isEmbeddingError(err)).toBe(true);
			expect((err as Error).message).toContain('Connection refused');
		}
	});

	it('re-throws existing embedding errors without wrapping', async () => {
		globalThis.fetch = mock(() =>
			Promise.resolve(
				new Response('gone', { status: 410, statusText: 'Gone' }),
			),
		);

		const embedder = createTEIEmbedder();

		try {
			await embedder.embed('test');
			expect.unreachable('should have thrown');
		} catch (err) {
			expect(isEmbeddingError(err)).toBe(true);
			// Should be directly thrown, not double-wrapped
			expect((err as Error).message).not.toContain('TEI embedding failed');
		}
	});
});
