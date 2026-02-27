import { describe, expect, test } from 'bun:test';
import { deriveToolSummary } from '../hooks/use-agentic-loop.js';

describe('deriveToolSummary', () => {
	test('counts lines for multiline output', () => {
		const output = 'line1\nline2\nline3';
		expect(deriveToolSummary('vfs_read', output)).toBe('3 lines');
	});

	test('returns char count for long single-line output', () => {
		const output = 'x'.repeat(150);
		expect(deriveToolSummary('vfs_read', output)).toContain('150');
	});

	test('returns undefined for empty output', () => {
		expect(deriveToolSummary('vfs_read', '')).toBeUndefined();
	});
});
