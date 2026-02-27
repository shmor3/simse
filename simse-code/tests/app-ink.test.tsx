import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { App } from '../app-ink.js';

describe('App', () => {
	test('renders without crashing', () => {
		const { lastFrame } = render(<App dataDir="/test" />);
		expect(lastFrame()).toBeDefined();
	});

	test('renders the prompt character', () => {
		const { lastFrame } = render(<App dataDir="/test" />);
		expect(lastFrame()).toContain('>');
	});

	test('renders the banner', () => {
		const { lastFrame } = render(<App dataDir="/test" serverName="claude" />);
		expect(lastFrame()).toContain('simse');
	});
});
