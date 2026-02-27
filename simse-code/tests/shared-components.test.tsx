import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { ThinkingSpinner } from '../components/shared/spinner.js';
import { ErrorBox } from '../components/shared/error-box.js';
import { Badge } from '../components/shared/badge.js';

describe('ThinkingSpinner', () => {
	test('renders with default label', () => {
		const { lastFrame } = render(<ThinkingSpinner />);
		expect(lastFrame()).toBeDefined();
	});

	test('renders with custom label', () => {
		const { lastFrame } = render(<ThinkingSpinner label="Searching..." />);
		expect(lastFrame()).toContain('Searching...');
	});
});

describe('ErrorBox', () => {
	test('renders error message', () => {
		const { lastFrame } = render(<ErrorBox message="Something went wrong" />);
		expect(lastFrame()).toContain('Something went wrong');
	});
});

describe('Badge', () => {
	test('renders PLAN badge', () => {
		const { lastFrame } = render(<Badge label="PLAN" />);
		expect(lastFrame()).toContain('PLAN');
	});

	test('renders VERBOSE badge', () => {
		const { lastFrame } = render(<Badge label="VERBOSE" />);
		expect(lastFrame()).toContain('VERBOSE');
	});
});
