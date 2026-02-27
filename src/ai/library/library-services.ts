// ---------------------------------------------------------------------------
// Memory Middleware
// ---------------------------------------------------------------------------
//
// Automatically enriches every agentic-loop turn with relevant context
// from memory, and stores responses back into memory after the loop.
// ---------------------------------------------------------------------------

import type { Logger } from '../../logger.js';
import type { MemoryManager } from './library.js';
import {
	formatMemoryContext,
	type PromptInjectionOptions,
} from './prompt-injection.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface MiddlewareContext {
	readonly userInput: string;
	readonly currentSystemPrompt: string;
	readonly conversationHistory: string;
	readonly turn: number;
}

export interface MemoryMiddleware {
	readonly enrichSystemPrompt: (context: MiddlewareContext) => Promise<string>;
	readonly afterResponse: (
		userInput: string,
		response: string,
	) => Promise<void>;
}

export interface MemoryMiddlewareOptions {
	/** Maximum results to retrieve per turn. Defaults to `5`. */
	readonly maxResults?: number;
	/** Minimum relevance score for inclusion. */
	readonly minScore?: number;
	/** Prompt injection formatting options. */
	readonly format?: PromptInjectionOptions;
	/** Topic to tag stored Q&A pairs with. Defaults to `'conversation'`. */
	readonly storeTopic?: string;
	/** Whether to store Q&A pairs in memory. Defaults to `true`. */
	readonly storeResponses?: boolean;
	/** Optional logger for debug/warning output. */
	readonly logger?: Logger;
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

export function createMemoryMiddleware(
	memoryManager: MemoryManager,
	options?: MemoryMiddlewareOptions,
): MemoryMiddleware {
	const maxResults = options?.maxResults ?? 5;
	const minScore = options?.minScore;
	const storeTopic = options?.storeTopic ?? 'conversation';
	const storeResponses = options?.storeResponses ?? true;
	const formatOptions = options?.format;
	const logger = options?.logger;

	const enrichSystemPrompt = async (
		context: MiddlewareContext,
	): Promise<string> => {
		if (!memoryManager.isInitialized || memoryManager.size === 0) {
			return context.currentSystemPrompt;
		}

		try {
			const results = await memoryManager.search(
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
				'Memory middleware: search failed, continuing without context',
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
		if (!memoryManager.isInitialized) return;

		try {
			const text = `Q: ${userInput}\nA: ${response}`;
			await memoryManager.add(text, { topic: storeTopic });
		} catch (err) {
			logger?.warn('Memory middleware: failed to store response', {
				error: err instanceof Error ? err.message : String(err),
			});
		}
	};

	return Object.freeze({ enrichSystemPrompt, afterResponse });
}
