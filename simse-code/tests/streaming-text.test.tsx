import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { StreamingText } from '../components/chat/streaming-text.js';

describe('StreamingText', () => {
	test('renders accumulated text', () => {
		const { lastFrame } = render(
			<StreamingText text="Hello, world!" />,
		);
		expect(lastFrame()).toContain('Hello, world!');
	});

	test('renders empty text without crashing', () => {
		const { lastFrame } = render(
			<StreamingText text="" />,
		);
		expect(lastFrame()).toBeDefined();
	});
});
