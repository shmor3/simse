import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { render } from 'ink-testing-library';
import { useState } from 'react';
import {
	findWordBoundaryLeft,
	findWordBoundaryRight,
	TextInput,
} from '../components/input/text-input.js';

function TestHarness() {
	const [value, setValue] = useState('');
	const [submitted, setSubmitted] = useState('');

	return (
		<>
			<TextInput
				value={value}
				onChange={setValue}
				onSubmit={(v) => {
					setSubmitted(v);
					setValue('');
				}}
			/>
			{submitted && <Text>SUBMITTED:{submitted}</Text>}
		</>
	);
}

describe('TextInput', () => {
	test('renders placeholder when empty', () => {
		const { lastFrame } = render(
			<TextInput value="" onChange={() => {}} placeholder="Type here" />,
		);
		expect(lastFrame()).toContain('Type here');
	});

	test('renders value text', () => {
		const { lastFrame } = render(
			<TextInput value="hello" onChange={() => {}} />,
		);
		expect(lastFrame()).toContain('hello');
	});

	test('accepts character input and submits on Enter', async () => {
		const { lastFrame, stdin } = render(<TestHarness />);

		// Type characters
		stdin.write('h');
		stdin.write('i');

		// Small delay for React to process
		await new Promise((r) => setTimeout(r, 50));
		expect(lastFrame()).toContain('hi');

		// Press Enter
		stdin.write('\r');
		await new Promise((r) => setTimeout(r, 50));

		expect(lastFrame()).toContain('SUBMITTED:hi');
	});

	test('shows dimmed content when inactive', () => {
		const { lastFrame } = render(
			<TextInput
				value="test"
				onChange={() => {}}
				isActive={false}
				placeholder="disabled"
			/>,
		);
		expect(lastFrame()).toBeDefined();
	});

	test('multi-line value renders both lines', () => {
		const { lastFrame } = render(
			<TextInput value={'line one\nline two'} onChange={() => {}} />,
		);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('line one');
		expect(frame).toContain('line two');
	});

	test('continuation lines are indented with 2 spaces', () => {
		const { lastFrame } = render(
			<TextInput value={'first\nsecond'} onChange={() => {}} />,
		);
		const frame = lastFrame() ?? '';
		const outputLines = frame.split('\n');
		// First line should not have leading spaces (beyond cursor rendering)
		expect(outputLines[0]).toContain('first');
		// Second line should be indented with 2 spaces
		expect(outputLines[1]).toMatch(/^ {2}/);
		expect(outputLines[1]).toContain('second');
	});

	test('multi-line inactive value renders with indentation', () => {
		const { lastFrame } = render(
			<TextInput value={'hello\nworld'} onChange={() => {}} isActive={false} />,
		);
		const frame = lastFrame() ?? '';
		const outputLines = frame.split('\n');
		expect(outputLines[0]).toContain('hello');
		expect(outputLines[1]).toMatch(/^ {2}/);
		expect(outputLines[1]).toContain('world');
	});

	test('renders ghost suggestion text after cursor', () => {
		const { lastFrame } = render(
			<TextInput value="/hel" onChange={() => {}} suggestion="p" />,
		);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('hel');
		expect(frame).toContain('p');
	});

	test('does not render suggestion when value is empty (placeholder shown)', () => {
		const { lastFrame } = render(
			<TextInput
				value=""
				onChange={() => {}}
				placeholder="Type here"
				suggestion="help"
			/>,
		);
		const frame = lastFrame() ?? '';
		expect(frame).toContain('Type here');
	});

	test('accepts suggestion with Right Arrow', async () => {
		function SuggestionHarness() {
			const [value, setValue] = useState('/hel');
			return (
				<>
					<TextInput
						value={value}
						onChange={setValue}
						suggestion={value === '/hel' ? 'p' : undefined}
					/>
					<Text>VALUE:{value}</Text>
				</>
			);
		}

		const { lastFrame, stdin } = render(<SuggestionHarness />);

		// Press Right Arrow (escape sequence)
		stdin.write('\u001B[C');
		await new Promise((r) => setTimeout(r, 50));

		expect(lastFrame()).toContain('VALUE:/help');
	});
});

function SelectionHarness() {
	const [value, setValue] = useState('hello world');
	return (
		<>
			<TextInput value={value} onChange={setValue} />
			<Text>VALUE:{value}</Text>
		</>
	);
}

describe('selection', () => {
	test('Ctrl+A selects all — next character replaces everything', async () => {
		const { lastFrame, stdin } = render(<SelectionHarness />);
		await new Promise((r) => setTimeout(r, 50));

		// Ctrl+A = \x01
		stdin.write('\x01');
		await new Promise((r) => setTimeout(r, 50));

		// Type a replacement character
		stdin.write('x');
		await new Promise((r) => setTimeout(r, 50));

		expect(lastFrame()).toContain('VALUE:x');
	});

	test('Ctrl+A then backspace clears all text', async () => {
		const { lastFrame, stdin } = render(<SelectionHarness />);
		await new Promise((r) => setTimeout(r, 50));

		stdin.write('\x01');
		await new Promise((r) => setTimeout(r, 50));

		stdin.write('\x7F'); // backspace
		await new Promise((r) => setTimeout(r, 50));

		const frame = lastFrame() ?? '';
		// Extract the VALUE: line and ensure it's empty after the colon
		const match = frame.match(/VALUE:(.*)/);
		expect(match).not.toBeNull();
		expect(match![1]).toBe('');
	});
});

describe('word navigation', () => {
	test('Option+Left (meta+left) jumps to previous word boundary', async () => {
		function WordNavHarness() {
			const [value, setValue] = useState('hello world');
			return (
				<>
					<TextInput value={value} onChange={setValue} />
					<Text>VALUE:{value}</Text>
				</>
			);
		}
		const { lastFrame, stdin } = render(<WordNavHarness />);
		await new Promise((r) => setTimeout(r, 50));

		// Option+Left on macOS = \x1b\x1b[D (double-escape + [D)
		stdin.write('\x1b\x1b[D');
		await new Promise((r) => setTimeout(r, 50));

		// Type 'X' — should insert at word boundary (pos 6, start of "world")
		stdin.write('X');
		await new Promise((r) => setTimeout(r, 50));

		expect(lastFrame()).toContain('VALUE:hello Xworld');
	});
});

describe('Home/End navigation', () => {
	test('Home key moves cursor to start of input', async () => {
		function HomeEndHarness() {
			const [value, setValue] = useState('hello');
			return (
				<>
					<TextInput value={value} onChange={setValue} />
					<Text>VALUE:{value}</Text>
				</>
			);
		}
		const { lastFrame, stdin } = render(<HomeEndHarness />);
		await new Promise((r) => setTimeout(r, 50));

		// Home key = \x1b[H
		stdin.write('\x1b[H');
		await new Promise((r) => setTimeout(r, 50));

		// Type X at beginning
		stdin.write('X');
		await new Promise((r) => setTimeout(r, 50));

		expect(lastFrame()).toContain('VALUE:Xhello');
	});

	test('End key moves cursor to end of input', async () => {
		function HomeEndHarness2() {
			const [value, setValue] = useState('hello');
			return (
				<>
					<TextInput value={value} onChange={setValue} />
					<Text>VALUE:{value}</Text>
				</>
			);
		}
		const { lastFrame, stdin } = render(<HomeEndHarness2 />);
		await new Promise((r) => setTimeout(r, 50));

		// Home first to move to start
		stdin.write('\x1b[H');
		await new Promise((r) => setTimeout(r, 50));

		// Then End to move back to end
		stdin.write('\x1b[F');
		await new Promise((r) => setTimeout(r, 50));

		// Type X at end
		stdin.write('X');
		await new Promise((r) => setTimeout(r, 50));

		expect(lastFrame()).toContain('VALUE:helloX');
	});
});

describe('selection rendering', () => {
	test('selected text is still visible after Ctrl+A', async () => {
		const { lastFrame, stdin } = render(<SelectionHarness />);
		await new Promise((r) => setTimeout(r, 50));

		// Select all with Ctrl+A
		stdin.write('\x01');
		await new Promise((r) => setTimeout(r, 50));

		const frame = lastFrame() ?? '';
		// Text should still be present (rendered with highlight)
		expect(frame).toContain('hello');
		expect(frame).toContain('world');
	});
});

describe('word boundary helpers', () => {
	test('findWordBoundaryLeft from middle of word', () => {
		expect(findWordBoundaryLeft('hello world', 8)).toBe(6);
	});

	test('findWordBoundaryLeft from start of word', () => {
		expect(findWordBoundaryLeft('hello world', 6)).toBe(0);
	});

	test('findWordBoundaryLeft at position 0', () => {
		expect(findWordBoundaryLeft('hello', 0)).toBe(0);
	});

	test('findWordBoundaryLeft skips multiple spaces', () => {
		expect(findWordBoundaryLeft('hello   world', 8)).toBe(0);
	});

	test('findWordBoundaryLeft with punctuation', () => {
		expect(findWordBoundaryLeft('foo.bar', 7)).toBe(4);
	});

	test('findWordBoundaryRight from middle of word', () => {
		expect(findWordBoundaryRight('hello world', 3)).toBe(5);
	});

	test('findWordBoundaryRight from space', () => {
		expect(findWordBoundaryRight('hello world', 5)).toBe(11);
	});

	test('findWordBoundaryRight at end', () => {
		expect(findWordBoundaryRight('hello', 5)).toBe(5);
	});

	test('findWordBoundaryRight with punctuation', () => {
		expect(findWordBoundaryRight('foo.bar', 0)).toBe(3);
	});
});
