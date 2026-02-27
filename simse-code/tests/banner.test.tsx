import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import { Banner } from '../components/layout/banner.js';

describe('Banner', () => {
	test('renders version info', () => {
		const { lastFrame } = render(
			<Banner version="1.0.0" workDir="/projects/test" dataDir="~/.simse" />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('simse-code');
		expect(frame).toContain('1.0.0');
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
		const frame = lastFrame()!;
		expect(frame).toContain('ollama: llama3');
	});

	test('renders server only when no model', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				server="ollama"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('ollama');
	});

	test('renders model only when no server', () => {
		const { lastFrame } = render(
			<Banner
				version="1.0.0"
				workDir="/projects/test"
				dataDir="~/.simse"
				model="llama3"
			/>,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('llama3');
	});

	test('renders working directory', () => {
		const { lastFrame } = render(
			<Banner version="1.0.0" workDir="/projects/test" dataDir="~/.simse" />,
		);
		expect(lastFrame()).toContain('/projects/test');
	});

	test('renders tips text', () => {
		const { lastFrame } = render(
			<Banner version="1.0.0" workDir="/projects/test" dataDir="~/.simse" />,
		);
		const frame = lastFrame()!;
		expect(frame).toContain('/help');
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
