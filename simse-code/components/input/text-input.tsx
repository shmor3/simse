import chalk from 'chalk';
import { Text, useInput } from 'ink';
import React, { useEffect, useState } from 'react';

interface TextInputProps {
	readonly value: string;
	readonly onChange: (value: string) => void;
	readonly onSubmit?: (value: string) => void;
	readonly isActive?: boolean;
	readonly placeholder?: string;
}

export function TextInput({
	value,
	onChange,
	onSubmit,
	isActive = true,
	placeholder = '',
}: TextInputProps) {
	const [cursorOffset, setCursorOffset] = useState(value.length);

	useEffect(() => {
		if (cursorOffset > value.length) {
			setCursorOffset(value.length);
		}
	}, [value, cursorOffset]);

	useInput(
		(input, key) => {
			if (
				key.upArrow ||
				key.downArrow ||
				(key.ctrl && input === 'c') ||
				key.tab ||
				(key.shift && key.tab)
			) {
				return;
			}

			if (key.return) {
				onSubmit?.(value);
				return;
			}

			if (key.backspace || key.delete) {
				if (cursorOffset > 0) {
					const next =
						value.slice(0, cursorOffset - 1) + value.slice(cursorOffset);
					setCursorOffset((c) => c - 1);
					onChange(next);
				}
				return;
			}

			if (key.leftArrow) {
				setCursorOffset((c) => Math.max(0, c - 1));
				return;
			}

			if (key.rightArrow) {
				setCursorOffset((c) => Math.min(value.length, c + 1));
				return;
			}

			// Regular character input (including paste)
			const next =
				value.slice(0, cursorOffset) + input + value.slice(cursorOffset);
			setCursorOffset((c) => c + input.length);
			onChange(next);
		},
		{ isActive },
	);

	// When inactive, show dimmed value or placeholder
	if (!isActive) {
		return <Text dimColor>{value || placeholder}</Text>;
	}

	// Empty with placeholder — show cursor on first char of placeholder
	if (value.length === 0) {
		if (placeholder.length > 0) {
			return (
				<Text>
					{chalk.inverse(placeholder[0])}
					{chalk.gray(placeholder.slice(1))}
				</Text>
			);
		}
		return <Text>{chalk.inverse(' ')}</Text>;
	}

	// Render value with fake inverse cursor
	let rendered = '';
	for (let i = 0; i < value.length; i++) {
		rendered += i === cursorOffset ? chalk.inverse(value[i]!) : value[i]!;
	}
	if (cursorOffset === value.length) {
		rendered += chalk.inverse(' ');
	}

	return <Text>{rendered}</Text>;
}
