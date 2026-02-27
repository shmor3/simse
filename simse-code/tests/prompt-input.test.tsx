import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { PromptInput } from '../components/input/prompt-input.js';

describe('PromptInput', () => {
	test('renders prompt character', () => {
		const { lastFrame } = render(
			<PromptInput onSubmit={() => {}} />,
		);
		expect(lastFrame()).toContain('>');
	});

	test('shows plan mode badge when active', () => {
		const { lastFrame } = render(
			<PromptInput onSubmit={() => {}} planMode />,
		);
		expect(lastFrame()).toContain('PLAN');
	});

	test('renders when disabled', () => {
		const { lastFrame } = render(
			<PromptInput onSubmit={() => {}} disabled />,
		);
		expect(lastFrame()).toBeDefined();
	});
});
