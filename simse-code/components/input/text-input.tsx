import chalk from 'chalk';
import { Box, Text, useInput } from 'ink';
import { useCallback, useEffect, useRef, useState } from 'react';

const IS_MAC = process.platform === 'darwin';

/** Render a single character with cursor or selection highlight. */
function renderChar(ch: string, isCursor: boolean, isSelected: boolean): string {
	if (isCursor) return chalk.inverse(ch);
	if (isSelected) return chalk.bgCyan.black(ch);
	return ch;
}

/** Scan left to find the start of the previous word. */
export function findWordBoundaryLeft(text: string, pos: number): number {
	let i = pos;
	// Skip non-word chars to the left
	while (i > 0 && !/\w/.test(text[i - 1] ?? '')) i--;
	// Skip word chars to the left
	while (i > 0 && /\w/.test(text[i - 1] ?? '')) i--;
	return i;
}

/** Scan right to find the end of the current word. */
export function findWordBoundaryRight(text: string, pos: number): number {
	let i = pos;
	const len = text.length;
	if (i < len && !/\w/.test(text[i] ?? '')) {
		// Starting on non-word chars: skip them, then skip the next word
		while (i < len && !/\w/.test(text[i] ?? '')) i++;
		while (i < len && /\w/.test(text[i] ?? '')) i++;
	} else {
		// Starting on a word char: skip to end of current word
		while (i < len && /\w/.test(text[i] ?? '')) i++;
	}
	return i;
}

interface TextInputProps {
	readonly value: string;
	readonly onChange: (value: string) => void;
	readonly onSubmit?: (value: string) => void;
	readonly isActive?: boolean;
	readonly placeholder?: string;
	/** Ghost text shown after cursor when at end of input. Accepted with Right Arrow. */
	readonly suggestion?: string;
	/** When true, return/tab/rightArrow are suppressed so the parent can handle them for autocomplete. */
	readonly interceptKeys?: boolean;
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
	return { lineIndex: last, col: (lines[last] ?? '').length };
}

export function TextInput({
	value,
	onChange,
	onSubmit,
	isActive = true,
	placeholder = '',
	suggestion,
	interceptKeys = false,
}: TextInputProps) {
	const [cursorOffset, setCursorOffset] = useState(value.length);
	const [selectionAnchor, setSelectionAnchor] = useState<number | null>(null);
	const anchorRef = useRef<number | null>(null);

	// Sync anchor ref
	anchorRef.current = selectionAnchor;

	// Refs for stable callback access — synced from props during render
	// AND eagerly updated inside the handler to avoid stale reads between renders
	const valueRef = useRef(value);
	const cursorRef = useRef(cursorOffset);
	const onChangeRef = useRef(onChange);
	const onSubmitRef = useRef(onSubmit);
	const suggestionRef = useRef(suggestion);
	const interceptKeysRef = useRef(interceptKeys);
	const internalChangeRef = useRef(false);

	// Sync from props each render
	valueRef.current = value;
	cursorRef.current = cursorOffset;
	onChangeRef.current = onChange;
	onSubmitRef.current = onSubmit;
	suggestionRef.current = suggestion;
	interceptKeysRef.current = interceptKeys;

	// Move cursor to end when value changes externally (e.g., autocomplete fill, history)
	// Internal changes (user typing) set the flag so we skip the override.
	useEffect(() => {
		if (internalChangeRef.current) {
			internalChangeRef.current = false;
			return;
		}
		const end = value.length;
		setCursorOffset(end);
		cursorRef.current = end;
	}, [value]);

	/** Delete the selected range and return { newValue, newCursor }. */
	function deleteSelection(): { value: string; cursor: number } | null {
		const anchor = anchorRef.current;
		if (anchor === null) return null;
		const cursor = cursorRef.current;
		const v = valueRef.current;
		const start = Math.min(anchor, cursor);
		const end = Math.max(anchor, cursor);
		if (start === end) return null;
		return { value: v.slice(0, start) + v.slice(end), cursor: start };
	}

	/** Clear selection state. */
	function clearSelection(): void {
		anchorRef.current = null;
		setSelectionAnchor(null);
	}

	/** Set or extend selection. If no anchor yet, set anchor at current cursor pos, then move cursor. */
	function extendSelection(newCursor: number): void {
		if (anchorRef.current === null) {
			const anchor = cursorRef.current;
			anchorRef.current = anchor;
			setSelectionAnchor(anchor);
		}
		cursorRef.current = newCursor;
		setCursorOffset(newCursor);
	}

	/** Collapse selection in a direction: 'left' = move to start, 'right' = move to end. */
	function collapseSelection(direction: 'left' | 'right'): number {
		const anchor = anchorRef.current;
		if (anchor === null) return cursorRef.current;
		const cursor = cursorRef.current;
		const pos =
			direction === 'left'
				? Math.min(anchor, cursor)
				: Math.max(anchor, cursor);
		clearSelection();
		return pos;
	}

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
				home: boolean;
				end: boolean;
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

			// Select all: Ctrl+A on all platforms
			if (key.ctrl && input === 'a') {
				const v = valueRef.current;
				if (v.length > 0) {
					anchorRef.current = 0;
					setSelectionAnchor(0);
					cursorRef.current = v.length;
					setCursorOffset(v.length);
				}
				return;
			}

			// Home: move to start of input
			if (key.home) {
				if (key.shift) {
					extendSelection(0);
				} else {
					if (anchorRef.current !== null) clearSelection();
					cursorRef.current = 0;
					setCursorOffset(0);
				}
				return;
			}

			// End: move to end of input
			if (key.end) {
				const v = valueRef.current;
				if (key.shift) {
					extendSelection(v.length);
				} else {
					if (anchorRef.current !== null) clearSelection();
					cursorRef.current = v.length;
					setCursorOffset(v.length);
				}
				return;
			}

			// Word navigation: meta+arrow on macOS, ctrl+arrow on Linux/Windows
			const isWordModifier = IS_MAC ? key.meta : key.ctrl;

			if (isWordModifier && key.leftArrow) {
				const v = valueRef.current;
				const c = cursorRef.current;
				const target = findWordBoundaryLeft(v, c);
				if (key.shift) {
					extendSelection(target);
				} else {
					if (anchorRef.current !== null) clearSelection();
					cursorRef.current = target;
					setCursorOffset(target);
				}
				return;
			}

			if (isWordModifier && key.rightArrow) {
				const v = valueRef.current;
				const c = cursorRef.current;
				const target = findWordBoundaryRight(v, c);
				if (key.shift) {
					extendSelection(target);
				} else {
					if (anchorRef.current !== null) clearSelection();
					cursorRef.current = target;
					setCursorOffset(target);
				}
				return;
			}

			// Word delete: meta+backspace on macOS, ctrl+backspace on Linux/Windows
			if (isWordModifier && (key.backspace || key.delete)) {
				const v = valueRef.current;
				const c = cursorRef.current;
				// Delete selection first if any
				const sel = deleteSelection();
				if (sel) {
					clearSelection();
					valueRef.current = sel.value;
					cursorRef.current = sel.cursor;
					setCursorOffset(sel.cursor);
					internalChangeRef.current = true;
					onChangeRef.current(sel.value);
					return;
				}
				if (key.backspace) {
					const target = findWordBoundaryLeft(v, c);
					const next = v.slice(0, target) + v.slice(c);
					valueRef.current = next;
					cursorRef.current = target;
					setCursorOffset(target);
					internalChangeRef.current = true;
					onChangeRef.current(next);
				} else {
					const target = findWordBoundaryRight(v, c);
					const next = v.slice(0, c) + v.slice(target);
					valueRef.current = next;
					internalChangeRef.current = true;
					onChangeRef.current(next);
				}
				return;
			}

			// When interceptKeys is active, let the parent handle return and rightArrow
			if (interceptKeysRef.current && (key.return || key.rightArrow)) {
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
					internalChangeRef.current = true;
					onChangeRef.current(next);
					return;
				}
				onSubmitRef.current?.(valueRef.current);
				return;
			}

			if (key.backspace || key.delete) {
				const sel = deleteSelection();
				if (sel) {
					clearSelection();
					valueRef.current = sel.value;
					cursorRef.current = sel.cursor;
					setCursorOffset(sel.cursor);
					internalChangeRef.current = true;
					onChangeRef.current(sel.value);
					return;
				}
				const c = cursorRef.current;
				if (c > 0) {
					const v = valueRef.current;
					const next = v.slice(0, c - 1) + v.slice(c);
					// Eagerly update refs so next event in same batch sees correct state
					valueRef.current = next;
					cursorRef.current = c - 1;
					setCursorOffset(c - 1);
					internalChangeRef.current = true;
					onChangeRef.current(next);
				}
				return;
			}

			if (key.leftArrow) {
				if (key.shift) {
					const c = Math.max(0, cursorRef.current - 1);
					extendSelection(c);
					return;
				}
				// Collapse selection if active
				if (anchorRef.current !== null) {
					const pos = collapseSelection('left');
					cursorRef.current = pos;
					setCursorOffset(pos);
					return;
				}
				const c = Math.max(0, cursorRef.current - 1);
				cursorRef.current = c;
				setCursorOffset(c);
				return;
			}

			if (key.rightArrow) {
				if (key.shift) {
					const v = valueRef.current;
					const c = Math.min(v.length, cursorRef.current + 1);
					extendSelection(c);
					return;
				}
				// Collapse selection if active
				if (anchorRef.current !== null) {
					const pos = collapseSelection('right');
					cursorRef.current = pos;
					setCursorOffset(pos);
					return;
				}
				const v = valueRef.current;
				const c = cursorRef.current;
				// Accept suggestion when cursor is at end and suggestion exists
				if (c === v.length && suggestionRef.current) {
					const next = v + suggestionRef.current;
					const newCursor = next.length;
					valueRef.current = next;
					cursorRef.current = newCursor;
					setCursorOffset(newCursor);
					internalChangeRef.current = true;
					onChangeRef.current(next);
					return;
				}
				const nc = Math.min(v.length, c + 1);
				cursorRef.current = nc;
				setCursorOffset(nc);
				return;
			}

			// Regular character input (including paste)
			const sel = deleteSelection();
			if (sel) {
				clearSelection();
				const next =
					sel.value.slice(0, sel.cursor) +
					input +
					sel.value.slice(sel.cursor);
				const newCursor = sel.cursor + input.length;
				valueRef.current = next;
				cursorRef.current = newCursor;
				setCursorOffset(newCursor);
				internalChangeRef.current = true;
				onChangeRef.current(next);
				return;
			}
			clearSelection(); // Clear any degenerate selection (anchor === cursor) on regular typing
			const v = valueRef.current;
			const c = cursorRef.current;
			const next = v.slice(0, c) + input + v.slice(c);
			const newCursor = c + input.length;
			// Eagerly update refs so next event in same batch sees correct state
			valueRef.current = next;
			cursorRef.current = newCursor;
			setCursorOffset(newCursor);
			internalChangeRef.current = true;
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
		const selStart =
			selectionAnchor !== null
				? Math.min(selectionAnchor, cursorOffset)
				: -1;
		const selEnd =
			selectionAnchor !== null
				? Math.max(selectionAnchor, cursorOffset)
				: -1;
		let rendered = '';
		for (let i = 0; i < value.length; i++) {
			const ch = value[i] ?? '';
			const isCursor = i === cursorOffset;
			const isSelected =
				selStart !== -1 && i >= selStart && i < selEnd;
			rendered += renderChar(ch, isCursor, isSelected);
		}
		if (cursorOffset === value.length) {
			rendered += chalk.inverse(' ');
			// Only show ghost suggestion when no selection is active
			if (suggestion && selectionAnchor === null) {
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
	const selStart =
		selectionAnchor !== null
			? Math.min(selectionAnchor, cursorOffset)
			: -1;
	const selEnd =
		selectionAnchor !== null
			? Math.max(selectionAnchor, cursorOffset)
			: -1;

	return (
		<Box flexDirection="column">
			{lines.map((line, i) => {
				const isCursorLine = i === cursorLine;
				const indent = i > 0 ? '  ' : '';

				// Compute flat offset of the start of this line
				let lineStartOffset = 0;
				for (let li = 0; li < i; li++) {
					lineStartOffset += (lines[li] ?? '').length + 1; // +1 for '\n'
				}

				let rendered = '';
				for (let j = 0; j < line.length; j++) {
					const ch = line[j] ?? '';
					const flatOffset = lineStartOffset + j;
					const isCursor = isCursorLine && j === cursorCol;
					const isSelected =
						selStart !== -1 &&
						flatOffset >= selStart &&
						flatOffset < selEnd;
					rendered += renderChar(ch, isCursor, isSelected);
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
