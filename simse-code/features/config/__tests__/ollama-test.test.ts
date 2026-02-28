import { afterEach, describe, expect, it, mock } from 'bun:test';
import {
	formatBytes,
	listOllamaModels,
	testOllamaConnection,
} from '../ollama-test.js';
import type {
	OllamaConnectionResult,
	OllamaModelInfo,
} from '../ollama-test.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Assign a mock to globalThis.fetch, working around Bun's `preconnect` prop. */
// biome-ignore lint/suspicious/noExplicitAny: mock needs flexible signature
function mockFetch(impl: (...args: any[]) => any): void {
	globalThis.fetch = mock(impl) as unknown as typeof fetch;
}

// ---------------------------------------------------------------------------
// formatBytes
// ---------------------------------------------------------------------------

describe('formatBytes', () => {
	it('should format gigabytes', () => {
		expect(formatBytes(1_900_000_000)).toBe('1.8 GB');
	});

	it('should format megabytes', () => {
		expect(formatBytes(500_000_000)).toBe('476.8 MB');
	});

	it('should format kilobytes', () => {
		expect(formatBytes(1_500)).toBe('1.5 KB');
	});

	it('should format bytes', () => {
		expect(formatBytes(500)).toBe('500 B');
	});

	it('should format zero bytes', () => {
		expect(formatBytes(0)).toBe('0 B');
	});

	it('should format exact GB boundary', () => {
		expect(formatBytes(1_073_741_824)).toBe('1.0 GB');
	});

	it('should format exact MB boundary', () => {
		expect(formatBytes(1_048_576)).toBe('1.0 MB');
	});

	it('should format exact KB boundary', () => {
		expect(formatBytes(1_024)).toBe('1.0 KB');
	});
});

// ---------------------------------------------------------------------------
// testOllamaConnection
// ---------------------------------------------------------------------------

describe('testOllamaConnection', () => {
	const originalFetch = globalThis.fetch;

	afterEach(() => {
		globalThis.fetch = originalFetch;
	});

	it('should return ok: true on successful connection', async () => {
		mockFetch(() =>
			Promise.resolve(
				new Response(JSON.stringify({ models: [] }), {
					status: 200,
					headers: { 'x-ollama-version': '0.6.2' },
				}),
			),
		);

		const result = await testOllamaConnection('http://127.0.0.1:11434');
		expect(result.ok).toBe(true);
		if (result.ok) {
			expect(result.version).toBe('0.6.2');
		}
	});

	it('should return ok: true without version when header missing', async () => {
		mockFetch(() =>
			Promise.resolve(
				new Response(JSON.stringify({ models: [] }), { status: 200 }),
			),
		);

		const result = await testOllamaConnection('http://127.0.0.1:11434');
		expect(result.ok).toBe(true);
		if (result.ok) {
			expect(result.version).toBeUndefined();
		}
	});

	it('should return ok: false on network error', async () => {
		mockFetch(() => Promise.reject(new Error('Connection refused')));

		const result = await testOllamaConnection('http://127.0.0.1:11434');
		expect(result.ok).toBe(false);
		if (!result.ok) {
			expect(result.error).toContain('Connection refused');
		}
	});

	it('should return ok: false on non-200 status', async () => {
		mockFetch(() =>
			Promise.resolve(new Response('Not Found', { status: 404 })),
		);

		const result = await testOllamaConnection('http://127.0.0.1:11434');
		expect(result.ok).toBe(false);
		if (!result.ok) {
			expect(result.error).toContain('404');
		}
	});

	it('should return ok: false on timeout', async () => {
		mockFetch(
			(_url: unknown, init?: RequestInit) =>
				new Promise<Response>((_resolve, reject) => {
					// Listen for abort and reject when it fires
					if (init?.signal) {
						init.signal.addEventListener('abort', () => {
							reject(
								new DOMException(
									'The operation was aborted.',
									'AbortError',
								),
							);
						});
					}
				}),
		);

		const result = await testOllamaConnection(
			'http://127.0.0.1:11434',
			50,
		);
		expect(result.ok).toBe(false);
		if (!result.ok) {
			expect(result.error.toLowerCase()).toContain('timed out');
		}
	});

	it('should strip trailing slashes from URL', async () => {
		let capturedUrl = '';
		mockFetch((url: unknown) => {
			capturedUrl = typeof url === 'string' ? url : String(url);
			return Promise.resolve(
				new Response(JSON.stringify({ models: [] }), { status: 200 }),
			);
		});

		await testOllamaConnection('http://127.0.0.1:11434///');
		expect(capturedUrl).toBe('http://127.0.0.1:11434/api/tags');
	});

	it('should use default timeout of 5000ms', async () => {
		let capturedSignal: AbortSignal | undefined;
		mockFetch((_url: unknown, init?: RequestInit) => {
			capturedSignal = init?.signal ?? undefined;
			return Promise.resolve(
				new Response(JSON.stringify({ models: [] }), { status: 200 }),
			);
		});

		await testOllamaConnection('http://127.0.0.1:11434');
		expect(capturedSignal).toBeDefined();
	});
});

// ---------------------------------------------------------------------------
// listOllamaModels
// ---------------------------------------------------------------------------

describe('listOllamaModels', () => {
	const originalFetch = globalThis.fetch;

	afterEach(() => {
		globalThis.fetch = originalFetch;
	});

	it('should return model names and formatted sizes', async () => {
		mockFetch(() =>
			Promise.resolve(
				new Response(
					JSON.stringify({
						models: [
							{
								name: 'llama3.2:latest',
								size: 2_000_000_000,
								digest: 'abc',
								modified_at: '2024-01-01',
							},
							{
								name: 'codellama:7b',
								size: 3_800_000_000,
								digest: 'def',
								modified_at: '2024-01-02',
							},
						],
					}),
					{ status: 200 },
				),
			),
		);

		const models = await listOllamaModels('http://127.0.0.1:11434');
		expect(models).toHaveLength(2);
		expect(models[0].name).toBe('llama3.2:latest');
		expect(models[0].size).toBe('1.9 GB');
		expect(models[1].name).toBe('codellama:7b');
		expect(models[1].size).toBe('3.5 GB');
	});

	it('should return empty array on network error', async () => {
		mockFetch(() => Promise.reject(new Error('Connection refused')));

		const models = await listOllamaModels('http://127.0.0.1:11434');
		expect(models).toEqual([]);
	});

	it('should return empty array on non-200 status', async () => {
		mockFetch(() =>
			Promise.resolve(new Response('Error', { status: 500 })),
		);

		const models = await listOllamaModels('http://127.0.0.1:11434');
		expect(models).toEqual([]);
	});

	it('should return empty array on invalid JSON', async () => {
		mockFetch(() =>
			Promise.resolve(new Response('not json', { status: 200 })),
		);

		const models = await listOllamaModels('http://127.0.0.1:11434');
		expect(models).toEqual([]);
	});

	it('should return empty array when models field is missing', async () => {
		mockFetch(() =>
			Promise.resolve(
				new Response(JSON.stringify({}), { status: 200 }),
			),
		);

		const models = await listOllamaModels('http://127.0.0.1:11434');
		expect(models).toEqual([]);
	});

	it('should return frozen model objects', async () => {
		mockFetch(() =>
			Promise.resolve(
				new Response(
					JSON.stringify({
						models: [
							{
								name: 'test-model',
								size: 1_000_000,
							},
						],
					}),
					{ status: 200 },
				),
			),
		);

		const models = await listOllamaModels('http://127.0.0.1:11434');
		expect(models).toHaveLength(1);
		expect(Object.isFrozen(models[0])).toBe(true);
	});
});
