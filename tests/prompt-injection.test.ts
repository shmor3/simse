import { describe, expect, it } from 'bun:test';
import { formatMemoryContext } from '../src/ai/memory/prompt-injection.js';
import type { SearchResult } from '../src/ai/memory/types.js';

function makeResult(
	text: string,
	topic: string,
	score: number,
	ageMs = 0,
): SearchResult {
	return {
		entry: {
			id: `id-${text}`,
			text,
			embedding: [0.1, 0.2],
			metadata: { topic },
			timestamp: Date.now() - ageMs,
		},
		score,
	};
}

describe('formatMemoryContext', () => {
	it('returns empty string for empty results', () => {
		expect(formatMemoryContext([])).toBe('');
	});

	it('formats results as structured XML tags by default', () => {
		const results = [makeResult('Use bun test', 'testing', 0.92)];
		const output = formatMemoryContext(results);
		expect(output).toContain('<memory-context>');
		expect(output).toContain('</memory-context>');
		expect(output).toContain('topic="testing"');
		expect(output).toContain('relevance="0.92"');
		expect(output).toContain('Use bun test');
	});

	it('filters results below minScore', () => {
		const results = [makeResult('high', 'a', 0.9), makeResult('low', 'b', 0.3)];
		const output = formatMemoryContext(results, { minScore: 0.5 });
		expect(output).toContain('high');
		expect(output).not.toContain('low');
	});

	it('limits to maxResults', () => {
		const results = [
			makeResult('one', 'a', 0.9),
			makeResult('two', 'b', 0.8),
			makeResult('three', 'c', 0.7),
		];
		const output = formatMemoryContext(results, { maxResults: 2 });
		expect(output).toContain('one');
		expect(output).toContain('two');
		expect(output).not.toContain('three');
	});

	it('truncates to maxChars', () => {
		const longText = 'x'.repeat(5000);
		const results = [makeResult(longText, 'a', 0.9)];
		const output = formatMemoryContext(results, { maxChars: 200 });
		expect(output.length).toBeLessThanOrEqual(250); // tag overhead
	});

	it('uses custom tag name', () => {
		const results = [makeResult('hello', 'a', 0.9)];
		const output = formatMemoryContext(results, { tag: 'context' });
		expect(output).toContain('<context>');
		expect(output).toContain('</context>');
	});

	it('formats as natural text when format is natural', () => {
		const results = [makeResult('Use bun test', 'testing', 0.92)];
		const output = formatMemoryContext(results, { format: 'natural' });
		expect(output).not.toContain('<memory-context>');
		expect(output).toContain('Relevant context from memory:');
		expect(output).toContain('Use bun test');
	});

	it('formats relative age for entries', () => {
		const results = [makeResult('old entry', 'a', 0.9, 3_600_000)]; // 1h ago
		const output = formatMemoryContext(results);
		expect(output).toContain('age="1h"');
	});

	it('returns empty string when all results filtered by minScore', () => {
		const results = [makeResult('low', 'a', 0.2)];
		const output = formatMemoryContext(results, { minScore: 0.5 });
		expect(output).toBe('');
	});

	it('uses uncategorized for entries without topic metadata', () => {
		const result: SearchResult = {
			entry: {
				id: 'no-topic',
				text: 'no topic here',
				embedding: [0.1],
				metadata: {},
				timestamp: Date.now(),
			},
			score: 0.8,
		};
		const output = formatMemoryContext([result]);
		expect(output).toContain('topic="uncategorized"');
	});
});
