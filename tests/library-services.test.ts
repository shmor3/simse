import { describe, expect, it, mock } from 'bun:test';
import type { Library } from '../src/ai/library/library.js';
import { createLibraryServices } from '../src/ai/library/library-services.js';

function createMockLibrary(
	searchResults: Array<{
		volume: {
			id: string;
			text: string;
			embedding: number[];
			metadata: Record<string, string>;
			timestamp: number;
		};
		score: number;
	}> = [],
): Library {
	return {
		initialize: mock(async () => {}),
		dispose: mock(async () => {}),
		add: mock(async () => 'mock-id'),
		addBatch: mock(async () => []),
		search: mock(async () => searchResults),
		textSearch: mock(() => []),
		filterByMetadata: mock(() => []),
		filterByDateRange: mock(() => []),
		advancedSearch: mock(async () => []),
		query: mock(async () => []),
		getById: mock(() => undefined),
		getAll: mock(() => []),
		getTopics: mock(() => []),
		filterByTopic: mock(() => []),
		recommend: mock(async () => []),
		findDuplicates: mock(() => []),
		checkDuplicate: mock(async () => ({ isDuplicate: false })),
		compendium: mock(async () => ({
			summaryId: '',
			summaryText: '',
			sourceIds: [],
			deletedOriginals: false,
		})),
		setTextGenerator: mock(() => {}),
		recordFeedback: mock(() => {}),
		delete: mock(async () => false),
		deleteBatch: mock(async () => 0),
		clear: mock(async () => {}),
		patronProfile: undefined,
		size: searchResults.length,
		isInitialized: true,
		isDirty: false,
		embeddingAgent: undefined,
	} as unknown as Library;
}

describe('createLibraryServices', () => {
	it('returns a frozen object with enrichSystemPrompt and afterResponse', () => {
		const svc = createLibraryServices(createMockLibrary());
		expect(svc.enrichSystemPrompt).toBeFunction();
		expect(svc.afterResponse).toBeFunction();
		expect(Object.isFrozen(svc)).toBe(true);
	});

	it('enrichSystemPrompt appends library context to system prompt', async () => {
		const results = [
			{
				volume: {
					id: '1',
					text: 'Use bun test',
					embedding: [0.1],
					metadata: { topic: 'testing' },
					timestamp: Date.now(),
				},
				score: 0.9,
			},
		];
		const svc = createLibraryServices(createMockLibrary(results));
		const enriched = await svc.enrichSystemPrompt({
			userInput: 'how do I test?',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toContain('You are helpful.');
		expect(enriched).toContain('<memory-context>');
		expect(enriched).toContain('Use bun test');
	});

	it('enrichSystemPrompt returns original prompt when no results', async () => {
		const svc = createLibraryServices(createMockLibrary([]));
		const enriched = await svc.enrichSystemPrompt({
			userInput: 'hello',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toBe('You are helpful.');
	});

	it('enrichSystemPrompt gracefully handles search errors', async () => {
		const lib = createMockLibrary();
		(lib.search as ReturnType<typeof mock>).mockImplementation(async () => {
			throw new Error('embed failed');
		});
		const svc = createLibraryServices(lib);
		const enriched = await svc.enrichSystemPrompt({
			userInput: 'hello',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toBe('You are helpful.');
	});

	it('afterResponse stores Q&A in library', async () => {
		const lib = createMockLibrary();
		const svc = createLibraryServices(lib, { storeTopic: 'chat' });
		await svc.afterResponse('What is Bun?', 'Bun is a JS runtime.');
		expect(lib.add).toHaveBeenCalledTimes(1);
		const callArgs = (lib.add as ReturnType<typeof mock>).mock.calls[0];
		expect(callArgs[0]).toContain('What is Bun?');
		expect(callArgs[0]).toContain('Bun is a JS runtime.');
		expect(callArgs[1]).toEqual(expect.objectContaining({ topic: 'chat' }));
	});

	it('afterResponse skips empty responses', async () => {
		const lib = createMockLibrary();
		const svc = createLibraryServices(lib);
		await svc.afterResponse('hello', '');
		expect(lib.add).not.toHaveBeenCalled();
	});

	it('afterResponse skips error responses', async () => {
		const lib = createMockLibrary();
		const svc = createLibraryServices(lib);
		await svc.afterResponse('hello', 'Error communicating with ACP');
		expect(lib.add).not.toHaveBeenCalled();
	});

	it('respects maxResults option', async () => {
		const lib = createMockLibrary([
			{
				volume: {
					id: '1',
					text: 'a',
					embedding: [0.1],
					metadata: { topic: 'x' },
					timestamp: Date.now(),
				},
				score: 0.9,
			},
		]);
		const svc = createLibraryServices(lib, { maxResults: 3 });
		await svc.enrichSystemPrompt({
			userInput: 'test',
			currentSystemPrompt: '',
			conversationHistory: '',
			turn: 1,
		});
		expect(lib.search).toHaveBeenCalledWith('test', 3, undefined);
	});

	it('respects storeResponses: false', async () => {
		const lib = createMockLibrary();
		const svc = createLibraryServices(lib, { storeResponses: false });
		await svc.afterResponse('hello', 'world');
		expect(lib.add).not.toHaveBeenCalled();
	});

	it('returns original prompt when library not initialized', async () => {
		const lib = createMockLibrary();
		(lib as { isInitialized: boolean }).isInitialized = false;
		const svc = createLibraryServices(lib);
		const enriched = await svc.enrichSystemPrompt({
			userInput: 'test',
			currentSystemPrompt: 'System prompt.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toBe('System prompt.');
	});
});
