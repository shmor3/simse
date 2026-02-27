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

	test('shows placeholder when disabled', () => {
		const { lastFrame } = render(<PromptInput onSubmit={() => {}} disabled />);
		const frame = lastFrame()!;
		expect(frame).toContain('\u276F');
		expect(frame).toContain('Send a message');
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
});
