import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import React from 'react';
import { formatTokens, StatusBar } from '../components/layout/status-bar.js';

describe('formatTokens', () => {
	test('formats tokens below 1000 without suffix', () => {
		expect(formatTokens(500)).toBe('500 tokens');
	});

	test('formats tokens at 1000 with k suffix', () => {
		expect(formatTokens(1000)).toBe('1.0k tokens');
	});

	test('formats tokens above 1000 with k suffix', () => {
		expect(formatTokens(12345)).toBe('12.3k tokens');
	});
});

describe('StatusBar', () => {
	test('renders server and model with colon separator', () => {
		const { lastFrame } = render(
			<StatusBar server="claude" model="opus-4" tokens={1234} cost="$0.03" />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('claude:opus-4');
	});

	test('renders token count with k suffix', () => {
		const { lastFrame } = render(
			<StatusBar server="claude" model="opus-4" tokens={12345} cost="$0.03" />,
		);
		expect(lastFrame()).toContain('12.3k tokens');
	});

	test('renders cost', () => {
		const { lastFrame } = render(
			<StatusBar server="claude" model="opus-4" tokens={1234} cost="$0.03" />,
		);
		expect(lastFrame()).toContain('$0.03');
	});

	test('shows fallback text when no server or model', () => {
		const { lastFrame } = render(<StatusBar />);
		expect(lastFrame()).toContain('no server configured');
	});

	test('renders mode badges on the right', () => {
		const { lastFrame } = render(
			<StatusBar
				server="claude"
				model="opus-4"
				tokens={0}
				cost="$0.00"
				planMode
				verbose
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('PLAN');
		expect(frame).toContain('VERBOSE');
	});
});
