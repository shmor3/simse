import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { StatusBar } from '../components/layout/status-bar.js';

describe('StatusBar', () => {
	test('renders server and model', () => {
		const { lastFrame } = render(
			<StatusBar
				server="claude"
				model="opus-4"
				tokens={1234}
				cost="$0.03"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('claude');
		expect(frame).toContain('opus-4');
	});

	test('renders token count', () => {
		const { lastFrame } = render(
			<StatusBar
				server="claude"
				model="opus-4"
				tokens={1234}
				cost="$0.03"
			/>,
		);
		expect(lastFrame()).toContain('1234');
	});

	test('renders badges when modes active', () => {
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
