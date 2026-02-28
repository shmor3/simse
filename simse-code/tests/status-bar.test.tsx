import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
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
	test('renders default hints without interrupt when idle', () => {
		const { lastFrame } = render(<StatusBar />);
		const frame = lastFrame() ?? '';
		expect(frame).not.toContain('esc to interrupt');
		expect(frame).toContain('? for shortcuts');
	});

	test('shows esc to interrupt when processing', () => {
		const { lastFrame } = render(<StatusBar isProcessing />);
		expect(lastFrame()).toContain('esc to interrupt');
	});

	test('shows bypass permissions hint when enabled', () => {
		const { lastFrame } = render(<StatusBar bypassPermissions />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('bypass permissions on');
		expect(frame).toContain('shift+tab to cycle');
	});

	test('shows plan mode hint', () => {
		const { lastFrame } = render(<StatusBar planMode />);
		expect(lastFrame()).toContain('plan mode');
	});

	test('shows verbose hint', () => {
		const { lastFrame } = render(<StatusBar verbose />);
		expect(lastFrame()).toContain('verbose on');
	});

	test('joins hints with middot separator', () => {
		const { lastFrame } = render(<StatusBar planMode verbose />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('\u00b7');
	});
});
