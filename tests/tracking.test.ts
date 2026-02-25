import { describe, expect, it } from 'bun:test';
import {
	extractContentText,
	extractTokenUsage,
} from '../src/ai/acp/acp-results.js';
import type { ACPContentBlock } from '../src/ai/acp/types.js';

// ---------------------------------------------------------------------------
// extractTokenUsage
// ---------------------------------------------------------------------------

describe('extractTokenUsage', () => {
	it('returns undefined for undefined metadata', () => {
		expect(extractTokenUsage(undefined)).toBeUndefined();
	});

	it('returns undefined when metadata has no usage field', () => {
		expect(extractTokenUsage({ foo: 'bar' })).toBeUndefined();
	});

	it('returns undefined when usage is not an object', () => {
		expect(extractTokenUsage({ usage: 'not-an-object' })).toBeUndefined();
		expect(extractTokenUsage({ usage: 42 })).toBeUndefined();
		expect(extractTokenUsage({ usage: null })).toBeUndefined();
	});

	it('extracts snake_case token counts (OpenAI-compatible)', () => {
		const result = extractTokenUsage({
			usage: {
				prompt_tokens: 10,
				completion_tokens: 20,
				total_tokens: 30,
			},
		});

		expect(result).toEqual({
			promptTokens: 10,
			completionTokens: 20,
			totalTokens: 30,
		});
	});

	it('extracts camelCase token counts', () => {
		const result = extractTokenUsage({
			usage: {
				promptTokens: 15,
				completionTokens: 25,
				totalTokens: 40,
			},
		});

		expect(result).toEqual({
			promptTokens: 15,
			completionTokens: 25,
			totalTokens: 40,
		});
	});

	it('computes totalTokens when not provided', () => {
		const result = extractTokenUsage({
			usage: {
				prompt_tokens: 10,
				completion_tokens: 20,
			},
		});

		expect(result).toEqual({
			promptTokens: 10,
			completionTokens: 20,
			totalTokens: 30,
		});
	});

	it('prefers snake_case over camelCase when both present', () => {
		const result = extractTokenUsage({
			usage: {
				prompt_tokens: 10,
				completion_tokens: 20,
				total_tokens: 30,
				promptTokens: 99,
				completionTokens: 99,
				totalTokens: 99,
			},
		});

		expect(result).toEqual({
			promptTokens: 10,
			completionTokens: 20,
			totalTokens: 30,
		});
	});

	it('returns undefined when prompt_tokens is missing', () => {
		const result = extractTokenUsage({
			usage: {
				completion_tokens: 20,
				total_tokens: 30,
			},
		});

		expect(result).toBeUndefined();
	});

	it('returns undefined when completion_tokens is missing', () => {
		const result = extractTokenUsage({
			usage: {
				prompt_tokens: 10,
				total_tokens: 30,
			},
		});

		expect(result).toBeUndefined();
	});

	it('returns undefined when token values are not numbers', () => {
		const result = extractTokenUsage({
			usage: {
				prompt_tokens: '10',
				completion_tokens: '20',
			},
		});

		expect(result).toBeUndefined();
	});
});

// ---------------------------------------------------------------------------
// extractContentText
// ---------------------------------------------------------------------------

describe('extractContentText', () => {
	it('extracts text from a single text block', () => {
		const blocks: ACPContentBlock[] = [{ type: 'text', text: 'Hello world' }];
		expect(extractContentText(blocks)).toBe('Hello world');
	});

	it('concatenates multiple text blocks', () => {
		const blocks: ACPContentBlock[] = [
			{ type: 'text', text: 'Hello' },
			{ type: 'text', text: ' world' },
		];
		expect(extractContentText(blocks)).toBe('Hello world');
	});

	it('returns empty string for empty array', () => {
		expect(extractContentText([])).toBe('');
	});

	it('ignores data blocks', () => {
		const blocks: ACPContentBlock[] = [
			{ type: 'text', text: 'before' },
			{ type: 'data', data: { key: 'value' } },
			{ type: 'text', text: 'after' },
		];
		expect(extractContentText(blocks)).toBe('beforeafter');
	});

	it('returns empty string when only data blocks are present', () => {
		const blocks: ACPContentBlock[] = [
			{ type: 'data', data: { key: 'value' } },
		];
		expect(extractContentText(blocks)).toBe('');
	});
});
