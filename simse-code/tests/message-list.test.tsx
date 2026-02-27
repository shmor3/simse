import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { MessageList } from '../components/chat/message-list.js';
import type { OutputItem } from '../ink-types.js';

describe('MessageList', () => {
	test('renders user and assistant messages', () => {
		const items: OutputItem[] = [
			{ kind: 'message', role: 'user', text: 'Hello' },
			{ kind: 'message', role: 'assistant', text: 'Hi there!' },
		];

		const { lastFrame } = render(<MessageList items={items} />);
		const frame = lastFrame()!;
		expect(frame).toContain('Hello');
		expect(frame).toContain('Hi there!');
	});

	test('renders error items', () => {
		const items: OutputItem[] = [
			{ kind: 'error', message: 'Something broke' },
		];

		const { lastFrame } = render(<MessageList items={items} />);
		expect(lastFrame()).toContain('Something broke');
	});

	test('renders info items', () => {
		const items: OutputItem[] = [
			{ kind: 'info', text: 'Library enabled' },
		];

		const { lastFrame } = render(<MessageList items={items} />);
		expect(lastFrame()).toContain('Library enabled');
	});
});
