import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import { MessageList } from '../components/chat/message-list.js';
import type { OutputItem } from '../ink-types.js';

describe('MessageList', () => {
	test('user messages render with chevron prompt marker', () => {
		const items: OutputItem[] = [
			{ kind: 'message', role: 'user', text: 'Hello' },
		];

		const { lastFrame } = render(<MessageList items={items} />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('\u276F'); // ❯
		expect(frame).toContain('Hello');
	});

	test('assistant messages render through Markdown', () => {
		const items: OutputItem[] = [
			{ kind: 'message', role: 'assistant', text: 'This is **bold** text' },
		];

		const { lastFrame } = render(<MessageList items={items} />);
		const frame = lastFrame() ?? '';
		// Markdown component parses **bold** into bold text nodes
		expect(frame).toContain('bold');
		expect(frame).toContain('text');
		// Should NOT contain literal markdown markers
		expect(frame).not.toContain('**');
	});

	test('tool calls render inline', () => {
		const items: OutputItem[] = [
			{
				kind: 'tool-call',
				name: 'bash',
				args: '{"command":"ls"}',
				status: 'completed',
				duration: 120,
				summary: '3 files',
			},
		];

		const { lastFrame } = render(<MessageList items={items} />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Bash');
		expect(frame).toContain('ls');
	});

	test('renders error items', () => {
		const items: OutputItem[] = [{ kind: 'error', message: 'Something broke' }];

		const { lastFrame } = render(<MessageList items={items} />);
		expect(lastFrame()).toContain('Something broke');
	});

	test('renders info items', () => {
		const items: OutputItem[] = [{ kind: 'info', text: 'Library enabled' }];

		const { lastFrame } = render(<MessageList items={items} />);
		expect(lastFrame()).toContain('Library enabled');
	});
});
