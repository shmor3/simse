import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
import { render } from 'ink-testing-library';
import { useState } from 'react';
import { TextInput } from '../components/input/text-input.js';

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
		const frame = lastFrame()!;
		expect(frame).toContain('line one');
		expect(frame).toContain('line two');
	});

	test('continuation lines are indented with 2 spaces', () => {
		const { lastFrame } = render(
			<TextInput value={'first\nsecond'} onChange={() => {}} />,
		);
		const frame = lastFrame()!;
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
		const frame = lastFrame()!;
		const outputLines = frame.split('\n');
		expect(outputLines[0]).toContain('hello');
		expect(outputLines[1]).toMatch(/^ {2}/);
		expect(outputLines[1]).toContain('world');
	});
});
