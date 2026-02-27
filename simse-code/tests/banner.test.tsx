import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import { Banner } from '../components/layout/banner.js';

describe('Banner', () => {
	test('renders title line with version', () => {
		const { lastFrame } = render(
			<Banner version="1.0.0" workDir="/projects/test" dataDir="~/.simse" />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('simse-code');
		expect(frame).toContain('1.0.0');
		// Uses horizontal rule, not rounded box
		expect(frame).toContain('─');
	});

	test('renders mascot lines', () => {
		const { lastFrame } = render(
			<Banner version="1.0.0" workDir="/projects/test" dataDir="~/.simse" />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('╭──╮');
		expect(frame).toContain('╰─╮│');
		expect(frame).toContain('╰╯');
	});

	test('renders two-column layout with divider', () => {
		const { lastFrame } = render(
			<Banner version="1.0.0" workDir="/projects/test" dataDir="~/.simse" />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('│');
		expect(frame).toContain('Tips for getting started');
		expect(frame).toContain('Recent activity');
	});

	test('renders server and model info', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				server="ollama"
				model="llama3"
			/>,
		);
		expect(lastFrame()).toContain('ollama \u00b7 llama3');
	});

	test('renders working directory', () => {
		const { lastFrame } = render(
			<Banner version="1.0.0" workDir="/projects/test" dataDir="~/.simse" />,
		);
		expect(lastFrame()).toContain('/projects/test');
	});

	test('renders tips and hint lines below banner', () => {
		const { lastFrame } = render(
			<Banner version="1.0.0" workDir="/projects/test" dataDir="~/.simse" />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('/help');
		expect(frame).toContain('▢');
		expect(frame).toContain('? for shortcuts');
	});

	test('renders custom tips', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				tips={['Custom tip one', 'Custom tip two']}
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('Custom tip one');
		expect(frame).toContain('Custom tip two');
	});
});
