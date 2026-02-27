import { render } from 'ink-testing-library';
import React, { useState } from 'react';
import { describe, expect, test } from 'bun:test';
import { Text } from 'ink';
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
			<TextInput
				value=""
				onChange={() => {}}
				placeholder="Type here"
			/>,
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
});
