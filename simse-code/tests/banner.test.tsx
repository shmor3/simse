import { render } from 'ink-testing-library';
import React from 'react';
import { describe, expect, test } from 'bun:test';
import { Banner } from '../components/layout/banner.js';

describe('Banner', () => {
	test('renders app name', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				server="claude"
			/>,
		);
		expect(lastFrame()).toContain('simse');
	});

	test('renders server name', () => {
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

	test('renders service counts', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				server="claude"
				noteCount={42}
				toolCount={7}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('42');
		expect(frame).toContain('7');
	});
});
