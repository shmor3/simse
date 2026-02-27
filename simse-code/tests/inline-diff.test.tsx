import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { InlineDiff } from '../components/chat/inline-diff.js';

describe('InlineDiff', () => {
	test('renders additions and removals', () => {
		const lines = [
			{ type: 'context' as const, content: 'const x = 1;', oldLineNumber: 1, newLineNumber: 1 },
			{ type: 'remove' as const, content: 'const y = 2;', oldLineNumber: 2 },
			{ type: 'add' as const, content: 'const y = 3;', newLineNumber: 2 },
		];

		const { lastFrame } = render(
			<InlineDiff path="/src/main.ts" lines={lines} />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('/src/main.ts');
		expect(frame).toContain('const y = 2;');
		expect(frame).toContain('const y = 3;');
	});

	test('renders empty diff gracefully', () => {
		const { lastFrame } = render(
			<InlineDiff path="/src/main.ts" lines={[]} />,
		);
		expect(lastFrame()).toContain('No changes');
	});
});
