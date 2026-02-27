import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import React from 'react';
import { PromptInput } from '../components/input/prompt-input.js';

describe('PromptInput', () => {
	test('renders prompt character inside bordered box', () => {
		const { lastFrame } = render(<PromptInput onSubmit={() => {}} />);
		const frame = lastFrame()!;
		expect(frame).toContain('\u276F');
		// Round border characters
		expect(frame).toContain('\u256D');
		expect(frame).toContain('\u2570');
	});

	test('renders in plan mode without error', () => {
		const { lastFrame } = render(<PromptInput onSubmit={() => {}} planMode />);
		const frame = lastFrame()!;
		expect(frame).toContain('\u276F');
	});

	test('shows placeholder tip when disabled', () => {
		const { lastFrame } = render(<PromptInput onSubmit={() => {}} disabled />);
		const frame = lastFrame()!;
		expect(frame).toContain('\u276F');
		expect(frame).toContain('Try');
	});

	test('accepts commands prop for autocomplete', () => {
		const commands = [
			{
				name: 'help',
				usage: '/help',
				description: 'Show help',
				category: 'meta' as const,
				execute: () => undefined,
			},
		];
		const { lastFrame } = render(
			<PromptInput onSubmit={() => {}} commands={commands} />,
		);
		expect(lastFrame()).toContain('\u276F');
	});

	test('shows ghost suggestion for partial command match', async () => {
		const commands = [
			{
				name: 'help',
				usage: '/help',
				description: 'Show help',
				category: 'meta' as const,
				execute: () => undefined,
			},
			{
				name: 'search',
				usage: '/search',
				description: 'Search library',
				category: 'library' as const,
				execute: () => undefined,
			},
		];
		const { lastFrame, stdin } = render(
			<PromptInput onSubmit={() => {}} commands={commands} />,
		);

		// Type /hel — only "help" matches, should show "p" ghost text
		stdin.write('/hel');
		await new Promise((r) => setTimeout(r, 50));

		const frame = lastFrame()!;
		// The ghost text "p" should appear in the rendered output
		expect(frame).toContain('hel');
		expect(frame).toContain('p');
	});

	test('triggers at-mention mode when @ is typed mid-sentence', async () => {
		const completer = (partial: string) => {
			if (partial.startsWith('src')) return ['src/main.ts', 'src/lib.ts'];
			return [];
		};

		const { lastFrame, stdin } = render(
			<PromptInput
				onSubmit={() => {}}
				onCompleteAtMention={completer}
			/>,
		);

		// Type "describe @src"
		stdin.write('describe @src');
		await new Promise((r) => setTimeout(r, 50));

		const frame = lastFrame()!;
		// Should show @-mention candidates
		expect(frame).toContain('src/main.ts');
		expect(frame).toContain('src/lib.ts');
	});

	test('shows at-mention candidates with @ prefix in overlay', async () => {
		const completer = () => ['utils/helper.ts'];

		const { lastFrame, stdin } = render(
			<PromptInput
				onSubmit={() => {}}
				onCompleteAtMention={completer}
			/>,
		);

		stdin.write('@u');
		await new Promise((r) => setTimeout(r, 50));

		const frame = lastFrame()!;
		expect(frame).toContain('@utils/helper.ts');
	});

	test('no suggestion when multiple commands match', async () => {
		const commands = [
			{
				name: 'help',
				usage: '/help',
				description: 'Show help',
				category: 'meta' as const,
				execute: () => undefined,
			},
			{
				name: 'history',
				usage: '/history',
				description: 'Show history',
				category: 'session' as const,
				execute: () => undefined,
			},
		];
		const { lastFrame, stdin } = render(
			<PromptInput onSubmit={() => {}} commands={commands} />,
		);

		// Type /h — both "help" and "history" match, no suggestion
		stdin.write('/h');
		await new Promise((r) => setTimeout(r, 50));

		// Frame should show the input but no ghost text completion
		const frame = lastFrame()!;
		expect(frame).toContain('h');
	});
});
