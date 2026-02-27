import { describe, expect, it, mock } from 'bun:test';
import { createLibraryServices } from '../src/ai/library/library-services.js';
import type { Library } from '../src/ai/library/library.js';

function createMockLibrary(): Library {
	return {
		isInitialized: true,
		size: 5,
		search: mock(async () => [
			{
				volume: {
					id: '1',
					text: 'relevant context',
					embedding: [0.1],
					metadata: { topic: 'test' },
					timestamp: Date.now(),
				},
				score: 0.9,
			},
		]),
		add: mock(async () => 'new-id'),
	} as unknown as Library;
}

describe('LibraryServices', () => {
	it('enrichSystemPrompt injects library context', async () => {
		const library = createMockLibrary();
		const services = createLibraryServices(library);
		const result = await services.enrichSystemPrompt({
			userInput: 'tell me about databases',
			currentSystemPrompt: 'You are a helper.',
			conversationHistory: '',
			turn: 1,
		});
		expect(result).toContain('relevant context');
	});

	it('afterResponse stores Q&A in library when no circulationDesk', async () => {
		const library = createMockLibrary();
		const services = createLibraryServices(library);
		await services.afterResponse('What is X?', 'X is a thing.');
		expect(library.add).toHaveBeenCalled();
	});

	it('afterResponse with circulationDesk enqueues extraction instead of direct add', async () => {
		const library = createMockLibrary();
		const enqueueMock = mock(() => {});
		const services = createLibraryServices(library, {
			circulationDesk: {
				enqueueExtraction: enqueueMock,
				enqueueCompendium: mock(() => {}),
				enqueueReorganization: mock(() => {}),
				drain: mock(async () => {}),
				flush: mock(async () => {}),
				dispose: mock(() => {}),
				pending: 0,
				processing: false,
			},
		});
		await services.afterResponse('What is X?', 'X is a thing.');
		expect(enqueueMock).toHaveBeenCalled();
		// When circulationDesk is present, should NOT do direct add
		expect(library.add).not.toHaveBeenCalled();
	});

	it('afterResponse skips error responses', async () => {
		const library = createMockLibrary();
		const services = createLibraryServices(library);
		await services.afterResponse('What?', 'Error communicating with server');
		expect(library.add).not.toHaveBeenCalled();
	});

	it('afterResponse skips empty responses', async () => {
		const library = createMockLibrary();
		const services = createLibraryServices(library);
		await services.afterResponse('What?', '');
		expect(library.add).not.toHaveBeenCalled();
	});
});
