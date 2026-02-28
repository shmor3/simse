// ---------------------------------------------------------------------------
// Library Services (was Memory Middleware)
// ---------------------------------------------------------------------------
//
// Automatically enriches every agentic-loop turn with relevant context
// from the library, and stores responses back into the library after the loop.
// ---------------------------------------------------------------------------

import type { Logger } from '../../logger.js';
import type { LibrarianRegistry } from './librarian-registry.js';
import type { Library } from './library.js';
import {
	formatMemoryContext,
	type PromptInjectionOptions,
} from './prompt-injection.js';
import type { CirculationDesk } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface LibraryContext {
	readonly userInput: string;
	readonly currentSystemPrompt: string;
	readonly conversationHistory: string;
	readonly turn: number;
}

export interface LibraryServices {
	readonly enrichSystemPrompt: (context: LibraryContext) => Promise<string>;
	readonly afterResponse: (
		userInput: string,
		response: string,
	) => Promise<void>;
}

export interface LibraryServicesOptions {
	/** Maximum results to retrieve per turn. Defaults to `5`. */
	readonly maxResults?: number;
	/** Minimum relevance score for inclusion. */
	readonly minScore?: number;
	/** Prompt injection formatting options. */
	readonly format?: PromptInjectionOptions;
	/** Topic to tag stored Q&A pairs with. Defaults to `'conversation'`. */
	readonly storeTopic?: string;
	/** Whether to store Q&A pairs in library. Defaults to `true`. */
	readonly storeResponses?: boolean;
	/** Optional logger for debug/warning output. */
	readonly logger?: Logger;
	/** Optional CirculationDesk for async background extraction instead of direct library.add(). */
	readonly circulationDesk?: CirculationDesk;
	/** Optional LibrarianRegistry for multi-librarian routing via the CirculationDesk. */
	readonly registry?: LibrarianRegistry;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isErrorResponse(response: string): boolean {
	const lower = response.toLowerCase();
	return (
		lower.startsWith('error') ||
		lower.includes('error communicating') ||
		lower.includes('failed to')
	);
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createLibraryServices(
	library: Library,
	options?: LibraryServicesOptions,
): LibraryServices {
	const maxResults = options?.maxResults ?? 5;
	const minScore = options?.minScore;
	const storeTopic = options?.storeTopic ?? 'conversation';
	const storeResponses = options?.storeResponses ?? true;
	const formatOptions = options?.format;
	const logger = options?.logger;
	const circulationDesk = options?.circulationDesk;

	const enrichSystemPrompt = async (
		context: LibraryContext,
	): Promise<string> => {
		if (!library.isInitialized || library.size === 0) {
			return context.currentSystemPrompt;
		}

		try {
			const results = await library.search(
				context.userInput,
				maxResults,
				minScore,
			);

			if (results.length === 0) {
				return context.currentSystemPrompt;
			}

			const memoryBlock = formatMemoryContext(results, {
				...formatOptions,
				maxResults,
				minScore,
			});

			if (!memoryBlock) {
				return context.currentSystemPrompt;
			}

			return `${context.currentSystemPrompt}\n\n${memoryBlock}`;
		} catch (err) {
			logger?.warn(
				'Library services: search failed, continuing without context',
				{
					error: err instanceof Error ? err.message : String(err),
				},
			);
			return context.currentSystemPrompt;
		}
	};

	const afterResponse = async (
		userInput: string,
		response: string,
	): Promise<void> => {
		if (!storeResponses) return;
		if (!response || response.trim().length === 0) return;
		if (isErrorResponse(response)) return;
		if (!library.isInitialized) return;

		try {
			if (circulationDesk) {
				circulationDesk.enqueueExtraction({ userInput, response });
			} else {
				const text = `Q: ${userInput}\nA: ${response}`;
				await library.add(text, { topic: storeTopic });
			}
		} catch (err) {
			logger?.warn('Library services: failed to store response', {
				error: err instanceof Error ? err.message : String(err),
			});
		}
	};

	return Object.freeze({ enrichSystemPrompt, afterResponse });
}
