import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Banner } from '../components/layout/banner.js';

describe('Banner', () => {
	test('renders title with version', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
			/>,
		);
		expect(lastFrame()).toContain('simse-code v1.0.0');
	});

	test('renders server name in left column', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				server="claude"
			/>,
		);
		expect(lastFrame()).toContain('claude');
	});

	test('renders mascot', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('╭──╮');
		expect(frame).toContain('╰─╮│');
		expect(frame).toContain('╰╯');
	});

	test('renders tips section', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('Tips for getting started');
		expect(frame).toContain('/help');
	});

	test('renders column separator', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
			/>,
		);
		expect(lastFrame()).toContain('│');
	});

	test('renders hint lines', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('▢');
		expect(frame).toContain('shortcuts');
	});
});
