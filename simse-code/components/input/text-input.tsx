import chalk from 'chalk';
import { Text, useInput } from 'ink';
import React, { useCallback, useRef, useState } from 'react';

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

	// Refs for stable callback access — synced from props during render
	// AND eagerly updated inside the handler to avoid stale reads between renders
	const valueRef = useRef(value);
	const cursorRef = useRef(cursorOffset);
	const onChangeRef = useRef(onChange);
	const onSubmitRef = useRef(onSubmit);

	// Sync from props each render
	valueRef.current = value;
	cursorRef.current = cursorOffset;
	onChangeRef.current = onChange;
	onSubmitRef.current = onSubmit;

	const handleInput = useCallback(
		(
			input: string,
			key: {
				upArrow: boolean;
				downArrow: boolean;
				leftArrow: boolean;
				rightArrow: boolean;
				return: boolean;
				backspace: boolean;
				delete: boolean;
				ctrl: boolean;
				shift: boolean;
				tab: boolean;
				escape: boolean;
			},
		) => {
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
				onSubmitRef.current?.(valueRef.current);
				return;
			}

			if (key.backspace || key.delete) {
				const c = cursorRef.current;
				if (c > 0) {
					const v = valueRef.current;
					const next = v.slice(0, c - 1) + v.slice(c);
					// Eagerly update refs so next event in same batch sees correct state
					valueRef.current = next;
					cursorRef.current = c - 1;
					setCursorOffset(c - 1);
					onChangeRef.current(next);
				}
				return;
			}

			if (key.leftArrow) {
				const c = Math.max(0, cursorRef.current - 1);
				cursorRef.current = c;
				setCursorOffset(c);
				return;
			}

			if (key.rightArrow) {
				const c = Math.min(valueRef.current.length, cursorRef.current + 1);
				cursorRef.current = c;
				setCursorOffset(c);
				return;
			}

			// Regular character input (including paste)
			const v = valueRef.current;
			const c = cursorRef.current;
			const next = v.slice(0, c) + input + v.slice(c);
			const newCursor = c + input.length;
			// Eagerly update refs so next event in same batch sees correct state
			valueRef.current = next;
			cursorRef.current = newCursor;
			setCursorOffset(newCursor);
			onChangeRef.current(next);
		},
		[],
	);

	useInput(handleInput, { isActive });

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
