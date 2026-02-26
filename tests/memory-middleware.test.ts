import { describe, expect, it, mock } from 'bun:test';
import type { MemoryManager } from '../src/ai/memory/memory.js';
import { createMemoryMiddleware } from '../src/ai/memory/middleware.js';

function createMockMemoryManager(
	searchResults: Array<{
		entry: {
			id: string;
			text: string;
			embedding: number[];
			metadata: Record<string, string>;
			timestamp: number;
		};
		score: number;
	}> = [],
): MemoryManager {
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
		summarize: mock(async () => ({
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
		learningProfile: undefined,
		size: searchResults.length,
		isInitialized: true,
		isDirty: false,
		embeddingAgent: undefined,
	} as unknown as MemoryManager;
}

describe('createMemoryMiddleware', () => {
	it('returns a frozen object with enrichSystemPrompt and afterResponse', () => {
		const mw = createMemoryMiddleware(createMockMemoryManager());
		expect(mw.enrichSystemPrompt).toBeFunction();
		expect(mw.afterResponse).toBeFunction();
		expect(Object.isFrozen(mw)).toBe(true);
	});

	it('enrichSystemPrompt appends memory context to system prompt', async () => {
		const results = [
			{
				entry: {
					id: '1',
					text: 'Use bun test',
					embedding: [0.1],
					metadata: { topic: 'testing' },
					timestamp: Date.now(),
				},
				score: 0.9,
			},
		];
		const mw = createMemoryMiddleware(createMockMemoryManager(results));
		const enriched = await mw.enrichSystemPrompt({
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
		const mw = createMemoryMiddleware(createMockMemoryManager([]));
		const enriched = await mw.enrichSystemPrompt({
			userInput: 'hello',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toBe('You are helpful.');
	});

	it('enrichSystemPrompt gracefully handles search errors', async () => {
		const mm = createMockMemoryManager();
		(mm.search as ReturnType<typeof mock>).mockImplementation(async () => {
			throw new Error('embed failed');
		});
		const mw = createMemoryMiddleware(mm);
		const enriched = await mw.enrichSystemPrompt({
			userInput: 'hello',
			currentSystemPrompt: 'You are helpful.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toBe('You are helpful.');
	});

	it('afterResponse stores Q&A in memory', async () => {
		const mm = createMockMemoryManager();
		const mw = createMemoryMiddleware(mm, { storeTopic: 'chat' });
		await mw.afterResponse('What is Bun?', 'Bun is a JS runtime.');
		expect(mm.add).toHaveBeenCalledTimes(1);
		const callArgs = (mm.add as ReturnType<typeof mock>).mock.calls[0];
		expect(callArgs[0]).toContain('What is Bun?');
		expect(callArgs[0]).toContain('Bun is a JS runtime.');
		expect(callArgs[1]).toEqual(expect.objectContaining({ topic: 'chat' }));
	});

	it('afterResponse skips empty responses', async () => {
		const mm = createMockMemoryManager();
		const mw = createMemoryMiddleware(mm);
		await mw.afterResponse('hello', '');
		expect(mm.add).not.toHaveBeenCalled();
	});

	it('afterResponse skips error responses', async () => {
		const mm = createMockMemoryManager();
		const mw = createMemoryMiddleware(mm);
		await mw.afterResponse('hello', 'Error communicating with ACP');
		expect(mm.add).not.toHaveBeenCalled();
	});

	it('respects maxResults option', async () => {
		const mm = createMockMemoryManager([
			{
				entry: {
					id: '1',
					text: 'a',
					embedding: [0.1],
					metadata: { topic: 'x' },
					timestamp: Date.now(),
				},
				score: 0.9,
			},
		]);
		const mw = createMemoryMiddleware(mm, { maxResults: 3 });
		await mw.enrichSystemPrompt({
			userInput: 'test',
			currentSystemPrompt: '',
			conversationHistory: '',
			turn: 1,
		});
		expect(mm.search).toHaveBeenCalledWith('test', 3, undefined);
	});

	it('respects storeResponses: false', async () => {
		const mm = createMockMemoryManager();
		const mw = createMemoryMiddleware(mm, { storeResponses: false });
		await mw.afterResponse('hello', 'world');
		expect(mm.add).not.toHaveBeenCalled();
	});

	it('returns original prompt when memory not initialized', async () => {
		const mm = createMockMemoryManager();
		(mm as { isInitialized: boolean }).isInitialized = false;
		const mw = createMemoryMiddleware(mm);
		const enriched = await mw.enrichSystemPrompt({
			userInput: 'test',
			currentSystemPrompt: 'System prompt.',
			conversationHistory: '',
			turn: 1,
		});
		expect(enriched).toBe('System prompt.');
	});
});
