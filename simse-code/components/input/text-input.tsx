import chalk from 'chalk';
import { Box, Text, useInput } from 'ink';
import { useCallback, useEffect, useRef, useState } from 'react';

interface TextInputProps {
	readonly value: string;
	readonly onChange: (value: string) => void;
	readonly onSubmit?: (value: string) => void;
	readonly isActive?: boolean;
	readonly placeholder?: string;
	/** Ghost text shown after cursor when at end of input. Accepted with Right Arrow. */
	readonly suggestion?: string;
}

/**
 * Map a flat cursor offset (index into the full string) to
 * { lineIndex, col } within the split lines array.
 */
function cursorToLineCol(
	lines: string[],
	offset: number,
): { lineIndex: number; col: number } {
	let remaining = offset;
	for (let i = 0; i < lines.length; i++) {
		// biome-ignore lint/style/noNonNullAssertion: index is bounds-checked by loop condition
		const lineLen = lines[i]!.length;
		if (remaining <= lineLen) {
			return { lineIndex: i, col: remaining };
		}
		// +1 accounts for the '\n' between lines
		remaining -= lineLen + 1;
	}
	// Fallback: cursor at end of last line
	const last = lines.length - 1;
	return { lineIndex: last, col: lines[last]!.length };
}

export function TextInput({
	value,
	onChange,
	onSubmit,
	isActive = true,
	placeholder = '',
	suggestion,
}: TextInputProps) {
	const [cursorOffset, setCursorOffset] = useState(value.length);

	// Refs for stable callback access — synced from props during render
	// AND eagerly updated inside the handler to avoid stale reads between renders
	const valueRef = useRef(value);
	const cursorRef = useRef(cursorOffset);
	const onChangeRef = useRef(onChange);
	const onSubmitRef = useRef(onSubmit);
	const suggestionRef = useRef(suggestion);

	// Sync from props each render
	valueRef.current = value;
	cursorRef.current = cursorOffset;
	onChangeRef.current = onChange;
	onSubmitRef.current = onSubmit;
	suggestionRef.current = suggestion;

	// Clamp cursor when value changes externally (e.g., autocomplete fill)
	useEffect(() => {
		if (cursorOffset > value.length) {
			const clamped = value.length;
			setCursorOffset(clamped);
			cursorRef.current = clamped;
		}
	}, [value, cursorOffset]);

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
				meta: boolean;
			},
		) => {
			if (
				key.upArrow ||
				key.downArrow ||
				(key.ctrl && (input === 'c' || input === 'l')) ||
				key.tab ||
				(key.shift && key.tab)
			) {
				return;
			}

			if (key.return) {
				// Shift+Enter (Kitty protocol) or Alt/Option+Enter (legacy terminals)
				// inserts a newline instead of submitting
				if (key.shift || key.meta) {
					const v = valueRef.current;
					const c = cursorRef.current;
					const next = `${v.slice(0, c)}\n${v.slice(c)}`;
					const newCursor = c + 1;
					valueRef.current = next;
					cursorRef.current = newCursor;
					setCursorOffset(newCursor);
					onChangeRef.current(next);
					return;
				}
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
				const v = valueRef.current;
				const c = cursorRef.current;
				// Accept suggestion when cursor is at end and suggestion exists
				if (c === v.length && suggestionRef.current) {
					const next = v + suggestionRef.current;
					const newCursor = next.length;
					valueRef.current = next;
					cursorRef.current = newCursor;
					setCursorOffset(newCursor);
					onChangeRef.current(next);
					return;
				}
				const nc = Math.min(v.length, c + 1);
				cursorRef.current = nc;
				setCursorOffset(nc);
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
		if (!value.includes('\n')) {
			return <Text dimColor>{value || placeholder}</Text>;
		}
		// Multi-line inactive: indent continuation lines
		const lines = value.split('\n');
		return (
			<Box flexDirection="column">
				{lines.map((line, i) => (
					// biome-ignore lint/suspicious/noArrayIndexKey: lines derived from value split, index is stable
					<Text key={i} dimColor>
						{i > 0 ? `  ${line}` : line}
					</Text>
				))}
			</Box>
		);
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

	// Single-line fast path
	if (!value.includes('\n')) {
		let rendered = '';
		for (let i = 0; i < value.length; i++) {
			const ch = value[i] ?? '';
			rendered += i === cursorOffset ? chalk.inverse(ch) : ch;
		}
		if (cursorOffset === value.length) {
			rendered += chalk.inverse(' ');
			// Show ghost suggestion after cursor when at end of input
			if (suggestion) {
				rendered += chalk.gray(suggestion);
			}
		}
		return <Text>{rendered}</Text>;
	}

	// Multi-line rendering
	const lines = value.split('\n');
	const { lineIndex: cursorLine, col: cursorCol } = cursorToLineCol(
		lines,
		cursorOffset,
	);

	return (
		<Box flexDirection="column">
			{lines.map((line, i) => {
				const isCursorLine = i === cursorLine;
				const indent = i > 0 ? '  ' : '';

				let rendered = '';
				for (let j = 0; j < line.length; j++) {
					const ch = line[j] ?? '';
					rendered += isCursorLine && j === cursorCol ? chalk.inverse(ch) : ch;
				}
				// Cursor at end of this line
				if (isCursorLine && cursorCol === line.length) {
					rendered += chalk.inverse(' ');
				}

				return (
					// biome-ignore lint/suspicious/noArrayIndexKey: lines derived from value split, index is stable
					<Text key={i}>
						{indent}
						{rendered}
					</Text>
				);
			})}
		</Box>
	);
}
