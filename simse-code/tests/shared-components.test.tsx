import { describe, expect, test } from 'bun:test';
import { render } from 'ink-testing-library';
import { Badge } from '../components/shared/badge.js';
import { ErrorBox } from '../components/shared/error-box.js';
import {
	formatDuration,
	formatTokens,
	ThinkingSpinner,
} from '../components/shared/spinner.js';

describe('formatDuration', () => {
	test('formats sub-second as milliseconds', () => {
		expect(formatDuration(500)).toBe('500ms');
		expect(formatDuration(0)).toBe('0ms');
		expect(formatDuration(999)).toBe('999ms');
	});

	test('formats seconds with one decimal', () => {
		expect(formatDuration(1000)).toBe('1.0s');
		expect(formatDuration(3200)).toBe('3.2s');
		expect(formatDuration(59999)).toBe('60.0s');
	});

	test('formats minutes and seconds', () => {
		expect(formatDuration(60000)).toBe('1m0s');
		expect(formatDuration(90000)).toBe('1m30s');
		expect(formatDuration(125000)).toBe('2m5s');
	});
});

describe('formatTokens', () => {
	test('formats tokens below 1000 without suffix', () => {
		expect(formatTokens(0)).toBe('0 tokens');
		expect(formatTokens(500)).toBe('500 tokens');
		expect(formatTokens(999)).toBe('999 tokens');
	});

	test('formats tokens >= 1000 with k suffix', () => {
		expect(formatTokens(1000)).toBe('1.0k tokens');
		expect(formatTokens(1500)).toBe('1.5k tokens');
		expect(formatTokens(12345)).toBe('12.3k tokens');
	});
});

describe('ThinkingSpinner', () => {
	test('renders with default label', () => {
		const { lastFrame } = render(<ThinkingSpinner />);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Thinking...');
	});

	test('renders with custom label', () => {
		const { lastFrame } = render(<ThinkingSpinner label="Searching" />);
		expect(lastFrame()).toContain('Searching...');
	});

	test('shows formatted suffix with elapsed, tokens, and server', () => {
		const { lastFrame } = render(
			<ThinkingSpinner elapsed={3200} tokens={1500} server="claude-sonnet" />,
		);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('3.2s');
		expect(frame).toContain('1.5k tokens');
		expect(frame).toContain('claude-sonnet');
		expect(frame).toContain('\u00b7');
	});

	test('elapsed time formatted as milliseconds under 1s', () => {
		const { lastFrame } = render(<ThinkingSpinner elapsed={750} />);
		expect(lastFrame()).toContain('750ms');
	});

	test('elapsed time formatted as seconds', () => {
		const { lastFrame } = render(<ThinkingSpinner elapsed={3200} />);
		expect(lastFrame()).toContain('3.2s');
	});

	test('tokens formatted with k suffix', () => {
		const { lastFrame } = render(<ThinkingSpinner tokens={1500} />);
		expect(lastFrame()).toContain('1.5k tokens');
	});

	test('tokens below 1000 shown as plain number', () => {
		const { lastFrame } = render(<ThinkingSpinner tokens={42} />);
		expect(lastFrame()).toContain('42 tokens');
	});

	test('no suffix when no optional props provided', () => {
		const { lastFrame } = render(<ThinkingSpinner />);
		const frame = lastFrame() ?? '';
		expect(frame).not.toContain('(');
		expect(frame).not.toContain('\u00b7');
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
